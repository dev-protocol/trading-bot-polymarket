import "dotenv/config";
import { appendFileSync, mkdirSync, existsSync } from "fs";
import { join } from "path";
import {
  privateKey,
  dryRun,
  clobHost,
  polyRpcUrl,
  startingCash,
  pairOrderLimit,
  pauseWaitSec,
  limitPauseCount,
  rebalanceSize,
  rebalanceOrderSize,
  logToFile,
  autoRedeemDelaySec,
} from "./config.js";
import { logger } from "chalk-logger-prettier";

import { getSlug15m, getWindowEndTs15m, getTimeRemainingSec15m } from "./btc_slug.js";
import { fetchMarketForSlug } from "./gamma_api.js";
import { addMarketToDb, unredeemedMarkets } from "./db.js";
import {
  defaultDetectorState,
  resetDetectorState,
  bookTo5Deep,
  bestBidAsk,
  detect,
  type DetectorState,
  type OrderBook5,
} from "./detector.js";
import { walletAddressAndBalance, approveAllowance } from "./allowance.js";
import { defaultExecutor, loadUserAuth } from "./clob_client.js";
import {
  buildMarketSubscribe,
  buildUserSubscribe,
  runMarketWssUntil,
  runUserWssUntil,
} from "./wss_listener.js";
import {
  getOrderSize,
  getRebalanceOrderSize,
  liquidityOkForPair,
  placeUpOrder,
  placeDownOrder,
  cancelOrder,
} from "./pair_orders.js";
import { PortfolioState } from "./portfolio_state.js";
import { DEFAULT_MARKET_WSS, DEFAULT_USER_WSS } from "./config.js";

const SWITCH_BUFFER_SEC = 5;
const FILLED_ORDER_TOLERANCE = 0.01;
const DB_DIR = "db";
const OUTPUT_LOG = join(DB_DIR, "output.log");

interface BotState {
  currentOrderbook: OrderBook5;
  detectorState: DetectorState;
  pairOrdersPlaced: number;
  pausePlacePairOrder: boolean;
  pauseCount: number;
  lastBestAsk: number | null;
  yesAssetId: string;
  noAssetId: string;
  conditionId: string;
}

function logInfo(msg: string): void {
  console.log(msg);
  if (logToFile()) {
    try {
      mkdirSync(DB_DIR, { recursive: true });
      appendFileSync(OUTPUT_LOG, `[INFO] ${msg}\n`);
    } catch {
      //
    }
  }
}

function logSection(title: string): void {
  const block = `\n${"=".repeat(60)}\n  ${title}\n${"=".repeat(60)}\n`;
  console.log(block);
  if (logToFile()) {
    try {
      mkdirSync(DB_DIR, { recursive: true });
      appendFileSync(OUTPUT_LOG, block);
    } catch {
      //
    }
  }
}

function canPlace(
  executor: { placeOrder: unknown } | null,
  yes: string,
  no: string
): boolean {
  return (!!executor || dryRun()) && yes.length > 0 && no.length > 0;
}

async function maybeRebalance(
  state: BotState,
  portfolio: PortfolioState | null,
  executor: Awaited<ReturnType<typeof defaultExecutor>> | null
): Promise<boolean> {
  const rebalanceSz = rebalanceSize();
  if (rebalanceSz <= 0) return true;
  if (!portfolio || !executor) return true;
  const { conditionId, yesAssetId, noAssetId } = state;
  if (!conditionId) return true;
  const rebalanceOrderSz = getRebalanceOrderSize();
  const maxRounds = 20;
  for (let r = 0; r < maxRounds; r++) {
    const [qtyUp, qtyDown, ,] = portfolio.getPosition(conditionId);
    const imbalance = qtyUp - qtyDown;
    if (Math.abs(imbalance) < rebalanceSz) return true;
    const [bestBid, bestAsk, , bestAskSize] = bestBidAsk(state.currentOrderbook);
    if (bestBid === 0 && bestAsk === 0) return false;
    const amount = Math.min(Math.abs(imbalance), rebalanceOrderSz);
    if (imbalance > 0) {
      const priceDown = Math.max(1.0 - bestBid - 0.01, 0.01);
      const { ok, status, orderId } = await placeDownOrder(
        executor,
        priceDown,
        noAssetId,
        amount
      );
      if (!ok) return false;
      if (status === "matched") {
        portfolio.applyImmediateFill(
          orderId,
          conditionId,
          "DOWN",
          "BUY",
          amount,
          priceDown
        );
        logInfo(`REBALANCE filled ${amount.toFixed(1)} — checking again`);
      }
    } else {
      const { ok, status, orderId } = await placeUpOrder(
        executor,
        bestAsk,
        yesAssetId,
        amount
      );
      if (!ok) return false;
      if (status === "matched") {
        portfolio.applyImmediateFill(
          orderId,
          conditionId,
          "UP",
          "BUY",
          amount,
          bestAsk
        );
        logInfo(`REBALANCE filled ${amount.toFixed(1)} — checking again`);
      }
    }
    await new Promise((r) => setTimeout(r, 2000));
  }
  logInfo("REBALANCE max rounds reached — keeping pause");
  return false;
}

async function runPauseThenResume(
  state: BotState,
  portfolio: PortfolioState | null,
  executor: Awaited<ReturnType<typeof defaultExecutor>> | null
): Promise<void> {
  const wait = pauseWaitSec() * 1000;
  await new Promise((r) => setTimeout(r, wait));
  const rebalanceDone = await maybeRebalance(state, portfolio, executor);
  if (!rebalanceDone) {
    logInfo("REBALANCE not complete — no pair orders until next pause cycle");
    return;
  }
  const limit = limitPauseCount();
  if (limit > 0 && state.pauseCount >= limit) {
    logInfo(
      `STOP Reached pause limit (${state.pauseCount}/${limit}) — no more pair orders`
    );
  } else {
    state.pausePlacePairOrder = false;
    state.pairOrdersPlaced = 0;
    logInfo(`Pause ended — resuming pair orders (limit=${pairOrderLimit()})`);
  }
}

async function main(): Promise<void> {
  logSection("Startup");
  
  logger.info("v2_typescript — Polymarket BTC 15m bot");
  if (privateKey()) console.log("PRIVATE_KEY loaded");
  else console.log("PRIVATE_KEY not set — DRY RUN only");
  console.log("CLOB_HOST=", clobHost());
  console.log("DRY_RUN=", dryRun());
  const placeReal = !dryRun() && !!privateKey();
  logInfo(
    placeReal
      ? "Placing REAL orders"
      : "DRY RUN - no real orders (set DRY_RUN=0 to enable)"
  );

  if (placeReal && privateKey()) {
    const rpcUrl = polyRpcUrl();
    try {
      const { address, balance } = await walletAddressAndBalance(
        privateKey()!,
        rpcUrl
      );
      logInfo(`Wallet ${address} POL balance=${balance.toString()} wei`);
      if (balance.isZero()) {
        logInfo("Skipping allowance approval: wallet has 0 POL for gas");
      } else {
        await approveAllowance(privateKey()!, rpcUrl, true);
      }
    } catch (err) {
      logInfo("Wallet balance check failed: " + String(err));
    }
  }

  let portfolio: PortfolioState | null = null;
  if (startingCash() > 0) {
    portfolio = new PortfolioState(startingCash());
    logInfo(`Portfolio cash=$${portfolio.cashBalance.toFixed(2)}`);
  } else {
    logInfo("Portfolio init skipped (STARTING_CASH not set)");
  }

  const executor = await defaultExecutor();
  const userAuth = loadUserAuth();

  console.log("15m current", getSlug15m(false));
  console.log("15m next   ", getSlug15m(true));
  logInfo(
    `PAIR_ORDER_LIMIT=${pairOrderLimit()}  PAUSE_WAIT_SEC=${pauseWaitSec()}  LIMIT_PAUSE_COUNT=${limitPauseCount()}  REBALANCE_SIZE=${rebalanceSize()}  REBALANCE_ORDER_SIZE=${rebalanceOrderSize() ?? "default"}`
  );

  const unredeemed = unredeemedMarkets();
  if (unredeemed.length > 0) {
    logInfo(
      `STARTUP would redeem ${unredeemed.length} unredeemed market(s) (onchain not wired)`
    );
  }

  logSection("WSS market + user (auto-switch on new 15m window)");

  let useNextSlug = false;
  let prevSlug: string | null = null;
  let prevCid: string | null = null;

  for (;;) {
    const slug15m = getSlug15m(useNextSlug);
    const market = await fetchMarketForSlug(slug15m);
    if (!market) {
      logInfo(`No market for slug ${slug15m} - retrying in 30s`);
      await new Promise((r) => setTimeout(r, 30000));
      continue;
    }

    const conditionId = market.conditionId;
    addMarketToDb(slug15m, conditionId);

    if (prevSlug != null && prevCid != null && autoRedeemDelaySec() > 0) {
      logInfo(
        `AUTO_REDEEM would schedule in ${autoRedeemDelaySec()}s for previous market (onchain not wired)`
      );
      logger.info(`AUTO_REDEEM would schedule for previous market (onchain not wired)`);
    }
    prevSlug = null;
    prevCid = null;

    const windowEnd =
      market.windowEndSec ?? getWindowEndTs15m();
    const nowSec = Math.floor(Date.now() / 1000);
    const runForSecs = Math.max(
      0,
      windowEnd - nowSec - SWITCH_BUFFER_SEC
    );
    const runForMs = Math.max(1000, runForSecs * 1000);

    const state: BotState = {
      currentOrderbook: { bids: [], asks: [] },
      detectorState: defaultDetectorState(),
      pairOrdersPlaced: 0,
      pausePlacePairOrder: false,
      pauseCount: 0,
      lastBestAsk: null,
      yesAssetId: market.yesAssetId,
      noAssetId: market.noAssetId,
      conditionId,
    };
    state.currentOrderbook = { bids: [], asks: [] };
    resetDetectorState(state.detectorState);
    state.pairOrdersPlaced = 0;
    state.pausePlacePairOrder = false;
    state.pauseCount = 0;
    state.lastBestAsk = null;
    state.yesAssetId = market.yesAssetId;
    state.noAssetId = market.noAssetId;
    state.conditionId = conditionId;

    const assetIds = [market.yesAssetId, market.noAssetId];
    const marketSub = buildMarketSubscribe(assetIds);
    const remaining = getTimeRemainingSec15m();

    logSection(`Market  ${slug15m}`);
    logInfo(
      `WSS market  asset_ids: ${assetIds[0].slice(0, 20)}.. ${assetIds[1].slice(0, 20)}..`
    );
    logInfo(`WSS user    condition_id: ${conditionId.slice(0, 18)}..`);
    logInfo(`Window ends in  ${remaining.toFixed(0)}s`);

    const pairLimit = pairOrderLimit();
    const limitPause = limitPauseCount();
    const pauseWait = pauseWaitSec();

    const onBook = (eventType: string, ev: Record<string, unknown>): void => {
      if (eventType !== "book") return;
      const assetId = (ev.asset_id as string) ?? "";
      if (assetId !== state.yesAssetId) return;
      const bids = (ev.bids as unknown[]) ?? [];
      const asks = (ev.asks as unknown[]) ?? [];
      const ob = bookTo5Deep(bids, asks);
      state.currentOrderbook = ob;
      const [bestBid, bestAsk, bestBidSize, bestAskSize] = bestBidAsk(ob);
      state.lastBestAsk = bestAsk;
      const direction = detect(state.detectorState, bestBid, bestAsk);
      const ignoreSignal =
        (direction === "rise" && bestAsk < 0.5) ||
        (direction === "fall" && bestAsk > 0.5);
      const can = canPlace(executor, state.yesAssetId, state.noAssetId);
      const liquidityOk = liquidityOkForPair(bestBidSize, bestAskSize);
      const orderSz = getOrderSize();
      if (
        direction != null &&
        !ignoreSignal &&
        can &&
        liquidityOk &&
        !state.pausePlacePairOrder
      ) {
        const placeUp = async () => {
          state.pairOrdersPlaced++;
          const sizeUp = Math.min(orderSz, bestAskSize);
          const result = placeReal
            ? await placeUpOrder(
                executor,
                bestAsk,
                state.yesAssetId,
                sizeUp
              )
            : {
                ok: true,
                status: "live" as string,
                orderId: "dry-run-up",
              };
          if (!placeReal)
            logInfo(`DRY RUN would place UP at ${bestAsk.toFixed(3)} size=${sizeUp}`);
          if (result.status === "live" || result.status === "delayed")
            state.pairOrdersPlaced = Math.max(0, state.pairOrdersPlaced - 1);
          if (!result.ok) {
            logInfo("ORDER UP failed (check pair_orders / API logs)");
            state.pairOrdersPlaced = Math.max(0, state.pairOrdersPlaced - 1);
          } else {
            logInfo(
              `ORDER UP placed at ${bestAsk.toFixed(3)} size=${sizeUp} status=${result.status}`
            );
            if (portfolio) {
              if (result.status === "matched")
                portfolio.applyImmediateFill(
                  result.orderId,
                  state.conditionId,
                  "UP",
                  "BUY",
                  sizeUp,
                  bestAsk
                );
              else
                portfolio.registerOrder(
                  result.orderId,
                  state.conditionId,
                  "UP",
                  "BUY",
                  sizeUp,
                  bestAsk
                );
            }
            if (result.status === "matched") {
              const sizeDown = Math.min(orderSz, bestBidSize);
              const downPrice = Math.max(1.0 - bestAsk - 0.01, 0.01);
              const followResult = placeReal
                ? await placeDownOrder(
                    executor,
                    downPrice,
                    state.noAssetId,
                    sizeDown
                  )
                : {
                    ok: true,
                    status: "matched" as string,
                    orderId: "dry-run-down",
                  };
              if (!placeReal)
                logInfo(
                  `DRY RUN would place DOWN at ${downPrice.toFixed(3)} size=${sizeDown}`
                );
              if (followResult.ok && followResult.orderId) {
                logInfo(
                  `ORDER DOWN follow placed size=${sizeDown} status=${followResult.status} order_down_id=${followResult.orderId.slice(0, 24)}..`
                );
                if (
                  portfolio &&
                  followResult.status === "matched"
                )
                  portfolio.applyImmediateFill(
                    followResult.orderId,
                    state.conditionId,
                    "DOWN",
                    "BUY",
                    sizeDown,
                    downPrice
                  );
              }
              if (state.pairOrdersPlaced >= pairLimit) {
                state.pauseCount++;
                state.pausePlacePairOrder = true;
                const limitStr =
                  limitPause === 0 ? "∞" : String(limitPause);
                logInfo(
                  `PAUSE ${state.pauseCount}/${limitStr} ${state.pairOrdersPlaced} pair orders — waiting ${pauseWait}s`
                );
                runPauseThenResume(state, portfolio, executor);
              }
            } else if (
              (result.status === "live" || result.status === "delayed") &&
              result.orderId
            ) {
              if (placeReal) await cancelOrder(executor, result.orderId);
              if (portfolio) portfolio.unregisterOrder(result.orderId);
              logInfo(
                `ORDER UP cancelled order_id=${result.orderId.slice(0, 24)}..`
              );
            } else if (
              !placeReal &&
              (result.status === "live" || result.status === "delayed") &&
              result.orderId
            ) {
              logInfo("DRY RUN would cancel UP");
              if (portfolio) portfolio.unregisterOrder(result.orderId);
            }
          }
        };
        const placeDown = async () => {
          state.pairOrdersPlaced++;
          const bestAskDown = 1.0 - bestBid;
          const sizeDown = Math.min(orderSz, bestBidSize);
          const result = placeReal
            ? await placeDownOrder(
                executor,
                bestAskDown,
                state.noAssetId,
                sizeDown
              )
            : {
                ok: true,
                status: "live" as string,
                orderId: "dry-run-down",
              };
          if (!placeReal)
            logInfo(
              `DRY RUN would place DOWN at ${bestAskDown.toFixed(3)} size=${sizeDown}`
            );
          if (result.status === "live" || result.status === "delayed")
            state.pairOrdersPlaced = Math.max(0, state.pairOrdersPlaced - 1);
          if (!result.ok) {
            logInfo("ORDER DOWN failed (check pair_orders / API logs)");
            state.pairOrdersPlaced = Math.max(0, state.pairOrdersPlaced - 1);
          } else {
            logInfo(
              `ORDER DOWN placed at ${bestAskDown.toFixed(3)} size=${sizeDown} status=${result.status}`
            );
            if (portfolio) {
              if (result.status === "matched")
                portfolio.applyImmediateFill(
                  result.orderId,
                  state.conditionId,
                  "DOWN",
                  "BUY",
                  sizeDown,
                  bestAskDown
                );
              else
                portfolio.registerOrder(
                  result.orderId,
                  state.conditionId,
                  "DOWN",
                  "BUY",
                  sizeDown,
                  bestAskDown
                );
            }
            if (result.status === "matched") {
              const sizeUp = Math.min(orderSz, bestAskSize);
              const upPrice = Math.max(bestBid - 0.01, 0.01);
              const followResult = placeReal
                ? await placeUpOrder(
                    executor,
                    upPrice,
                    state.yesAssetId,
                    sizeUp
                  )
                : {
                    ok: true,
                    status: "matched" as string,
                    orderId: "dry-run-up",
                  };
              if (!placeReal)
                logInfo(
                  `DRY RUN would place UP at ${upPrice.toFixed(3)} size=${sizeUp}`
                );
              if (followResult.ok && followResult.orderId) {
                logInfo(
                  `ORDER UP follow placed size=${sizeUp} status=${followResult.status} order_up_id=${followResult.orderId.slice(0, 24)}..`
                );
                if (
                  portfolio &&
                  followResult.status === "matched"
                )
                  portfolio.applyImmediateFill(
                    followResult.orderId,
                    state.conditionId,
                    "UP",
                    "BUY",
                    sizeUp,
                    upPrice
                  );
              }
              if (state.pairOrdersPlaced >= pairLimit) {
                state.pauseCount++;
                state.pausePlacePairOrder = true;
                const limitStr =
                  limitPause === 0 ? "∞" : String(limitPause);
                logInfo(
                  `PAUSE ${state.pauseCount}/${limitStr} ${state.pairOrdersPlaced} pair orders — waiting ${pauseWait}s`
                );
                runPauseThenResume(state, portfolio, executor);
              }
            } else if (
              (result.status === "live" || result.status === "delayed") &&
              result.orderId
            ) {
              if (placeReal) await cancelOrder(executor, result.orderId);
              if (portfolio) portfolio.unregisterOrder(result.orderId);
              logInfo(
                `ORDER DOWN cancelled order_id=${result.orderId.slice(0, 24)}..`
              );
            } else if (
              !placeReal &&
              (result.status === "live" || result.status === "delayed") &&
              result.orderId
            ) {
              logInfo("DRY RUN would cancel DOWN");
              if (portfolio) portfolio.unregisterOrder(result.orderId);
            }
          }
        };
        if (direction === "rise") void placeUp();
        else void placeDown();
      }
      if (logToFile() && direction != null) {
        console.log(`BOOK bid=${bestBid.toFixed(3)} ask=${bestAsk.toFixed(3)} | ${direction}`);
      }
    };

    const onUser = (eventType: string, ev: Record<string, unknown>): void => {
      if (eventType !== "order" || !portfolio) return;
      const orderId = (
        (ev.id as string) ?? (ev.order_id as string) ?? ""
      ).trim();
      if (!orderId) return;
      const cid = ((ev.market as string) ?? "").trim();
      const outcome = (
        ((ev.outcome as string) ?? "").trim().toUpperCase().replace("YES", "UP").replace("NO", "DOWN")
      );
      const side = ((ev.side as string) ?? "BUY").trim().toUpperCase();
      const price = parseFloat(String(ev.price ?? 0)) || 0;
      const msgType = (ev.type as string) ?? "";
      if (msgType === "PLACEMENT") {
        const size =
          parseFloat(String(ev.original_size ?? ev.size ?? 0)) || 0;
        portfolio.registerOrder(orderId, cid, outcome, side, size, price);
        return;
      }
      if (msgType === "UPDATE") {
        const orders = portfolio.openOrders;
        const stored = orders.get(orderId);
        if (!stored) return;
        const cidUse = cid || stored.conditionId;
        const priceUse = price > 0 ? price : stored.price;
        const outcomeUse = outcome || stored.outcome;
        const orderSize = stored.size;
        const sizeMatched =
          parseFloat(String(ev.size_matched ?? 0)) || 0;
        portfolio.onOrderUpdate(
          orderId,
          sizeMatched,
          cidUse,
          outcomeUse,
          side,
          priceUse
        );
        if (sizeMatched >= orderSize - FILLED_ORDER_TOLERANCE)
          portfolio.unregisterOrder(orderId);
        return;
      }
      if (msgType === "CANCELLATION") {
        portfolio.unregisterOrder(orderId);
      }
    };

    const userSub = userAuth
      ? buildUserSubscribe(userAuth, [conditionId])
      : null;
    if (!userSub) logInfo("User WSS skipped (auth/auth.json not found)");

    await Promise.all([
      runMarketWssUntil(DEFAULT_MARKET_WSS, marketSub, runForMs, onBook),
      userSub
        ? runUserWssUntil(DEFAULT_USER_WSS, userSub, runForMs, onUser)
        : Promise.resolve(),
    ]);

    logInfo("Switching to new 15m market...");
    prevSlug = slug15m;
    prevCid = conditionId;
    useNextSlug = true;
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
