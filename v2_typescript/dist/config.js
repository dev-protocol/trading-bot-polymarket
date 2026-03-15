const DEFAULT_CLOB_HOST = "https://clob.polymarket.com";
export const DEFAULT_MARKET_WSS = "wss://ws-subscriptions-clob.polymarket.com/ws/market";
export const DEFAULT_USER_WSS = "wss://ws-subscriptions-clob.polymarket.com/ws/user";
export const GAMMA_BASE = "https://gamma-api.polymarket.com";
export const CHAIN_ID = 137;
export function privateKey() {
    const v = process.env.PRIVATE_KEY?.trim();
    return v && v.length > 0 ? v : undefined;
}
export function polyRpcUrl() {
    return process.env.POLY_RPC_URL ?? process.env.POLYGON_RPC_URL ?? "https://polygon-rpc.com";
}
export function clobHost() {
    return process.env.CLOB_HOST ?? DEFAULT_CLOB_HOST;
}
export function dryRun() {
    const v = (process.env.DRY_RUN ?? "0").trim().toLowerCase();
    return v === "1" || v === "true" || v === "yes";
}
export function signatureType() {
    const s = process.env.SIGNATURE_TYPE?.trim();
    if (s === undefined)
        return 0;
    const n = parseInt(s, 10);
    return isNaN(n) ? 0 : n;
}
export function funderAddress() {
    const v = process.env.FUNDER_ADDRESS?.trim() ?? process.env.POLY_FUNDER?.trim();
    return v && v.length > 0 ? v : undefined;
}
export function orderSize() {
    const s = process.env.ORDER_SIZE?.trim();
    if (s === undefined)
        return 5;
    const n = parseFloat(s);
    return isNaN(n) ? 5 : n;
}
export function minLiquiditySize() {
    const s = process.env.MIN_LIQUIDITY_SIZE?.trim();
    if (s === undefined)
        return 30;
    const n = parseFloat(s);
    return isNaN(n) ? 30 : n;
}
export function maxLiquiditySize() {
    const s = process.env.MAX_LIQUIDITY_SIZE?.trim();
    if (s === undefined)
        return 10000;
    const n = parseFloat(s);
    return isNaN(n) ? 10000 : n;
}
export function pauseWaitSec() {
    const s = process.env.PAUSE_WAIT_SEC?.trim();
    if (s === undefined)
        return 5;
    const n = parseFloat(s);
    return isNaN(n) ? 5 : n;
}
export function pairOrderLimit() {
    const s = process.env.PAIR_ORDER_LIMIT?.trim();
    if (s === undefined)
        return 4;
    const n = parseInt(s, 10);
    return isNaN(n) ? 4 : n;
}
export function limitPauseCount() {
    const s = process.env.LIMIT_PAUSE_COUNT?.trim();
    if (s === undefined)
        return 0;
    const n = parseInt(s, 10);
    return isNaN(n) ? 0 : n;
}
export function autoRedeemDelaySec() {
    const s = process.env.AUTO_REDEEM_DELAY_SEC?.trim();
    if (s === undefined)
        return 120;
    const n = parseFloat(s);
    return isNaN(n) ? 120 : n;
}
export function rebalanceSize() {
    const s = process.env.REBALANCE_SIZE?.trim();
    if (s === undefined)
        return 0;
    const n = parseFloat(s);
    return isNaN(n) ? 0 : n;
}
export function rebalanceOrderSize() {
    const s = process.env.REBALANCE_ORDER_SIZE?.trim();
    if (s === undefined || s.length === 0)
        return undefined;
    const n = parseFloat(s);
    return isNaN(n) ? undefined : n;
}
export function logToFile() {
    const v = (process.env.LOG_TO_FILE ?? "0").trim().toLowerCase();
    return v === "1" || v === "true" || v === "yes";
}
export function startingCash() {
    const s = process.env.STARTING_CASH?.trim();
    if (s === undefined)
        return 0;
    const n = parseFloat(s);
    return isNaN(n) ? 0 : n;
}
