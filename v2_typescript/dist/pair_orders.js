import { orderSize, rebalanceOrderSize, minLiquiditySize, maxLiquiditySize, } from "./config.js";
export function getOrderSize() {
    return orderSize();
}
export function getRebalanceOrderSize() {
    return rebalanceOrderSize() ?? orderSize();
}
export function getMinLiquiditySize() {
    return minLiquiditySize();
}
export function getMaxLiquiditySize() {
    return maxLiquiditySize();
}
export function liquidityOkForPair(bestBidSize, bestAskSize) {
    const min = getMinLiquiditySize();
    const max = getMaxLiquiditySize();
    return (bestBidSize >= min &&
        bestBidSize <= max &&
        bestAskSize >= min &&
        bestAskSize <= max);
}
export async function placeUpOrder(executor, price, yesAssetId, size) {
    const sz = size ?? getOrderSize();
    if (sz <= 0)
        return { ok: false, status: "", orderId: "" };
    return placeOrderImpl(executor, yesAssetId, price, sz, "UP");
}
export async function placeDownOrder(executor, price, noAssetId, size) {
    const sz = size ?? getOrderSize();
    if (sz <= 0)
        return { ok: false, status: "", orderId: "" };
    return placeOrderImpl(executor, noAssetId, price, sz, "DOWN");
}
async function placeOrderImpl(executor, tokenId, price, size, side) {
    if (executor)
        return executor.placeOrder(tokenId, price, size, side);
    console.log(`DRY RUN would place ${side} at ${price.toFixed(3)} size=${size}`);
    return {
        ok: true,
        status: "stub",
        orderId: `dry-run-${side.toLowerCase()}`,
    };
}
export async function cancelOrder(executor, orderId) {
    if (!orderId)
        return false;
    if (executor)
        return executor.cancel(orderId);
    console.log("DRY RUN would cancel order_id=" + orderId.slice(0, 24) + "..");
    return true;
}
