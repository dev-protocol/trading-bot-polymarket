const WINDOW_15M_SEC = 900;

export function getSlug15m(useNextWindow: boolean): string {
  const now = Math.floor(Date.now() / 1000);
  const windowStart = Math.floor(now / WINDOW_15M_SEC) * WINDOW_15M_SEC;
  const start = useNextWindow ? windowStart + WINDOW_15M_SEC : windowStart;
  return `btc-updown-15m-${start}`;
}

export function getWindowEndTs15m(): number {
  const now = Math.floor(Date.now() / 1000);
  const start = Math.floor(now / WINDOW_15M_SEC) * WINDOW_15M_SEC;
  return start + WINDOW_15M_SEC;
}

export function getTimeRemainingSec15m(): number {
  const now = Date.now() / 1000;
  const end = getWindowEndTs15m();
  return Math.max(0, end - now);
}
