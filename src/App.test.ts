import { expect, it } from "vitest";
// Raw source, not the compiled component: the assertion is about how many times
// the tag appears in the markup, which the compiled output no longer shows.
import appSource from "./App.svelte?raw";

// Each ToastContainer keeps its own `toasts` state and subscribes to the global
// toast bus independently, so a second instance renders every toast twice. That
// is a structural constraint, not runtime behaviour — checking the source is
// enough and avoids mounting the whole component tree.
it("renders exactly one ToastContainer", () => {
  expect(appSource.match(/<ToastContainer \/>/g)).toHaveLength(1);
});
