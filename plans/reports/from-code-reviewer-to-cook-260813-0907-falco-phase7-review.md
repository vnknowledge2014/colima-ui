# Phase 7 (Falco frontend) — production-readiness review

Reviewed 2026-08-13 against `plans/260812-1013-security-posture/phase-07-falco-frontend.md`
(Gate 0 constraints + "Đính chính" table). No code was modified.

Verdict: the *shape* of the phase is right — nothing here detects anything, identity
comes from the version string, `RunningWithoutRules` is its own state, storage is
redacted and capped, alerts reuse `alerts.rs`. The defects below are in the plumbing:
the alert path does not actually reach the user, and the reader's offset/ingest logic
replays and duplicates.

---

## Critical

### C1. A Critical Falco event produces **no** notification — the handler throws and the throw is swallowed
- `src-tauri/src/commands/alerts.rs:481-492` — `emit_security` publishes
  `{ruleId, ruleName, imageRef, value, threshold, metric, ts}`. No `containerId`,
  no `containerName`.
- `src/lib/alertNotifications.ts:35` — `const container = alert.containerName || alert.containerId.slice(0, 12);`
  → `containerName` is `undefined` (falsy) → `alert.containerId` is `undefined` →
  `TypeError: Cannot read properties of undefined (reading 'slice')`.
- `src/lib/alertNotifications.ts:83-89` — the call is inside `try { handle(...) } catch {}`,
  so the alert is dropped silently. No in-app notification, no OS notification.

Effect: phase 7 success criterion *"Event Critical → đúng một alert, qua đường alert đã có"*
fails end to end. This is pre-existing from phase 5 (`security_score` /
`new_vulnerabilities` alerts are equally invisible), but phase 7 is the phase that
depends on it, and phase 7 cannot be called done while it holds.

Also note `FiredAlert.metric` (`alertNotifications.ts:31`) is typed
`"cpu_pct" | "mem_pct" | "mem_bytes"` — three of the six real variants are missing, so
TypeScript never flagged this. Even once the crash is fixed, `format()` renders a
runtime event as `"5.0%, over the 5 threshold"` (raw enum ordinals through the percent
branch) — meaningless to the user.

### C2. Reader replays the entire event file, and re-replays it after any transient error
- `src-tauri/src/commands/falco_bridge.rs:603` — `offset` starts at `0`.
- `:626` — every re-detect sets `offset = 0` again.
- `:634-639` — **any** failed read (SSH hiccup, VM sleeping, file momentarily absent)
  sets `file = None`, which routes back through `:620` → `offset = 0`.

Effects, all on first "Start reading" and after every transient failure:
1. `read_since` pulls the whole `events.json` through `colima ssh` into one `String`
   (`:273`). A week-old Falco log is tens/hundreds of MB in RAM in one allocation.
2. Every historical event is re-inserted (no dedup key on `falco_events` —
   `security_history.rs:121-133` has no unique constraint), so history duplicates
   on every hiccup.
3. Every historical Critical/Alert/Emergency event re-fires an alert with its
   *original* `ts_ms`. Once C1 is fixed this becomes a notification storm about
   things that happened days ago.

Correct behaviour for a first attach is to start at the current file size, and to
preserve `offset` across a re-detect that resolves the same path.

---

## High

### H1. One alert per event, against the explicit contract of the function being called
`falco_bridge.rs:678-697` loops over the batch and calls `emit_security` per event.
`alerts.rs:454-456` documents the opposite: *"Callers are responsible for grouping:
one alert per image per scan, never one per vulnerability … hundreds of notifications
is the same as none."* A single `kubectl exec`/CI burst can put dozens of Critical
events in one 5 s poll. There is no cooldown, no dedup by rule, no cap.

The plan's own risk row (*"Event flood làm nghẽn UI/DB → ngưỡng priority tối thiểu để
lưu … đếm số bị bỏ"*) is unimplemented on the backend: **all** priorities are stored
(`ingest` at `:646` passes everything), there is no minimum-priority store threshold,
and nothing counts or reports drops. The UI filter (`EventStream.svelte:39-49`) filters
what was already fetched; it is not the flood control the plan asked for.

### H2. `detect()` runs every 5 seconds per open tab, and it is expensive
`EventStream.svelte:77-79` polls `refresh()` every 5 s, and `refresh()` (`:55`) calls
`falcoApi.state()` → `detect()` → one `colima ssh` running
`falco --version; pgrep; falco -L -o json_output=true; cat falco.yaml config.d/*.yaml`
(`falco_bridge.rs:194-198`). `falco -L -o json_output=true` dumps the **entire** rule
document (hundreds of KB on a stock pack) and `cat` adds the 64 KB config — every 5 s,
forever, with SSH setup each time. `detect()`'s own doc-comment justifies its cost with
*"this runs behind a user opening a tab"*; that is no longer true. There is no cache
(compare `helpers::TimedCache`, already used elsewhere for exactly this).

Same loop, backend side: `falco_bridge.rs:620-627` — when `event_file` is `None`
(the common "user has not enabled file output" state) the reader runs `detect()` and
`continue`s every 5 s indefinitely. Two SSH+rule-dumps per 5 s while nothing can
possibly be read.

### H3. Byte-offset arithmetic is not byte-accurate
`read_since` (`falco_bridge.rs:277-287`) computes `consumed` from `chunk.len()`, but
`chunk` comes from `run_cmd`, which returns `String::from_utf8_lossy(&output.stdout)`
(`helpers.rs:151`). Consequences:

- **Lossy replacement drifts the offset forward.** Falco fields (`fd.name`,
  `proc.cmdline`) carry raw filesystem/argv bytes; Falco does not guarantee valid
  UTF-8 in JSON string escapes for them. Each invalid byte becomes a 3-byte U+FFFD,
  so `chunk.len()` > bytes actually read. `offset` overshoots permanently and the
  first line of every subsequent read starts mid-record — silently dropped by
  `parse_event`, i.e. one lost event per poll, forever, with no log.
- **`lines()` strips `\r`, `chunk.len()` counts it.** Sign is opposite there: if the
  transport ever CRLF-translates (a pty-allocating `colima ssh`, which also breaks the
  `stat` parse at `:259-262`), `consumed` under-counts by one per line and events are
  re-read and re-stored.
- Any stray stdout from `colima ssh`/`sudo` (a banner, a `sudo` lecture) is counted
  as file bytes.

`lines().last()` for the partial tail is correct only under the assumption
`chunk.len() == bytes read from the file`; that assumption is not held by the transport.
Read the raw bytes (e.g. `base64`/`Command::output` without lossy conversion) or track
the offset from a value the VM reports rather than from the local string length.
The multi-byte-UTF-8 question in the brief is fine *within* valid UTF-8 (`len()` is
bytes on both sides); the problem is the invalid-byte path.

### H4. `correlate()` can bind an event to the wrong container
`falco_bridge.rs:525-530`:
```rust
.is_some_and(|cid| cid.starts_with(id) || id.starts_with(cid))
```
- If any entry in the `docker ps` JSON has `"ID": ""` (missing/blank, which
  `list_containers_cli` does not guarantee against), `id.starts_with("")` is `true`
  and **every** event correlates to that entry — wrong compose project/service on a
  security screen.
- `find` returns the first prefix match. With a short id on either side, an ambiguous
  prefix silently picks one. Require a minimum comparison length (Falco emits 12 hex
  chars) and reject blank ids.

---

## Medium

### M1. Unquoted path interpolated into `sudo bash -c`
`falco_bridge.rs:259` and `:273` build `stat -c %s {path}` and `tail -c +N {path}`
with the path taken from parsed `falco.yaml`/`config.d/*.yaml`. The source is a
root-owned file, so this is not a privilege-boundary crossing, but a path containing a
space, glob, or `;` breaks the read or executes. Quote it (`'{path}'` with escaping)
or pass it via a positional argument.

### M2. `parse_event_file` misreads real config lines
`falco_bridge.rs:169-177` compares `v.trim() == "true"` and takes the rest of the line
as the filename. Real YAML with inline comments:
- `enabled: true   # send alerts to a file` → parsed as **not** enabled → status
  downgraded to `OutputNotConfigured` and the reader never starts, on a correctly
  configured host.
- `filename: /var/log/falco/events.json  # json` → the path includes `# json`.
- Falco's shipped default is the relative `./events.txt`; with `enabled: true` that is
  tailed relative to `sudo`'s cwd, i.e. the wrong file, reported as `Ready`.

The tests at `:898-940` only cover comments on their own lines. Strip trailing
`#`-comments (outside quotes) and reject non-absolute paths.

### M3. Rule count describes a *different process* than the one running
`detect()` derives `rule_count` from a fresh `falco -L` (`:197`), while `running`
comes from `pgrep -x falco` (`:196`). If the running unit was started with a different
config or `-r` (which is exactly what the shipped KB article tells users to do —
`resources/kb/en/install-falco.md:70`, `sudo falco -r /etc/falco/falco_rules.yaml`),
the two disagree. The user can be shown `Ready, 25 rules` while the live engine has
zero, or `RunningWithoutRules` while it has rules. Since "how many rules is the engine
loading" is the safety claim this whole enum exists to make, either read it from the
running process (`falco --version`-style status endpoint / unit args) or state the
limitation in the UI copy.

### M4. Shipped KB article contradicts the module's own rule-counting rule
`src-tauri/resources/kb/en/install-falco.md:54` —
`sudo falco -L -o json_output=true | grep -c '"name"'` counts `"name"` across
`rules`, `macros` **and** `lists`. That is precisely the miscount
`parse_rule_count`'s doc-comment (`falco_bridge.rs:128-133`) warns about: it prints a
healthy-looking number for an engine with zero rules. The `python3` variant two lines
below is correct — drop the `grep -c` one. (Check the vi/ja/zh copies too.)

### M5. Stored history becomes unreachable the moment Falco degrades
`falco.ts:84-86` / `EventStream.svelte:191` gate the whole list on `status === "ready"`.
Stop Falco (or lose file output) and 7 days of retained events vanish from the UI —
including the events that might explain why it was stopped. Show history with a banner
instead of hiding it.

### M6. `fields_json` is written but has no reader
`security_history.rs:186` redacts and stores the full `output_fields`;
`StoredFalcoEvent` (`:211-225`) and the `SELECT` (`:232-234`) do not return it, and no
consumer exists. Either surface it in the detail view (the plan's `EventDetail.svelte`,
which was not created) or stop storing it — it is the largest column in the table.

### M7. `falco.event` SSE topic is published and never consumed
`falco_bridge.rs:679-682` publishes per event; nothing in `src/` subscribes (the UI
polls every 5 s instead). Wire the stream or delete the publish; a half-built pipeline
reads as working realtime to the next maintainer.

### M8. Tag round-trip is lossy
`security_history.rs:182` joins tags with `,`, `:248` splits on `,`. Falco tags are
user-authored strings; one containing a comma comes back as two tags. Store JSON.

---

## Low

- **L1** `security_history.rs:198-204` — the `id NOT IN (SELECT … LIMIT 20000)`
  cap runs on every 5 s batch; a 20 k-row anti-join per poll. Do it periodically, or
  only when a cheap `COUNT(*)` exceeds the cap.
- **L2** `falco_bridge.rs:590` / `:649` — interleaving `set_watching(false)` with an
  immediate `set_watching(true)` can leave `WATCHING=false` while `SHOULD_WATCH=true`
  and no reader alive (`swap` sees `true`, then the dying loop stores `false`).
  Self-heals on the next toggle; a generation counter or a `compare_exchange` on exit
  would close it.
- **L3** `falco_bridge.rs:690` passes the container label as `image_ref`. The alert
  list renders that column as an image reference.
- **L4** `parse_time_ms` (`:460-467`) — `evt.time` nanosecond division is correct
  (verified by the fixture assertion at `:752`), but an unparseable clock falls back
  to `ts_ms = 0`, which sorts the event to the bottom of a `ts_ms DESC` list and makes
  it immediately eligible for retention deletion. `now_ms` would be the honest default.
- **L5** Plan gaps: `commands/falco_triage.rs` (`explain()`, plan step 7 and the
  Architecture block) and `src/components/security/EventDetail.svelte` were not
  created. Phase is not complete against its own plan.

## Verified as honored

Gate-0 constraints 1 (VM-only detection; identity from the `Falco version:` line —
`falco_bridge.rs:119-126`, negatively tested against the Fastly banner at `:873`),
2 (`RunningWithoutRules` is a distinct state, surfaced as a warning, and all four
locales say plainly that an empty list does not mean nothing happened — en/vi/ja/zh
`falco.norules_body` checked, no softening, none implies ColimaUI detects),
3 (`rules` array only, `:134-139`, tested against a doc carrying `macros`/`lists`),
4 (no detection logic; `ALERT_AT` is a display/notification threshold, not a verdict),
5 (`entitled_now_cached()` inside the loop at `:613`, stop path ungated at `:585-594`),
6 (`redact` applied to `output` and `fields_json`, `security_history.rs:180,186`),
7 (single alert path via `emit_security` — modulo H1),
8 (7-day + 20 k-row retention).
`falco.*` locale keys: 24/24 present in all four files, placeholders intact.
Routes sit on the same authenticated router as the rest (`api_server.rs:161-163`).
No `unwrap`/`expect`/slicing reachable from Falco output in non-test code.

---

Status: DONE_WITH_CONCERNS
Summary: The architecture honours every Gate-0 constraint, but the alert path never
reaches the user (C1), and the reader replays whole log files and duplicates events on
any transient error (C2).
Concerns: C1, C2, H1 and H2 are blocking for a phase whose success criteria are
"one alert per critical event" and "no runaway background work". H3/H4 are correctness
bugs the current tests cannot see, because they exercise the parser, not the transport.
