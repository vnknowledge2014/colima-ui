# Phase 7 (Falco) — fix verification

Verified 2026-08-13 against `plans/reports/from-code-reviewer-to-cook-260813-0907-falco-phase7-review.md`.
No code modified. Line numbers are current `dev`.

## Verdict per finding

| Prior | State | Note |
|---|---|---|
| C1 alert never reaches user | **Fixed** | verified end to end |
| C2 whole-log replay | **Fixed, with a new hole** | see N1 |
| H1 one alert per event | **Fixed, with caveats** | see N2, N3 |
| H2 detect() every 5 s | **Fixed, wiring incomplete** | see N4 |
| H3 offset drift | **Fixed; its test does not test it** | see N5 |
| H4 blank container id | **Fixed on one side only** | see N6 |
| M1 unquoted path | Fixed | `shell_quote` at `falco_bridge.rs:138`, applied at `:144` and `:367`; `'` escaping correct |
| M2 inline comments | Mostly fixed | see N7 |
| M4 KB `grep -c` | Fixed | removed in all four locales |

### C1 — verified end to end
`alerts.rs:487-499` publishes `containerId:""`, `containerName:""`, `imageRef`.
`AlertMetric::as_str` (`alerts.rs:79-88`) emits exactly the six strings the widened
union lists (`alertNotifications.ts:39-45`) — no drift. `FalcoPriority` ordinals
(`falco_bridge.rs:394-403`, Debug=0 … Emergency=7) line up with `PRIORITY_NAMES`
(`alertNotifications.ts:51-60`), so `priority as i64 as f64` at `falco_bridge.rs:862`
renders as a name. No remaining throw on the `format` → `pushNotification` → `osNotify`
path. `FiredAlert` has no other consumer in `src/` (grepped), so widening broke nothing.
Self-heal reads Docker events, not alerts (`self_heal.rs:788-800`) — the empty
`containerId` cannot make it act on the wrong container.

## New / residual defects

### N1 (High) — a re-attach silently discards everything written during the outage
`falco_bridge.rs:769-779` sets `file = None` after 3 failures; `:726-754` then
re-detects and sets `offset = event_file_size(...)`, i.e. the *current* EOF.
Every event Falco wrote during the outage is skipped, permanently, with no log line and
nothing in the UI. The prior review asked for the opposite: *preserve `offset` across a
re-detect that resolves the same path*. Seek-to-EOF is right for a **first** attach only.
Fix: keep the old offset when the re-detected path equals the previous one.

Related, lower: rotation is detected only by `size < offset` (`:358`). A rotate whose
replacement grows past the old offset within one 5 s poll is read from the middle of a
line. No inode/`stat -c %i` check.

### N2 (Medium) — the cooldown trusts a clock the VM controls
`should_alert` (`:877-894`) keys off `ts_ms`, which comes from Falco's own
`evt.time` inside the VM. A VM clock ahead of the host (sleep/resume is the common
case) writes a future `last`; `ts_ms.saturating_sub(*last)` is then 0 for every
subsequent event, and **all** alerts for that (rule, subject) are suppressed until real
time catches up. Out-of-order arrival is harmless (suppressed, correctly), but forward
skew is a silent-alerting failure. Use `chrono::Utc::now().timestamp_millis()` for the
cooldown bookkeeping — it is a rate limit on *notifications*, not on event time.
`parse_time_ms`'s `ts_ms = 0` fallback (prior L4, still open) feeds the same key.

### N3 (Low) — the comment overstates what the count shows
`:824-826` claims "the suppressed count carried in the alert itself so the number is
visible". The `(xN)` only counts duplicates *within one 5 s batch* (`:833-845`);
everything the cooldown drops at `:849` is discarded uncounted. Also, mutating
`rule_name` to `Falco: X (x3)` (`:852-856`) puts a varying string in
`alert_events.rule_name`, so alert history cannot be grouped by rule and the
frontend's title-based collapse never merges these.

The plan's own flood control — a minimum priority to *store*, with dropped events
counted — is still unimplemented; `ingest` stores every priority (`:802-810`).

### N4 (Medium) — `invalidate_detect` is dead code, and the stale window is user-visible
`falco_bridge.rs:256` has zero callers (grepped across `src-tauri/src`). It is `pub`, so
clippy stays quiet, but its doc comment asserts "Called after the user is told to change
something", which is false. Combined with `EventStream.svelte:78` (`if (watching) void
refresh()` — no polling at all while stopped), a user who follows the on-screen
instructions, enables file output and comes back sees the old status until they leave
and re-enter the page. Call `invalidate_detect()` from `falco_set_watching`
(`:916`) and/or from an explicit refresh control, or delete the function.

### N5 (Medium) — the byte-offset test is a phantom test
`falco_bridge.rs:1175-1188` defines a `consume` closure that **reimplements** the
`rposition`/slice logic and asserts against the copy. `read_since` is never called.
Revert `read_since` to `String`/`lines()` and this test still passes. The production
logic at `:373-384` is correct — the test just does not defend it. Extract the
consume step into a named function and call *that* from both.

### N6 (Low) — `MIN_ID_LEN` guards only the Docker-side id
`:631-632` filters `cid.len() >= 8` but never checks `id` (the event's
`container.id`, `:620`). `cid.starts_with(id)` with a 2-char id from a custom rule or
a non-Docker runtime still prefix-matches the first listed container. Apply the same
minimum to `id`.

### N7 (Low) — `strip_inline_comment` needs a space before the `#`
`:231` matches `" #"`. `enabled: true\t# comment` or `enabled: true# comment` are not
stripped, so `enabled` reads `false` and a correctly configured host is reported
`OutputNotConfigured` — the original M2 failure mode, narrowed rather than closed.
Falco's shipped YAML uses a space, so this is unlikely, not impossible.

## Still open from the prior review (not claimed fixed)
M3 (rule count describes a different process), M5 (history hidden unless `ready`),
M6 (`fields_json` write-only), M7 (`falco.event` SSE published, unconsumed),
M8 (comma-joined tags), L1 (20 k anti-join per poll), L2 (`WATCHING`/`SHOULD_WATCH`
race at `:693`/`:783`), L3 (compose label stored in `image_ref`), L4 (`ts_ms = 0`
fallback — now also a cooldown key, see N2), L5 (`falco_triage.rs`,
`EventDetail.svelte` never created; phase incomplete against its own plan).

## Recommended order
1. N1 — preserve offset on same-path re-attach (silent data loss).
2. N2 — cooldown on host clock (silent alert suppression).
3. N5 — make the offset test exercise `read_since`.
4. N4 — wire or delete `invalidate_detect`.
5. N3 / N6 / N7 — cheap.

Status: DONE_WITH_CONCERNS
Summary: C1 is genuinely fixed and verified end to end; C2/H1/H2/H3/H4 and M1/M2/M4
are fixed in substance, but the C2 fix introduces silent event loss on re-attach (N1),
the H1 cooldown is keyed on a VM-controlled clock (N2), and the test added for H3 asserts
against a copy of the logic rather than the function (N5).
Concerns: N1 and N2 are both silent-failure modes on a security screen — no log, no UI
signal. N5 means H3 has no regression protection.
