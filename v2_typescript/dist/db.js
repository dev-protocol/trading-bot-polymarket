import { readFileSync, writeFileSync, mkdirSync, existsSync } from "fs";
import { join } from "path";
const DB_DIR = "db";
const MARKETS_PATH = join(DB_DIR, "markets.json");
function loadMarkets() {
    if (!existsSync(MARKETS_PATH))
        return [];
    try {
        const raw = readFileSync(MARKETS_PATH, "utf-8");
        const data = JSON.parse(raw);
        return Array.isArray(data) ? data : [];
    }
    catch {
        return [];
    }
}
function saveMarkets(data) {
    try {
        mkdirSync(DB_DIR, { recursive: true });
        writeFileSync(MARKETS_PATH, JSON.stringify(data, null, 2));
    }
    catch {
        //
    }
}
export function addMarketToDb(slug, conditionId, info) {
    const data = loadMarkets();
    const idx = data.findIndex((e) => e.condition_id === conditionId || e.slug === slug);
    if (idx < 0) {
        data.push({
            slug,
            condition_id: conditionId,
            info,
            redeemed: false,
        });
    }
    else {
        data[idx] = {
            ...data[idx],
            slug,
            condition_id: conditionId,
            info,
        };
    }
    saveMarkets(data);
}
export function markMarketRedeemed(conditionId) {
    const data = loadMarkets();
    const e = data.find((x) => x.condition_id === conditionId);
    if (e)
        e.redeemed = true;
    saveMarkets(data);
}
export function unredeemedMarkets() {
    const data = loadMarkets();
    return data
        .filter((e) => e.condition_id && !e.redeemed)
        .map((e) => [e.slug, e.condition_id]);
}
