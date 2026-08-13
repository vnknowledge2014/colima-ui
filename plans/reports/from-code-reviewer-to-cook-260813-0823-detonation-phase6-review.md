# Detonation (phase 6) review — 2026-08-13

Scope: `src-tauri/src/commands/detonation.rs`, `routes/detonation.rs`, `api_server.rs`,
`lib.rs`, `src/lib/api/detonation.ts`, `DetonationPanel.svelte`, `DetonationTimeline.svelte`,
`src/pages/Security.svelte`, `src/locales/{en,vi,ja,zh}.json`.

Verdict: the docker-context custody requirement (acceptance criterion 5) is **not met**, and
there is a second, larger custody hole via `DOCKER_HOST` that the design did not consider.
Everything else is close, with the frontend unable to reach a running session's Stop button.

## Critical

### C1 — context save/restore is a no-op; global context is left on the detonation VM
`detonation.rs:220-235`. `current_docker_context()` shells out via `helpers::run_cmd`, and
`helpers.rs:104-116` (`build_cmd`) sets `DOCKER_HOST` for every `docker` invocation when
`detect_docker_host()` finds a socket — which is the normal state for a Colima user. With
`DOCKER_HOST` set, `docker context show` prints `default`, not the user's real context.
Verified on this machine:

```
$ docker context show                        -> colima
$ DOCKER_HOST=tcp://127.0.0.1:1 docker context show   -> default
```

Consequences, in order:
1. `previous_context` (line 457) is always `Some("default")`, never the real context.
2. After `colima start` flips the global context to `colima-colimaui-detonate-<id>`,
   `restore_docker_context` re-reads the masked value, sees `default == default`
   (line 229-231) and returns without doing anything.
3. The global context therefore stays on the detonation instance for the whole session, and
   `teardown_profile` then deletes that context — leaving `~/.docker/config.json`
   `currentContext` pointing at a context that no longer exists. The user's `docker` CLI
   outside the app breaks until they run `docker context use colima` by hand.

Fix: read and set the context with a command that has `DOCKER_HOST` removed
(`Command::env_remove("DOCKER_HOST")`), or read `currentContext` from `~/.docker/config.json`
directly. Do not route context custody through `run_cmd`.

### C2 — `DOCKER_HOST` hijack: the app's own docker calls can bind to the detonation VM
`path_util.rs:210-232`. `detect_docker_host()` returns the **first running Colima profile it
finds in `read_dir` order**, uncached, on every call. While a detonation instance is up, that
can be `colimaui-detonate-<id>`. Live callers include `adapters/docker.rs:25` (the bollard
client), `commands/containers.rs:904`, `commands/metrics_collector.rs:398` and every
`run_cmd("docker", …)` in the app. The user's container list can show the detonating sample's
container as their own — precisely the Gate 0 scenario. The implemented context custody does
not touch this path because the hijack happens through the environment, not through the
docker context.

Fix: make `detect_docker_host()` skip profiles starting with the detonation prefix. Note that
this is the one place the prefix must be shared outside `detonation.rs`.

Verified as *not* a problem: `--context` does beat `DOCKER_HOST` for endpoint selection
(`DOCKER_HOST=tcp://127.0.0.1:1 docker --context colima version` reached the colima daemon),
so detonation's own commands do target the throwaway VM.

## High

### H1 — unbounded concurrent sessions racing on a global setting
`detonation.rs:606-641`. `start()` neither caps nor serialises sessions. Two sessions each
create a 2 GB VM and each independently save/restore a *global* docker context; an interleaved
restore can leave the process pointed at the other session's context, which is then deleted.
Serialise instance creation (single-slot mutex or a `Semaphore` of 1) and reject a second
concurrent session.

### H2 — no deadline on create/pull, and cancel is ignored before the observe loop
`helpers.rs:139-152` (`run_cmd` uses `Command::output()` with no timeout). The session deadline
(line 533) starts only after `docker run` succeeds, and `is_cancelled` is only consulted inside
the loop. A hung `colima start` or a slow `docker pull` leaves the session in `Preparing`
forever, `cancel()` flips a flag nobody reads, and the user has a VM they cannot stop from the
UI. Check `is_cancelled` after create and after pull, and bound both steps.

### H3 — the UI loses the handle on a running session
`DetonationPanel.svelte:37,97`. Session state is component-local and `onDestroy` clears the
poll; leaving the Runtime tab and returning yields `session = null` with the VM still running,
so the Stop button is gone. `detonationApi.list()` exists (`detonation.ts:114`) and is never
called — dead code that is also the fix. This also lets the user start a second concurrent
session (see H1). The requirement that the stop control is always reachable is only half met:
it is outside `ProGate`, but it is not reachable after a remount.

### H4 — sweep/teardown can restore to a context it just deleted
`detonation.rs:252,265` and `sweep_orphans` (273-289). After a crash the current context *is*
the detonation one; teardown captures it as `previous`, deletes the instance, then tries
`docker context use colima-colimaui-detonate-<id>`, which fails and leaves the setting broken.
Never restore a name derived from a detonation profile — fall back to the real one.
(Today C1 masks this by always reporting `default`; fixing C1 without fixing H4 makes it live.)

## Medium

- **M1** `DetonationTimeline.svelte:41` — keyed `{event.tsMs + event.detail}`. The log drain
  (`detonation.rs:590-595`) pushes up to 200 events in a tight loop, so `ts_ms` collides, and
  duplicate log lines are ordinary. Duplicate keys in a Svelte 5 `{#each}` are an error. Use a
  server-assigned sequence number.
- **M2** `detonation.rs:501-521` — no `--` separator before `config.image`. An image string
  starting with `-` is parsed by docker as a flag, with `cmd_override` tokens supplying the
  image. In practice `docker pull` rejects it first, so this is not exploitable today, but the
  argv assertion in the test at line 747 asserts on a hand-written literal, not on the argv the
  production code builds, so it would not catch a regression. Add `--` and validate the ref.
- **M3** `detonation.rs:647-656` — `assert_path_within(dir, dir.join(filename))` uses the
  caller-supplied directory as its own base. It stops `..` inside `filename` and nothing else;
  an authenticated HTTP caller picks any writable directory on the machine. Confine to an app
  export directory.
- **M4** `detonation.rs:547-552` — the timeline says "container killed" at the deadline, but
  nothing kills it until `docker rm -f` inside teardown, after the log drain. The timeline
  misreports the moment.
- **M5** `SESSIONS` is never pruned (`detonation.rs:168`); sessions and their timelines
  accumulate for the app's lifetime.
- **M6** `cancel()` (line 370) returns `true` for an already-terminal session, so the HTTP
  contract reports a stop that did not happen.

## Test quality

Four of the eight tests re-implement the logic inline instead of calling it:
`the_run_command_never_mounts_a_host_path…` (747) asserts on a literal argv vector,
`the_sweep_only_recognises_its_own_prefix` (732) re-writes the filter,
`timeouts_are_clamped_to_the_ceiling` (765) asserts `min`, and
`docker_top_output_yields_the_command_column` (773) re-writes the parser. None of them can
fail if `run_session_blocking`, `sweep_orphans` or `sample_processes` change. Extracting
`run_args(ctx, image, override)`, a `parse_profiles(listing)` and the command-column parser
into functions the tests call would make them real.

## Verified as correct

- Teardown is on every early-return path and after every loop exit (478-483, 488-493, 523-528,
  598).
- `teardown_profile` prefix guard (247-250) covers the `colima delete --force` blast radius.
- `--network none`, `--memory 512m`, `--pids-limit 256`; no `-v`/`--mount`/`--privileged`
  anywhere in the module.
- Entitlement is checked in `start()` (607), not at registration; cancel is ungated on both the
  Tauri and HTTP paths.
- All timeline text passes `crate::redact::redact` in `push_event` (306).
- `SESSIONS` locks are all short and none is held across a blocking command or an await.
- HTTP routes sit behind the bearer-token middleware (`api_server.rs:311`).
- Locale wording: en/vi/ja/zh all carry "observe behaviour in an isolated environment" and the
  explicit "not malware-analysis isolation" caveat; no forbidden claim in any of the four.
- `lib.rs:157` startup ordering is fine — the sweep is spawned off-thread and touches nothing
  the later initialisers depend on.

## Recommended order

1. C1, C2 (blocking — they corrupt the docker state the rest of the app relies on).
2. H4 with C1, H2, H3, H1.
3. M1 (visible crash), then M2/M3, then the rest.
