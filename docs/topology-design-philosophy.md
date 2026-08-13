# Topology — Design Philosophy

Design rules for the Docker topology graph page (`src/pages/Topology.svelte` and
`src/components/topology/`). Read this before adding a node kind, an edge kind,
or **any new action** on a topology node. The rules below exist because three
people (or one person three months apart, which is worse) each "helpfully"
re-implemented the same concept differently. This document is the single
source of truth; the code is the implementation of it.

## 1. The data model

One backend round-trip, one shape (`src/lib/api/topology.ts`):

- **Node kinds** — `container`, `network`, `volume`, `project`, `image`,
  `service`. A `service` is a compose service **declared but never created**:
  it exists only in a compose file. It is *not* a stopped container.
- **Node statuses** — `running`, `stopped`, `unhealthy`, `notCreated`, `none`.
  `notCreated` is distinct from `stopped`: a stopped container exists and can be
  started; a not-created service has nothing to start. Never collapse the two —
  the user's next action differs.
- **Edge kinds** — `network`, `volume`, `project`, `image`, `dependsOn`.
  `dependsOn` means "before" (ordering), every other edge means "attached to".
- **Warnings** — subsystems the backend could not list. The graph must say
  "partial", never silently pretend an attachment does not exist.

## 2. Canonical kind order (do not reorder casually)

One order everywhere: **`container, network, volume, project, image, service`**.

- `Topology.svelte` — the `KINDS` rail array
- `GraphCanvas.svelte` — the `KIND_ORDER` symbol array
- `NodeDetail.svelte` — the `related` grouping order

Rationale: the container is the primary subject; its attachments follow
(network, volume, project, image); `service` is a container-shaped afterthought
and comes last. A resource list that reshuffles between polls is unusable as a
navigation target — the rail, the legend, and the detail panel must present the
same hierarchy or users (and AI agents) will think they are different things.

## 3. The rail is the legend

The filter chips on the page *are* the legend — each chip carries the exact
icon and colour the graph uses for that kind. There is no separate explanatory
band. Consequences:

- A **kind label is a noun, never a status**. The `service` kind chip reads
  "Services", not "Not created" — "Not created" is a *status* and belongs only
  in the status legend. When a kind's label and a status label would collide,
  rename the kind label, never the status (statuses describe state across all
  kinds; a kind label names one resource type).
- A filtered-out kind is **dimmed, never hidden**: the count must stay visible,
  or "0 volumes" and "volumes turned off" look identical.

## 4. Status is shape first, colour second

Running = filled dot, stopped = hollow dot, unhealthy = triangle, not created =
dashed ring. Colour alone would hide running-vs-unhealthy from anyone with
red-green colour blindness — the one distinction on this screen that matters
most. The status legend mirrors the graph shapes exactly.

## 5. Graph rendering

- Layout is delegated to ELK (`src/lib/topology-elk.ts`), deterministic: the
  same graph always lands at the same coordinates, so a poll that only flips a
  status must not re-lay-out under the user's cursor (`graphKey` gates re-layout
  on shape change only).
- Networks owning at least one container become **containment boxes** (containers
  drawn *inside*); empty networks stay ordinary nodes.
- A box's title bar (icon, name, count) must always fit inside the box. ELK
  sizes a box from its containers — never from the name — so the renderer
  truncates the name to the available width (`titleLabel` in
  `topology-elk.ts`) and keeps the full name in a `<title>` tooltip. Do not
  reintroduce a min-width layout option: elkjs ignores minimum sizes on
  compound nodes, and a spacer child inflates the box height with an empty row.
- Every node is a chip of uniform height (icon, label, status marker on one
  line). The label lives *inside* the box; what ELK reserves is what gets painted.
- The canvas owns its viewport: wheel-zoom about the cursor, drag-to-pan,
  fit/reset buttons in a floating cluster. The page never re-fits after the
  first layout (that would throw away a viewport the user adjusted).

## 6. Action design philosophy (read before adding an action)

### 6.1 Where actions live

Actions live in the **NodeDetail panel** (`NodeDetail.svelte`), never on the
graph chips. The graph is for *understanding* the topology; the panel is for
*acting* on one node. Chips stay a single row of icon + label + status — adding
buttons to them would make the graph unreadable at 50 nodes.

### 6.2 One code path, always

An action calls the **same API method the list page uses** — there is no second
code path. `NodeDetail.runAction` calls `dockerApi.startContainer` /
`stopContainer` / `restartContainer`, the exact functions the Containers page
uses. If a new action needs backend support, add it to the API layer once and
call it from both places. Duplicated action logic is how the graph and the list
pages drift apart.

### 6.3 Kind → action matrix

| Kind | Actions today | Rules for tomorrow |
|---|---|---|
| `container` | Start / Stop / Restart / Logs | More lifecycle actions here (e.g. remove) only with the same single-path rule |
| `network` / `volume` / `image` / `project` | "Open in <page>" only | An action is acceptable only when it is *meaningful on this exact node* and safe without navigation |
| `service` | nothing | A not-created service has no actions — starting it is a compose operation, not a container operation |

Rules of thumb:

- **No action on a node that cannot act.** A not-created service gets no Start
  button; a network gets no Start button. A dead button is a lie.
- **Destructive actions** (remove, delete volume, prune) must be confirmable and
  must reuse the app's confirmation dialog — never a bare click.
- **Read-only views** (logs, inspect) are actions too: they go through the same
  busy/disabled machinery so the UI never double-fires.

### 6.4 Action state conventions

- `busy` is a single string key (the running action); every action button is
  `disabled={busy !== null}` so two actions can't race.
- **Always pair `.btn` with a variant** (`btn-ghost`, `btn-primary`,
  `btn-danger`). The bare `.btn` base class defines only layout and font — it
  sets no background or text colour, so a bare button falls back to the browser's
  default grey-on-light style and glares against the dark theme. Every action
  button must carry a variant class.
- Success → `globalToast("success", …)`; failure → `globalToast("error", …)`.
- After a mutating action, call `onChanged()` so the graph refreshes from the
  same source the list pages use.
- Selection-dependent state must reset on selection change: stale logs from a
  previously selected container are actively misleading, so `logs` clears
  whenever `node.id` changes.

### 6.5 Checklist for adding a new action

1. Add the API method once (or reuse the list page's).
2. Put the button in NodeDetail, not on the chip.
3. Show it only for the kinds where it is meaningful (`{#if node.kind === …}`).
4. Wire `busy`/`disabled`, success/error toasts, and `onChanged()`.
5. Reset any selection-dependent state.
6. Add the i18n key to all four locales (`topology.*`).
7. Update this document if the action contradicts or extends a rule.

## 7. Cross-page handoff contract

- `viewInTopology(kind, name)` (`src/lib/topology-link.ts`) jumps to the graph
  with one resource focused: it sets `uiState.focusResource` *then*
  `uiState.currentPage` — the destination reads the request as it mounts.
- `consumeFocus(page)` takes the pending focus for a page and **clears it**, so
  returning to that page later opens clean instead of re-focusing something
  stale. A focus abandoned mid-navigation must not fire on whatever page the
  user lands on next.
- The graph node ids are `kind:name`; list pages key on the resource's own
  identity. `NodeDetail.openInOwningPage` converts between the two — a service
  has no page of its own, so it opens its *project* in Compose.

## 8. Layout constants

- NodeDetail panel: fixed `320px`, right side, `flex-shrink: 0`; the canvas-wrap
  beside it is `flex: 1; min-width: 0` so the graph shrinks instead of sliding
  under the panel.
- Chip geometry lives in `topology-elk.ts` (`CHIP_HEIGHT`, label metrics) and is
  mirrored by `GraphCanvas` drawing constants — if you change one, change both,
  or ELK reserves space the renderer does not paint.
