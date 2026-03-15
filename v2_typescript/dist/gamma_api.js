import { GAMMA_BASE } from "./config.js";
export async function fetchMarketForSlug(slug) {
    const url = `${GAMMA_BASE}/events?limit=10&slug=${encodeURIComponent(slug)}&active=true&closed=false`;
    const res = await fetch(url);
    const data = (await res.json());
    if (!Array.isArray(data))
        return null;
    for (const ev of data) {
        const markets = ev.markets;
        const arr = Array.isArray(markets) ? markets : [];
        for (const m of arr) {
            const row = m;
            const cid = (row.conditionId ?? row.condition_id);
            if (!cid || typeof cid !== "string" || cid.length === 0)
                continue;
            let tokens = [];
            const t = row.clobTokenIds ?? row.clob_token_ids;
            if (Array.isArray(t)) {
                tokens = t.filter((x) => typeof x === "string");
            }
            else if (typeof t === "string") {
                try {
                    tokens = JSON.parse(t);
                }
                catch {
                    continue;
                }
            }
            if (tokens.length < 2)
                continue;
            let outcomes = [];
            const o = row.outcomes;
            if (Array.isArray(o)) {
                outcomes = o.filter((x) => typeof x === "string");
            }
            else if (typeof o === "string") {
                try {
                    outcomes = JSON.parse(o);
                }
                catch {
                    outcomes = [];
                }
            }
            let yesAsset = tokens[0] ?? "";
            let noAsset = tokens[1] ?? "";
            for (let i = 0; i < outcomes.length; i++) {
                const out = (outcomes[i] ?? "").toLowerCase();
                if (out.includes("up") || out === "yes")
                    yesAsset = tokens[i] ?? yesAsset;
                else if (out.includes("down") || out === "no")
                    noAsset = tokens[i] ?? noAsset;
            }
            if (!yesAsset)
                yesAsset = tokens[0];
            if (!noAsset)
                noAsset = tokens[1];
            const slugStr = (row.slug ?? row.questionId ?? slug);
            return {
                conditionId: cid,
                yesAssetId: yesAsset,
                noAssetId: noAsset,
                slug: slugStr,
                windowEndSec: undefined,
            };
        }
    }
    return null;
}
