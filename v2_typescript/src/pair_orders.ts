import {
  orderSize,
  rebalanceOrderSize,
  minLiquiditySize,
  maxLiquiditySize,
} from "./config.js";
import type { OrderExecutor } from "./clob_client.js";

export function getOrderSize(): number {
  return orderSize();
}

export function getRebalanceOrderSize(): number {
  return rebalanceOrderSize() ?? orderSize();
}

export function getMinLiquiditySize(): number {
  return minLiquiditySize();
}

export function getMaxLiquiditySize(): number {
  return maxLiquiditySize();
}

export function liquidityOkForPair(
  bestBidSize: number,
  bestAskSize: number
): boolean {
  const min = getMinLiquiditySize();
  const max = getMaxLiquiditySize();
  return (
    bestBidSize >= min &&
    bestBidSize <= max &&
    bestAskSize >= min &&
    bestAskSize <= max
  );
}

export async function placeUpOrder(
  executor: OrderExecutor | null,
  price: number,
  yesAssetId: string,
  size?: number
): Promise<{ ok: boolean; status: string; orderId: string }> {
  const sz = size ?? getOrderSize();
  if (sz <= 0) return { ok: false, status: "", orderId: "" };
  return placeOrderImpl(executor, yesAssetId, price, sz, "UP");
}

export async function placeDownOrder(
  executor: OrderExecutor | null,
  price: number,
  noAssetId: string,
  size?: number
): Promise<{ ok: boolean; status: string; orderId: string }> {
  const sz = size ?? getOrderSize();
  if (sz <= 0) return { ok: false, status: "", orderId: "" };
  return placeOrderImpl(executor, noAssetId, price, sz, "DOWN");
}

async function placeOrderImpl(
  executor: OrderExecutor | null,
  tokenId: string,
  price: number,
  size: number,
  side: string
): Promise<{ ok: boolean; status: string; orderId: string }> {
  if (executor) return executor.placeOrder(tokenId, price, size, side);
  console.log(`DRY RUN would place ${side} at ${price.toFixed(3)} size=${size}`);
  return {
    ok: true,
    status: "stub",
    orderId: `dry-run-${side.toLowerCase()}`,
  };
}

export async function cancelOrder(
  executor: OrderExecutor | null,
  orderId: string
): Promise<boolean> {
  if (!orderId) return false;
  if (executor) return executor.cancel(orderId);
  console.log("DRY RUN would cancel order_id=" + orderId.slice(0, 24) + "..");
  return true;
}
