const ROUND = 4;
function roundF64(v) {
    const m = Math.pow(10, ROUND);
    return Math.round(v * m) / m;
}
function normOutcome(outcome) {
    const o = outcome.trim().toUpperCase();
    if (o === "YES" || o === "UP")
        return "UP";
    if (o === "NO" || o === "DOWN")
        return "DOWN";
    return null;
}
export class PortfolioState {
    cashBalance = 0;
    realizedPnl = 0;
    positions = new Map();
    openOrders = new Map();
    applied = new Map();
    constructor(startingCash) {
        this.cashBalance = roundF64(startingCash);
    }
    market(cid) {
        let m = this.positions.get(cid);
        if (!m) {
            m = new Map([
                ["UP", { shares: 0, cost: 0 }],
                ["DOWN", { shares: 0, cost: 0 }],
            ]);
            this.positions.set(cid, m);
        }
        return m;
    }
    applyFill(cid, outcome, side, size, price) {
        const out = normOutcome(outcome);
        if (!out)
            return;
        const sz = roundF64(size);
        const pr = roundF64(price);
        const value = roundF64(sz * pr);
        const s = side.trim().toUpperCase();
        const market = this.market(cid);
        const pos = market.get(out);
        if (s === "BUY") {
            pos.shares = roundF64(pos.shares + sz);
            pos.cost = roundF64(pos.cost + value);
            this.cashBalance = roundF64(this.cashBalance - value);
            return;
        }
        if (pos.shares <= 0)
            return;
        const sellSize = roundF64(Math.min(size, pos.shares));
        if (sellSize <= 0)
            return;
        const avg = pos.shares > 0 ? pos.cost / pos.shares : 0;
        const sellValue = roundF64(sellSize * price);
        pos.shares = roundF64(pos.shares - sellSize);
        pos.cost = roundF64(pos.cost - (avg * sellSize));
        if (pos.shares <= 0) {
            pos.shares = 0;
            pos.cost = 0;
        }
        this.cashBalance = roundF64(this.cashBalance + sellValue);
        this.realizedPnl = roundF64(this.realizedPnl + sellSize * (price - avg));
    }
    applyImmediateFill(orderId, conditionId, outcome, side, size, price) {
        const out = normOutcome(outcome);
        if (!out)
            return;
        const prev = this.applied.get(orderId) ?? 0;
        const delta = roundF64(size) - prev;
        if (delta <= 0)
            return;
        this.applyFill(conditionId, out, side, delta, price);
        this.applied.set(orderId, roundF64(size));
    }
    onOrderUpdate(orderId, sizeMatched, conditionId, outcome, side, price) {
        const prev = this.applied.get(orderId) ?? 0;
        const delta = roundF64(sizeMatched - prev);
        if (delta <= 0)
            return true;
        const out = normOutcome(outcome);
        if (!out)
            return false;
        this.applyFill(conditionId, out, side, delta, price);
        this.applied.set(orderId, sizeMatched);
        return true;
    }
    registerOrder(orderId, conditionId, outcome, side, size, price) {
        const outcomeStr = outcome.trim().toUpperCase().replace("YES", "UP").replace("NO", "DOWN") ||
            "UP";
        const sideStr = side.trim().toUpperCase() || "BUY";
        this.openOrders.set(orderId, {
            conditionId,
            outcome: outcomeStr,
            side: sideStr,
            size,
            price,
        });
    }
    unregisterOrder(orderId) {
        this.openOrders.delete(orderId);
    }
    getPosition(conditionId) {
        const m = this.positions.get(conditionId);
        if (!m)
            return [0, 0, 0, 0];
        const u = m.get("UP") ?? { shares: 0, cost: 0 };
        const d = m.get("DOWN") ?? { shares: 0, cost: 0 };
        return [u.shares, d.shares, u.cost, d.cost];
    }
}
