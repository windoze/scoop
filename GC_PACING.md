# GC Pacing: Collect on Pressure

Status: P0 design. Platform-agnostic. Strictly more urgent than the
immortal-objects work in `GC_IMMORTAL_FIX.md`: that one reduces pressure,
this one decides whether long-running programs run at all.

## Motivation

Today the heap grows monotonically until the process OOMs. The only ways to
trigger a collection in production code are:

1. An explicit `scoop_gc_collect()` call from user code or codegen.
2. The `SCOOP_GC_STRESS=N` env knob, which is a testing instrument — it
   collects on every Nth allocation regardless of whether collection makes
   sense, and is documented as "default off, avoid in production"
   (`runtime/c/scoop_runtime.c:130`).

There is no "collect when the heap has grown by X" mechanism, no nursery-full
trigger, no fallback collection when the block pool is empty. A `loop { ... }`
in user code that allocates anything — including the immortal-fix candidates
in `GC_IMMORTAL_FIX.md` — eventually exhausts the address space.

This is not an edge case. It is the default behaviour of any non-trivial
program: a server, a script with a loop, even the test driver itself if
nothing called `scoop_gc_collect()` between phases.

## Current behavior (verified)

### Every `scoop_gc_collect()` caller

`runtime/c/scoop_runtime_api.h:37-38` exports `scoop_gc_collect` and
`scoop_gc_collect_minor` as public C API.

Internal callers (excluding tests and `scoop_test_*` smoke functions):

```
runtime/c/scoop_runtime.c:501-507    — SCOOP_GC_STRESS testing path only
```

That is the entire production trigger surface. Everything else is the
public API or test code.

### Block-pool exhaustion grows the heap unconditionally

`runtime/c/scoop_gc_immix_internal.h:548-575`:

```c
static inline ScoopGcImmixBlock *scoop_gc_immix_state_take_block(
    ScoopGcImmixState *state) {
    if (state == 0) return 0;

    ScoopGcImmixBlock *block = 0;
    if (state->reusable_blocks != 0) {
        block = state->reusable_blocks;
        state->reusable_blocks = block->next_free;
        block->next_free = 0;
    } else if (state->free_blocks != 0) {
        block = state->free_blocks;
        state->free_blocks = block->next_free;
        block->next_free = 0;
    } else {
        block = scoop_gc_immix_block_alloc_new();   // posix_memalign
        if (block == 0) return 0;
        block->next_all = state->all_blocks;
        state->all_blocks = block;
    }

    state->current_block = block;
    return block;
}
```

When both `reusable_blocks` and `free_blocks` are empty, the path goes
straight to `posix_memalign` for a fresh 32 KB block (`scoop_gc_immix_block_alloc_new`
at `scoop_gc_immix_internal.h:283-299`). There is no "before growing, try
collecting reclaimable blocks first" fallback.

### Nursery cap silently falls back to old space

`runtime/c/scoop_runtime.c:252-323` — when `state->nursery_blocks >=
state->nursery_max_blocks`, `scoop_gc_immix_nursery_take_block_locked`
returns `NULL`. The caller at `runtime/c/scoop_runtime.c:563-567`:

```c
if (state->nursery_max_blocks != 0) {
    (void)pthread_mutex_lock(&state->lock);
    p = scoop_gc_immix_nursery_alloc_locked(state, ...);
    (void)pthread_mutex_unlock(&state->lock);
}
// p is NULL when nursery is full — fall through to old-space alloc
```

drops to the old-space code path. **No minor GC is triggered.** This means
that with `SCOOP_GC_IMMIX_NURSERY_BLOCKS=N` set, the nursery fills up
exactly once on first burst and then stays full forever — every subsequent
allocation goes straight to old. Generations exist as bookkeeping but the
allocation pattern degrades to single-generation.

### `bytes_allocated` is an accounting counter, not a trigger

`runtime/c/scoop_gc_backend_immix.c:78-79,2409-2417,2473-2527,5382-5389`:

```c
static inline void scoop_gc_heap_bytes_allocated_add(uint64_t delta) {
    (void)__atomic_fetch_add(&scoop_gc_heap.bytes_allocated, delta, __ATOMIC_RELAXED);
}
// ...
scoop_gc_heap_bytes_allocated_add(obj->size_bytes);    // line 2416, on alloc register
// ...
heap->bytes_allocated = 0;                              // line 2524, heap init
// ...
uint64_t scoop_gc_debug_heap_bytes_allocated(void) {    // line 5382
    return __atomic_load_n(&scoop_gc_heap.bytes_allocated, __ATOMIC_RELAXED);
}
```

Incremented on allocation registration, initialized/reset by heap init, read
out via a debug helper. **Never compared against any threshold.** There is no
cycle-end `next_gc` update today.

The P1 cycle-end update point is `scoop_gc_collect` in
`runtime/c/scoop_gc_backend_immix.c:4892-5366`: while `state->lock` is held
and STW is active, object sweep runs at `:5213-5240`, region sweep at
`:5242-5348`, and optional compaction / verification at `:5350-5357`.
`next_gc = max(min_threshold, live * growth_factor)` should be updated after
those steps and before `scoop_gc_stop_the_world_end_unlocked()` / unlock at
`:5364-5365`.

### Env-variable surface

The fixed runtime env knobs visible from `grep getenv runtime/c/`:

| Var | Purpose |
|---|---|
| `SCOOP_GC_STRESS` | testing — collect every N allocs |
| `SCOOP_GC_IMMIX_PARALLEL_MARK` | parallel mark on/off |
| `SCOOP_GC_IMMIX_PARALLEL_SWEEP` | parallel sweep on/off |
| `SCOOP_GC_IMMIX_NURSERY_BYTES` | nursery cap (bytes) |
| `SCOOP_GC_IMMIX_NURSERY_BLOCKS` | nursery cap (blocks) |
| `SCOOP_GC_VERIFY_ROOTS` | GC roots verification diagnostic |
| `SCOOP_GC_MOVE` | baseline backend moving-GC diagnostic |
| `SCOOP_STACKMAP_STRICT` | stackmap parser strict-mode diagnostic |

`runtime/c/platform/platform_posix.c:30` is a generic platform getenv wrapper,
not a fixed GC pacing knob. No `SCOOP_GC_PACING`, no heap target / trigger
bytes, no growth-factor knob, no min-threshold knob, no hard cap. Pacing is
genuinely missing — not just untuned.

## Proposed design

### Pacing model: target heap = `live * growth_factor`

Industry-standard heuristic (Boehm, V8, Go, .NET):

```
After GC:    live    = bytes_allocated_after_sweep
             next_gc = max(min_threshold, live * growth_factor)
On alloc:    if (bytes_allocated >= next_gc) trigger_collect()
```

Default growth factor `1.5` is a good starting point: tolerates 50%
overhead before collecting, which is roughly where most steady-state
workloads converge. Defaults are tunable via env, but the **on-by-default
posture is critical** — we are not trading off correctness against
ergonomics here, the existing situation is unconditional unbounded growth.

Initial `next_gc` (before the first cycle) needs a floor so we don't trip
on small startup allocations: `min_threshold = 4 MB` is plenty for most
programs and trivial on real hardware. Embedded targets may want a smaller
floor; that is a tier-specific tuning concern, not a design issue.

### Three trigger points, layered

In ascending order of urgency:

1. **Soft trigger — heap-growth threshold.** Inside the alloc fast path,
   after `bytes_allocated_add`, compare against `next_gc`. If exceeded,
   request a collection at the next safepoint (not immediately — we are
   still inside an alloc, the new object isn't rooted yet). The natural
   place to land the collection is the existing `scoop_gc_safepoint_poll`
   that already runs at the top of `scoop_alloc` (`scoop_runtime.c:498-499`).
   Two-phase: alloc N+1 sets a "collection requested" flag; alloc N+2
   observes the flag at the safepoint and runs the cycle before allocating.

2. **Medium trigger — nursery full ⇒ minor GC.** Replace the silent
   fallback at `scoop_runtime.c:563-567` with: if nursery is full, run a
   minor collection, then retry the nursery alloc. If still full after
   minor GC, *then* fall through to old-space (so we don't infinite-loop
   when the nursery is genuinely small relative to a single object). This
   restores the actual benefit of having a nursery.

3. **Hard trigger — block pool exhausted ⇒ full GC then retry.** Replace
   the unconditional `scoop_gc_immix_block_alloc_new` in
   `scoop_gc_immix_internal.h:548-575` with: when both lists are empty,
   run a full collection first; if that produced reusable/free blocks,
   take one; if not, only then grow via `posix_memalign`. Optional hard
   cap `SCOOP_GC_MAX_HEAP_BYTES` returns OOM at the post-GC retry instead
   of growing past the limit.

The three are independent: (1) is pure pacing, (2) is generational
correctness, (3) is OOM defence. Land in that order; each is a useful
delta on its own.

### Why a flag, not synchronous collect inside `scoop_alloc`

Three reasons:

- **Root publication.** The new object is not yet rooted when alloc returns,
  so a collection right before the alloc finishes risks it being seen as
  unrooted. Today's stress path also collects "before alloc" precisely for
  this reason (`runtime/c/scoop_runtime.c:130` — comment confirms).
  Pacing must use the same discipline.
- **Reentrancy.** The collector itself allocates auxiliary structures
  (mark stack, pinned list); a synchronous trigger inside alloc can recurse.
- **Safepoint integration.** The existing `scoop_gc_safepoint_poll` is the
  single point where stop-the-world is set up. Routing the trigger through
  it gets cooperative STW for free.

```
scoop_alloc(size):
    safepoint_poll()          // <-- runs requested collect here
    if (bytes_allocated >= next_gc) request_collect()
    p = bump-allocate(size)
    bytes_allocated += size
    return p
```

The threshold check after alloc is fine because the *next* alloc will see
the flag and collect before its own allocation. A small overshoot of one
object is acceptable.

### Concurrency

The threshold check needs to be cheap on the hot path:

```c
uint64_t allocd = __atomic_fetch_add(&scoop_gc_heap.bytes_allocated,
                                     delta, __ATOMIC_RELAXED);
if (allocd + delta >= next_gc_load_relaxed()) {
    request_collect();
}
```

`next_gc` is updated only at the end of a GC cycle (under the GC lock), so
a relaxed load is fine — the worst case is some threads seeing the old
threshold for a few allocations, which means we collect a bit late, not
incorrectly.

`request_collect` is idempotent: it sets a flag (or increments a request
counter); the safepoint poll consumes it. No-op when collection is already
in progress.

### Env knobs

```
SCOOP_GC_HEAP_TARGET_GROWTH_FACTOR  default 1.5
SCOOP_GC_HEAP_MIN_THRESHOLD_BYTES   default 4 * 1024 * 1024
SCOOP_GC_MAX_HEAP_BYTES             default 0 (no cap)
SCOOP_GC_PACING                     default "on" — set "off" to fully
                                    disable pacing (testing only;
                                    keeps the old unbounded behaviour)
```

The last knob is intentional: there are corners of the test suite that
assert exact heap-object counts after explicit collects (e.g. the smoke
tests at `runtime/c/scoop_gc.c:2986/3225/3246`). Those need pacing off
to remain deterministic. The default is on for production.

`SCOOP_GC_STRESS` continues to work and continues to win — when stress
is active, pacing is bypassed (stress already collects far more often
than pacing would).

## Phasing

1. **Pacing core (Phase 1).** Add `next_gc` field to `ScoopGcHeap`,
   wire `request_collect` flag, hook into `scoop_gc_safepoint_poll`, add
   the threshold compare in alloc, set `next_gc = max(min, live * factor)`
   at the end of every cycle. Add the `SCOOP_GC_PACING` env knob. Land
   with a long-running test that loops 10M allocations and asserts
   `bytes_allocated` stays bounded (within ~`growth_factor * peak_live`).

2. **Nursery-full minor GC (Phase 2).** Replace the silent fallback at
   `scoop_runtime.c:563-567` with a minor-GC-then-retry. Add a regression
   test that fixes `SCOOP_GC_IMMIX_NURSERY_BLOCKS=4`, allocates a workload
   that produces lots of garbage, and asserts the nursery is *not* stuck
   at full forever (block count fluctuates; `bytes_freed` increases).

3. **Block-pool exhaustion fallback (Phase 3).** Modify
   `scoop_gc_immix_state_take_block` (`scoop_gc_immix_internal.h:548-575`)
   to attempt a full GC before `posix_memalign`. Add a regression test
   that pins a small heap via `SCOOP_GC_MAX_HEAP_BYTES` and verifies that
   programs running close to the cap still progress.

4. **Hard cap (Phase 4).** Wire `SCOOP_GC_MAX_HEAP_BYTES`. After the
   post-GC retry in step 3, if still over the cap, return NULL from
   `scoop_alloc`. The OOM behaviour upstream is unchanged — `scoop_alloc`
   already documents `OOM ⇒ NULL` (`runtime/c/scoop_runtime.c:514`); we
   are only making it reachable.

5. **Backend parity (Phase 5).** The `hosted` and `minimal` backends
   (`scoop_gc_backend_hosted.c`, `scoop_gc_backend_minimal.c`) should also
   honour the pacing knobs even though their `scoop_gc_collect` is more
   restrictive (no-op under multi-thread for hosted). The threshold
   compare itself is backend-independent.

Phases 1–4 are sequential; Phase 5 can land in parallel with any later
phase.

## Test plan

**Unit (runtime, C):**

- Long-running alloc loop: 10M tiny allocations; assert peak heap is
  bounded by `growth_factor * peak_live` plus one block of slop. With
  pacing off, the same test should grow without bound (sanity check that
  pacing is actually doing something).
- Nursery-full minor GC: fixed nursery cap, mixed live/dead workload,
  assert `gc_cycles` increments and nursery returns to having free space
  after the trigger fires.
- Block-pool exhaustion: configure tight `SCOOP_GC_MAX_HEAP_BYTES`,
  allocate close to the cap, assert that allocations succeed (because
  collection reclaims) and that overshoots return NULL cleanly.
- Threshold-check thread safety: many threads allocating concurrently;
  the request-collect flag should not deadlock and should bound the
  total over-allocation past the threshold.

**Integration:**

- Run the existing test suite with default pacing on. Whatever asserts
  exact heap counts must opt out via `SCOOP_GC_PACING=off`. This serves
  as the audit pass — every test that needs pacing off documents *why*.
- The immortal-fix tests (when that work lands) should *not* need
  pacing off, because immortals never enter the heap.

**Existing behaviour:**

- `SCOOP_GC_STRESS=N` semantics unchanged.
- Manual `scoop_gc_collect()` calls unchanged.
- Default behaviour with no env knobs: dramatically different (now
  bounded). This is the point.

## Out of scope (future work)

- **Incremental / concurrent GC.** Pacing here is purely about *when* to
  trigger a stop-the-world cycle. Concurrent marking is a separate
  initiative.
- **Time-budget pacing.** Collect within a deadline rather than a byte
  threshold (V8-style soft pacer). Wants concurrent first.
- **Allocation-rate prediction.** Adaptive growth factor based on observed
  rate. The fixed factor is fine until we have data showing it isn't.
- **Per-tier targets on heterogeneous memory.** ESP32-class chips with
  SRAM/PSRAM split likely want separate thresholds. Defer to the embedded
  port design.
- **Pause-time tuning.** The current marker is single-pass STW; pacing
  changes when collections happen, not how long they take. Reducing pause
  time is a different lever.
- **Tracing / profiling hooks.** A `scoop_gc_on_cycle` callback for
  observability. Useful but not blocking.
