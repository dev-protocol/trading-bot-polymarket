//! Detector: returns "rise"/"fall"/None from 5-deep orderbook.

pub struct DetectorState {
    pub prev_best_bid: Option<f64>,
    pub prev_best_ask: Option<f64>,
}

impl Default for DetectorState {
    fn default() -> Self {
        Self {
            prev_best_bid: None,
            prev_best_ask: None,
        }
    }
}

impl DetectorState {
    pub fn reset(&mut self) {
        self.prev_best_bid = None;
        self.prev_best_ask = None;
    }
}

/// Best bid/ask from 5-deep ob. Bids/asks are sorted; best bid = last bid, best ask = last ask.
pub fn best_bid_ask(ob: &OrderBook5) -> (f64, f64, f64, f64) {
    let (best_bid, best_bid_size) = ob.bids.last().map(|p| (p.price, p.size)).unwrap_or((0.0, 0.0));
    let (best_ask, best_ask_size) = ob.asks.last().map(|p| (p.price, p.size)).unwrap_or((0.0, 0.0));
    (best_bid, best_ask, best_bid_size, best_ask_size)
}

#[derive(Clone, Debug, Default)]
pub struct PriceLevel {
    pub price: f64,
    pub size: f64,
}

#[derive(Clone, Debug, Default)]
pub struct OrderBook5 {
    pub bids: Vec<PriceLevel>,
    pub asks: Vec<PriceLevel>,
}

/// Build 5-deep book from raw WSS book message (bids/asks arrays, best = last).
pub fn book_to_5_deep(bids: &[serde_json::Value], asks: &[serde_json::Value]) -> OrderBook5 {
    const DEPTH: usize = 5;
    let parse = |arr: &[serde_json::Value]| {
        let start = arr.len().saturating_sub(DEPTH);
        arr[start..]
            .iter()
            .filter_map(|p| {
                let price = p.get("price")?.as_str().and_then(|s| s.parse().ok())?;
                let size = p.get("size")?.as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                Some(PriceLevel { price, size })
            })
            .collect()
    };
    OrderBook5 {
        bids: parse(bids),
        asks: parse(asks),
    }
}

/// Simple direction: if best_ask dropped vs prev -> "rise"; if best_ask rose -> "fall". Otherwise None.
pub fn detect(state: &mut DetectorState, best_bid: f64, best_ask: f64) -> Option<&'static str> {
    let direction = match (state.prev_best_ask, state.prev_best_bid) {
        (Some(prev_ask), _) if best_ask < prev_ask && (prev_ask - best_ask) > 0.001 => Some("rise"),
        (Some(prev_ask), _) if best_ask > prev_ask && (best_ask - prev_ask) > 0.001 => Some("fall"),
        _ => None,
    };
    state.prev_best_bid = Some(best_bid);
    state.prev_best_ask = Some(best_ask);
    direction
}
