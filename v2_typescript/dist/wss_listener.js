import WebSocket from "ws";
const PING_INTERVAL_MS = 10_000;
export function buildMarketSubscribe(assetIds) {
    return JSON.stringify({
        assets_ids: assetIds,
        type: "market",
        custom_feature_enabled: true,
    });
}
export function buildUserSubscribe(auth, markets) {
    return JSON.stringify({
        auth: {
            apiKey: auth.apiKey,
            secret: auth.secret,
            passphrase: auth.passphrase,
        },
        markets,
        type: "user",
    });
}
function parseMessage(data, onMessage) {
    if (data.trim() === "PONG")
        return;
    try {
        const v = JSON.parse(data);
        if (Array.isArray(v)) {
            for (const ev of v) {
                const obj = ev;
                const eventType = obj.event_type ?? "";
                onMessage(eventType, obj);
            }
        }
        else if (v && typeof v === "object") {
            const obj = v;
            const eventType = obj.event_type ?? "";
            onMessage(eventType, obj);
        }
    }
    catch {
        //
    }
}
export function runMarketWssUntil(url, subscribePayload, runForMs, onMessage) {
    return new Promise((resolve, reject) => {
        const ws = new WebSocket(url);
        const deadline = setTimeout(() => {
            ws.close();
            resolve();
        }, runForMs);
        ws.on("open", () => {
            console.log("Market WSS connected:", url);
            ws.send(subscribePayload);
        });
        ws.on("message", (data) => {
            parseMessage(data.toString(), onMessage);
        });
        ws.on("close", () => {
            clearTimeout(deadline);
            resolve();
        });
        ws.on("error", (err) => {
            clearTimeout(deadline);
            reject(err);
        });
        const pingInterval = setInterval(() => {
            if (ws.readyState === WebSocket.OPEN)
                ws.send("PING");
        }, PING_INTERVAL_MS);
        ws.on("close", () => clearInterval(pingInterval));
    });
}
export function runUserWssUntil(url, subscribePayload, runForMs, onMessage) {
    return new Promise((resolve, reject) => {
        const ws = new WebSocket(url);
        const deadline = setTimeout(() => {
            ws.close();
            resolve();
        }, runForMs);
        ws.on("open", () => {
            console.log("User WSS connected:", url);
            ws.send(subscribePayload);
        });
        ws.on("message", (data) => {
            parseMessage(data.toString(), onMessage);
        });
        ws.on("close", () => {
            clearTimeout(deadline);
            resolve();
        });
        ws.on("error", (err) => {
            clearTimeout(deadline);
            reject(err);
        });
        const pingInterval = setInterval(() => {
            if (ws.readyState === WebSocket.OPEN)
                ws.send("PING");
        }, PING_INTERVAL_MS);
        ws.on("close", () => clearInterval(pingInterval));
    });
}
