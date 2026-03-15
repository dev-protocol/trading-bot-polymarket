//! BTC up/down market slug. 15m: btc-updown-15m-{unix_window_start}

pub const WINDOW_15M_SEC: u64 = 900;

pub fn get_slug_15m(use_next_window: bool) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let window_start = (now / WINDOW_15M_SEC) * WINDOW_15M_SEC;
    let start = if use_next_window {
        window_start + WINDOW_15M_SEC
    } else {
        window_start
    };
    format!("btc-updown-15m-{}", start)
}

pub fn get_window_end_ts_15m() -> u64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let start = (now / WINDOW_15M_SEC) * WINDOW_15M_SEC;
    start + WINDOW_15M_SEC
}

pub fn get_time_remaining_sec_15m() -> f64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64();
    let end = get_window_end_ts_15m() as f64;
    (end - now).max(0.0)
}
