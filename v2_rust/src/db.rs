//! db/markets.json: track slug, condition_id, redeemed. Same logic as v2_python main.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MarketEntry {
    pub slug: String,
    pub condition_id: String,
    #[serde(default)]
    pub info: Option<String>,
    #[serde(default)]
    pub redeemed: bool,
}

fn db_dir() -> std::path::PathBuf {
    std::path::PathBuf::from("db")
}

fn markets_path() -> std::path::PathBuf {
    db_dir().join("markets.json")
}

pub fn add_market_to_db(slug: &str, condition_id: &str, info: Option<&str>) {
    let path = markets_path();
    let mut data: Vec<MarketEntry> = if path.exists() {
        serde_json::from_slice(&std::fs::read(&path).unwrap_or_default()).unwrap_or_default()
    } else {
        Vec::new()
    };
    if !data.iter().any(|e| e.condition_id == condition_id || e.slug == slug) {
        data.push(MarketEntry {
            slug: slug.to_string(),
            condition_id: condition_id.to_string(),
            info: info.map(String::from),
            redeemed: false,
        });
    } else {
        for e in &mut data {
            if e.condition_id == condition_id || e.slug == slug {
                e.slug = slug.to_string();
                e.condition_id = condition_id.to_string();
                e.info = info.map(String::from);
                break;
            }
        }
    }
    let _ = std::fs::create_dir_all(db_dir());
    let _ = std::fs::write(path, serde_json::to_string_pretty(&data).unwrap_or_default());
}

pub fn mark_market_redeemed(condition_id: &str) {
    let path = markets_path();
    let Ok(bytes) = std::fs::read(&path) else { return };
    let Ok(mut data) = serde_json::from_slice::<Vec<MarketEntry>>(&bytes) else { return };
    for e in &mut data {
        if e.condition_id == condition_id {
            e.redeemed = true;
            break;
        }
    }
    let _ = std::fs::write(path, serde_json::to_string_pretty(&data).unwrap_or_default());
}

pub fn unredeemed_markets() -> Vec<(String, String)> {
    let path = markets_path();
    let Ok(bytes) = std::fs::read(&path) else { return Vec::new() };
    let Ok(data) = serde_json::from_slice::<Vec<MarketEntry>>(&bytes) else { return Vec::new() };
    data.into_iter()
        .filter(|e| !e.condition_id.is_empty() && !e.redeemed)
        .map(|e| (e.slug, e.condition_id))
        .collect()
}