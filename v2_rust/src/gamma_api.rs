//! Fetch market by slug from Polymarket Gamma API.

use anyhow::Result;

use crate::config::GAMMA_BASE;

#[derive(Clone, Debug)]
pub struct MarketInfo {
    pub condition_id: String,
    pub yes_asset_id: String,
    pub no_asset_id: String,
    pub slug: String,
    pub window_end_sec: Option<u64>,
}

pub async fn fetch_market_for_slug(
    client: &reqwest::Client,
    slug: &str,
) -> Result<Option<MarketInfo>> {
    let url = format!(
        "{}/events?limit=10&slug={}&active=true&closed=false",
        GAMMA_BASE,
        urlencoding::encode(slug)
    );
    let data: serde_json::Value = client.get(&url).send().await?.json().await?;
    let events = data
        .as_array()
        .map(|a| a.as_slice())
        .unwrap_or(&[]);
    for ev in events {
        let markets = ev
            .get("markets")
            .and_then(|m| m.as_array())
            .map(|a| a.as_slice())
            .unwrap_or(&[]);
        for m in markets {
            let cid = m
                .get("conditionId")
                .or_else(|| m.get("condition_id"))
                .and_then(|v| v.as_str())
                .map(str::to_string);
            let cid = match cid {
                Some(s) if !s.is_empty() => s,
                _ => continue,
            };
            let tokens: Vec<String> = m
                .get("clobTokenIds")
                .or_else(|| m.get("clob_token_ids"))
                .and_then(|t| {
                    if let Some(arr) = t.as_array() {
                        Some(
                            arr.iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect(),
                        )
                    } else if let Some(s) = t.as_str() {
                        serde_json::from_str(s).ok()
                    } else {
                        None
                    }
                })
                .unwrap_or_default();
            if tokens.len() < 2 {
                continue;
            }
            let outcomes: Vec<String> = m
                .get("outcomes")
                .and_then(|o| {
                    if let Some(arr) = o.as_array() {
                        Some(
                            arr.iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect(),
                        )
                    } else if let Some(s) = o.as_str() {
                        serde_json::from_str(s).ok()
                    } else {
                        None
                    }
                })
                .unwrap_or_default();
            let mut yes_asset = tokens.get(0).cloned().unwrap_or_default();
            let mut no_asset = tokens.get(1).cloned().unwrap_or_default();
            for (i, out) in outcomes.iter().enumerate() {
                let out_lower = out.to_lowercase();
                if out_lower.contains("up") || out_lower == "yes" {
                    if let Some(t) = tokens.get(i) {
                        yes_asset = t.clone();
                    }
                } else if out_lower.contains("down") || out_lower == "no" {
                    if let Some(t) = tokens.get(i) {
                        no_asset = t.clone();
                    }
                }
            }
            if yes_asset.is_empty() {
                yes_asset = tokens[0].clone();
            }
            if no_asset.is_empty() {
                no_asset = tokens[1].clone();
            }
            let slug_str = m
                .get("slug")
                .or_else(|| m.get("questionId"))
                .and_then(|v| v.as_str())
                .unwrap_or(slug)
                .to_string();
            return Ok(Some(MarketInfo {
                condition_id: cid,
                yes_asset_id: yes_asset,
                no_asset_id: no_asset,
                slug: slug_str,
                window_end_sec: None,
            }));
        }
    }
    Ok(None)
}
