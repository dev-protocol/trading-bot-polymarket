export function defaultDetectorState() {
    return { prevBestBid: null, prevBestAsk: null };
}
export function resetDetectorState(state) {
    state.prevBestBid = null;
    state.prevBestAsk = null;
}
export function bestBidAsk(ob) {
    const bestBid = ob.bids.length > 0 ? ob.bids[ob.bids.length - 1] : { price: 0, size: 0 };
    const bestAsk = ob.asks.length > 0 ? ob.asks[ob.asks.length - 1] : { price: 0, size: 0 };
    return [bestBid.price, bestAsk.price, bestBid.size, bestAsk.size];
}
export function bookTo5Deep(bids, asks) {
    const DEPTH = 5;
    const parse = (arr) => {
        const start = Math.max(0, arr.length - DEPTH);
        return arr.slice(start).flatMap((p) => {
            const o = p;
            const price = typeof o?.price === "string" ? parseFloat(o.price) : NaN;
            const size = typeof o?.size === "string" ? parseFloat(o.size) : 0;
            if (isNaN(price))
                return [];
            return [{ price, size }];
        });
    };
    return { bids: parse(bids), asks: parse(asks) };
}
export function detect(state, bestBid, bestAsk) {
    const { prevBestAsk } = state;
    let direction = null;
    if (prevBestAsk != null) {
        if (bestAsk < prevBestAsk && prevBestAsk - bestAsk > 0.001)
            direction = "rise";
        else if (bestAsk > prevBestAsk && bestAsk - prevBestAsk > 0.001)
            direction = "fall";
    }
    state.prevBestBid = bestBid;
    state.prevBestAsk = bestAsk;
    return direction;
}
