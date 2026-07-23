# Astrelis UI Next: incremental retained engine

Status: experimental implementation  
Prototype: `crates/astrelis-ui-next`

## Decision

Astrelis UI Next is a non-generic retained pass engine. It owns element
identity, topology, layout, composition, input targeting, cached local paint
fragments, and incremental accessibility output. It does not own application
state, component policy, design-system roles, commands, or services.

The current `astrelis-ui-core` remains the supported 0.3 implementation. The
prototype is unpublished and is not a compatibility promise.

## Findings from the 0.3 implementation

The current engine correctly avoids work while completely idle and caches
text shaping and many GPU resources. Its remaining scaling boundaries are:

- insert, remove, and reparent mark the mirrored Taffy tree structurally dirty,
  causing a complete rebuild;
- layout creates a whole-tree measurement map and assigns every node's bounds;
- paint and semantic snapshots rebuild complete output trees;
- overlays are rediscovered and sorted during paint and pointer targeting;
- hover changes scan every retained slot to update status;
- the paint renderer recompiles a flat display list into frame vertices and
  indices on every redraw;
- custom widgets can report intrinsic size but cannot own a custom container
  layout algorithm.

The current 1,000-node release baselines on the development machine were:

| Workload | Baseline |
| --- | ---: |
| Resize text-heavy layout | 1.009 ms |
| Change one label and lay out | 0.667 ms |
| Change one label and build display list | 0.735 ms |
| Warm display-list rebuild | 0.067 ms |
| Pointer hit test | 0.013 ms |
| Full semantic snapshot | 0.077 ms |

## Pass model

Each node has generational identity and stores local and subtree dirty bits:
`TREE`, `LAYOUT`, `COMPOSE`, `PAINT`, `ACCESSIBILITY`, and `HIT_TEST`.
Layout invalidation propagates to ancestors. Paint and semantic property
changes remain local. Composition caches world transforms, clips, and subtree
bounds. Hit testing traverses reverse paint order but rejects an entire
subtree when its cached bounds or clip excludes the pointer.

Elements are currently mutated through typed handles plus an explicit
invalidation contract. Production should replace the prototype's
`update(handle, flags, closure)` with element-specific `NodeMut` setters so an
invalid flag cannot be omitted.

## Layout

The prototype proves a constraints-based custom layout lifecycle: an element's
`layout` method receives `Constraints` and a restricted `LayoutContext` through
which it lays out and places direct children.

Before adoption, production will keep Taffy for standards-complete flex/grid
layout islands, edit its topology incrementally, represent custom layouts as
measured boundaries, remove copied whole-tree measurement maps, and schedule
composition only for changed geometry. Virtual lists, tables, code editors,
and scene canvases use custom layout boundaries.

## Paint, accessibility, and input

Elements paint in local coordinates into immutable `DisplayList` fragments.
The UI caches each fragment and emits a `Scene` of instances carrying world
transforms and clips. `DisplayList::compose` is the implemented compatibility
adapter for today's renderer.

The next renderer step is native fragment consumption in
`astrelis-paint-gpu`: cache compiled geometry by fragment identity/revision,
upload only changed fragments, and assemble order/transforms/clips per frame.
Current path, image, gradient, glyph, shadow, and compositor-view behavior
stays intact. Partial surface damage is a separate decision.

The larger prototype slice now shares `astrelis-text`'s real font database and
Parley shaping context. Labels retain immutable `TextLayout`s in their local
fragments. The editable field exercises shaped pointer-to-caret hit testing,
selection geometry, visual caret movement, Unicode grapheme deletion, typed
change/submit actions, focus, and IME preedit/commit. Per-update diagnostics
count shapes alongside layout and fragment work.

Accessibility output is a delta of inserted, changed, and removed nodes; full
deterministic snapshots remain available. Production input retains 0.3's
routed phases, capture, focus scopes, drag/drop, IME, and semantic actions.
Payload erasure is only the internal boundary used by typed higher layers.

On the development machine, the 10,000-row-model RXUI Next workload realizes
40 tree rows, 30 three-cell table rows, and 100 editable properties:

| Workload | Average | Shaped layouts | Rebuilt fragments | Layout elements |
| --- | ---: | ---: | ---: | ---: |
| Selection + property value | 0.147 ms | 1 | 3 | 5 |
| Resize one 30-row table column | 0.256 ms | 30 | 61 | 63 |
| Controlled property edit | 0.151 ms | 1 | 1 | 5 |

Current RXUI's existing reference-editor benchmark measured 1.218 ms for
selection and 2.402 ms for a table resize on the same machine. This is
directionally encouraging, but not an adoption-gate comparison: the current
benchmark includes its dock workspace and uses a smaller realized data set.
A shared workload and native fragment renderer are still required.

## Adoption gate

The experiment advances only if the paired RXUI slice demonstrates:

- zero retained mutation and passes after unchanged reconciliation;
- keyed moves preserve retained state;
- local paint changes rebuild one fragment;
- structure changes do not rebuild unrelated layout trees;
- editor selection/table resize are at least twice as fast as current RXUI;
- warm scene preparation and 1,000-node targeting are at least three times
  faster;
- native/WebGPU rendering, keyboard behavior, accessibility, and idle
  scheduling remain correct.
