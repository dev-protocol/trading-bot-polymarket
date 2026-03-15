//! Portfolio: cash, positions per market (UP/DOWN), open orders. Same logic as v2_python utils/portfolio_state.

use std::collections::HashMap;
use std::sync::RwLock;

const ROUND: i32 = 4;

fn round_f64(v: f64) -> f64 {
    let m = 10_f64.powi(ROUND);
    (v * m).round() / m
}

fn norm_outcome(outcome: &str) -> Option<&'static str> {
    let o = outcome.trim().to_uppercase();
    if o == "YES" || o == "UP" {
        return Some("UP");
    }
    if o == "NO" || o == "DOWN" {
        return Some("DOWN");
    }
    None
}

#[derive(Clone, Debug, Default)]
pub struct Position {
    pub shares: f64,
    pub cost: f64,
}

fn empty_position() -> Position {
    Position {
        shares: 0.0,
        cost: 0.0,
    }
}

#[derive(Clone, Debug)]
pub struct OpenOrder {
    pub condition_id: String,
    pub outcome: String,
    pub side: String,
    pub size: f64,
    pub price: f64,
}

pub struct PortfolioState {
    pub cash_balance: RwLock<f64>,
    pub realized_pnl: RwLock<f64>,
    pub positions: RwLock<HashMap<String, HashMap<String, Position>>>, // cid -> UP/DOWN -> Position
    pub open_orders: RwLock<HashMap<String, OpenOrder>>,
    applied: RwLock<HashMap<String, f64>>, // order_id -> size already applied
}

impl PortfolioState {
    pub fn new(starting_cash: f64) -> Self {
        Self {
            cash_balance: RwLock::new(round_f64(starting_cash)),
            realized_pnl: RwLock::new(0.0),
            positions: RwLock::new(HashMap::new()),
            open_orders: RwLock::new(HashMap::new()),
            applied: RwLock::new(HashMap::new()),
        }
    }

    fn market(&self, cid: &str) -> HashMap<String, Position> {
        let mut positions = self.positions.write().unwrap();
        positions
            .entry(cid.to_string())
            .or_insert_with(|| {
                let mut m = HashMap::new();
                m.insert("UP".to_string(), empty_position());
                m.insert("DOWN".to_string(), empty_position());
                m
            })
            .clone()
    }

    fn apply_fill(
        &self,
        cid: &str,
        outcome: &str,
        side: &str,
        size: f64,
        price: f64,
    ) {
        let outcome = match norm_outcome(outcome) {
            Some(o) => o.to_string(),
            None => return,
        };
        let size = round_f64(size);
        let price = round_f64(price);
        let value = round_f64(size * price);
        let side = side.trim().to_uppercase();

        let mut positions = self.positions.write().unwrap();
        let market = positions
            .entry(cid.to_string())
            .or_insert_with(|| {
                let mut m = HashMap::new();
                m.insert("UP".to_string(), empty_position());
                m.insert("DOWN".to_string(), empty_position());
                m
            });
        let pos = market.get_mut(&outcome).unwrap();

        if side == "BUY" {
            let current_cash = *self.cash_balance.read().unwrap();
            pos.shares = round_f64(pos.shares + size);
            pos.cost = round_f64(pos.cost + value);
            drop(positions);
            *self.cash_balance.write().unwrap() = round_f64(current_cash - value);
            return;
        }
        // SELL
        if pos.shares <= 0.0 {
            return;
        }
        let sell_size = round_f64(size.min(pos.shares));
        if sell_size <= 0.0 {
            return;
        }
        let avg = if pos.shares > 0.0 {
            pos.cost / pos.shares
        } else {
            0.0
        };
        let sell_value = round_f64(sell_size * price);
        let current_cash = *self.cash_balance.read().unwrap();
        let current_pnl = *self.realized_pnl.read().unwrap();
        pos.shares = round_f64(pos.shares - sell_size);
        pos.cost = round_f64(pos.cost - avg * sell_size);
        if pos.shares <= 0.0 {
            pos.shares = 0.0;
            pos.cost = 0.0;
        }
        drop(positions);
        *self.cash_balance.write().unwrap() = round_f64(current_cash + sell_value);
        *self.realized_pnl.write().unwrap() =
            round_f64(current_pnl + sell_size * (price - avg));
    }

    /// Apply fill when place-order response is MATCHED.
    pub fn apply_immediate_fill(
        &self,
        order_id: &str,
        condition_id: &str,
        outcome: &str,
        side: &str,
        size: f64,
        price: f64,
    ) {
        let out = match norm_outcome(outcome) {
            Some(o) => o,
            None => return,
        };
        let mut applied = self.applied.write().unwrap();
        let prev = applied.get(order_id).copied().unwrap_or(0.0);
        let delta = round_f64(size) - prev;
        if delta <= 0.0 {
            return;
        }
        drop(applied);
        self.apply_fill(condition_id, out, side, delta, price);
        self.applied.write().unwrap().insert(order_id.to_string(), round_f64(size));
    }

    /// Apply incremental fill from user WSS UPDATE.
    pub fn on_order_update(
        &self,
        order_id: &str,
        size_matched: f64,
        condition_id: &str,
        outcome: &str,
        side: &str,
        price: f64,
    ) -> bool {
        let total = size_matched;
        let mut applied = self.applied.write().unwrap();
        let prev = applied.get(order_id).copied().unwrap_or(0.0);
        let delta = round_f64(total - prev);
        if delta <= 0.0 {
            return true;
        }
        let outcome = match norm_outcome(outcome) {
            Some(o) => o.to_string(),
            None => return false,
        };
        drop(applied);
        self.apply_fill(condition_id, &outcome, side, delta, price);
        self.applied.write().unwrap().insert(order_id.to_string(), total);
        true
    }

    pub fn register_order(
        &self,
        order_id: &str,
        condition_id: &str,
        outcome: &str,
        side: &str,
        size: f64,
        price: f64,
    ) {
        let outcome = (outcome.trim().to_uppercase())
            .replace("YES", "UP")
            .replace("NO", "DOWN");
        let outcome_str = if outcome.is_empty() { "UP".to_string() } else { outcome };
        let side = side.trim().to_uppercase();
        let side_str = if side.is_empty() { "BUY".to_string() } else { side };
        self.open_orders.write().unwrap().insert(
            order_id.to_string(),
            OpenOrder {
                condition_id: condition_id.to_string(),
                outcome: outcome_str,
                side: side_str,
                size,
                price,
            },
        );
    }

    pub fn unregister_order(&self, order_id: &str) {
        self.open_orders.write().unwrap().remove(order_id);
    }

    /// (qty_up, qty_down, cost_up, cost_down)
    pub fn get_position(&self, condition_id: &str) -> (f64, f64, f64, f64) {
        let positions = self.positions.read().unwrap();
        let market = match positions.get(condition_id) {
            Some(m) => m,
            None => return (0.0, 0.0, 0.0, 0.0),
        };
        let empty = empty_position();
        let u = market.get("UP").unwrap_or(&empty);
        let d = market.get("DOWN").unwrap_or(&empty);
        (u.shares, d.shares, u.cost, d.cost)
    }
}
