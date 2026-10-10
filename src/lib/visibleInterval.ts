/**
 * setInterval that skips a tick while the window is hidden.
 *
 * Every poller here hits the Docker/Colima socket, so a minimised window or a
 * background tab was still paying full price for data nobody could see. The
 * timer keeps running rather than being torn down and rebuilt: the browser
 * already throttles background timers, and a skipped tick costs nothing.
 *
 * Returns the same clear function shape a caller would write by hand, so it
 * drops straight into an `onMount`/`$effect` teardown.
 */
export function setVisibleInterval(fn: () => void, ms: number): () => void {
  const id = setInterval(() => {
    if (document.visibilityState === "visible") fn();
  }, ms);
  return () => clearInterval(id);
}
