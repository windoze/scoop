# runtime

The C runtime implements a generational moving Immix collector and the
runtime-owned exception ABI:

- `src/boxing.c` and `src/arrays.c` execute descriptor-checked boxing,
  unboxing and array cloning, including zero-sized payloads;
- `src/gc.h` exposes complete allocation, checked-value and lifecycle operations
  to ordinary runtime code, including an inline copy with its range barrier;
- `src/value_shape.c` computes managed allocation sizes and checks dynamic
  counts, exact object sizes and operation-specific representation bounds;
- `src/gc/heap.c` owns region-external block/object metadata;
- `src/gc/regions.c` owns 16 MiB ordinary regions and independently sized large
  mappings; `page_map.c` publishes their sparse address index, and `cards.c`
  implements the native range barrier;
- `src/gc/allocation.c` owns nursery TLABs and pretenured allocation;
- `src/gc/mutator.c` owns per-thread allocation state and detached accounting;
  its private state embeds the existing generated-code TLAB context as a prefix;
- `src/gc/evacuation_sources.c` selects sparse unpinned regions for ordinary full
  collection; `evacuation_plan.c` reserves target space and checks concentration
  benefit before copying or publishing forwarding; `evacuation.c` owns forwarding
  and current-object traversal;
- `src/gc/reclamation.c` closes collection accounting; `block_sweep.c` releases
  dead objects and rebuilds live lines and reusable holes; `block_release.c`
  retires block metadata and protects stress quarantine; `memory_return.c`
  discards empty pages and removes empty mappings after reference updates;
- `src/gc/collector.c` coordinates minor/full tracing and relocation;
  `remembered.c` finds old objects intersecting dirty cards;
- `src/gc/mark.c` retains the first-marked live set for relocation; `mark_objects.c`
  owns atomic marking and batched scans, `mark_queue.c` owns task publication and
  stealing, and `mark_pool.c` coordinates persistent internal marker threads;
- `src/gc/pressure.c` deliberately ignores external pressure hints; the public
  `gc_binding/pressure.c` facade remains GC-free, including in release hooks;
- `src/gc/statistics.c` exposes allocation, promotion, scanning, collector phase
  times and per-worker CPU counters;
- `src/gc_binding/` adapts Scoop representation to collector visitors:
  `roots.c` owns process/image roots, `root_frames.c` owns native root frames,
  `collector_roots.c` visits frozen thread slots, and `stackmap.c` and
  `stack_roots.c` consume LLVM stack maps. `scan.c` interprets exact object/range
  scans; inline views in `object.h` expose allocation layout, object identity,
  release hooks and array ranges to the GC. `values.c` owns the heap-lock/query
  sequences for checked unboxing and clone sources.
  `entries.c` joins the collector to the exported managed/native entry ABI;
  `threads.c` adapts world coordination, frozen mutator iteration and statistics
  to the existing thread registry without maintaining a second directory;
- `src/gc/handles.c` owns handle roots and pin counts, with scoped thread pin
  slots enumerated by the binding;
- `src/thread.c` owns thread attachment and registry lifecycle; `src/thread/`
  separates STW collection, managed/native transitions, root frames, and
  debug queries;
- `src/eh.c` owns `ScoopExceptionRecord`, per-thread active and caught lists,
  and throw/begin/end/rethrow; the collector visits each active record's
  writable object slot through the existing thread root scan;
- `src/eh_personality.c` owns the Scoop personality; `src/eh/` contains
  the bounded LLVM 22.1 catch-all/cleanup LSDA lookup, including tables
  that omit TType for cleanup-only functions. Lookup reads an ordered prefix
  and only the selected action without revalidating the entire table;
- `src/platform/` provides the target-selected image, OS/VM and frame/ABI
  components for Darwin/AArch64 and Linux/amd64 with glibc or musl. OS/image
  code and architecture-specific frame/entry code are separate components.

Ordinary builds and both link commands use the selected Scoop sysroot's
`lib/<canonical-target-triple>/runtime/index.cbor` and precompiled objects.
An explicit `scoop build --runtime-root <source>` compiles the selected target's
runtime sources and caches the resulting objects and index. Development resources
are prepared manually with [`scripts/prepare_sysroot.py`](../scripts/prepare_sysroot.py).
User native compilation reads the public headers from the selected sysroot's
`include` directory. Runtime behavior is covered by `cargo test --workspace` and
`python3 tests/run_fixtures.py --all`, including real moving-GC programs in
addition to the fake-platform stack-map tests. The complete
contracts live in `docs/specs/SCOOP-RUNTIME-SPEC.md` and
`docs/milestone15/DESIGN.md`; the exception ABI and linking are described in
`docs/milestone25/DESIGN.md`. Linux
toolchain preparation and verification are documented in
[`docs/milestone28/BUILDING.md`](../docs/milestone28/BUILDING.md).

Generated Scoop programs use only the closed set of Itanium Level-I
`_Unwind_*` entries supplied by Darwin `libSystem` or the selected Linux LLVM
libunwind archive. Linux builds keep separate glibc and musl unwind prefixes.
Programs do not expose EH control-flow entries through the public C FFI
header or link `libc++abi`; one provider supplies the Level-I unwind ABI.

Build with `scoop --enable-gc-stress` to include runtime stress support. Without
that build option, stress code and environment reads are omitted, so the stress
environment variables have no effect. With support included, `0` or an unset
variable leaves its mode disabled.

`SCOOP_GC_STRESS_MOVE=1` enables the runtime-only relocation test mode:
every mutator-visible allocation first performs a full moving collection,
old copies are poisoned, and empty source blocks are permanently protected.

Ordinary allocation uses a 1 MiB nursery. Minor collection traces roots, dirty
old cards and the young graph, promotes survivors once, and keeps blocks with
pinned objects in place. Large objects and objects with release hooks start
in the old generation. An explicit collection remains full. Promotion reserves
space before changing the graph; failure falls back to full collection in the
same STW interval. `SCOOP_GC_STRESS_MINOR=1` reduces nursery capacity to one block
while retaining the ordinary allocation and remembered-set paths. Full moving
stress takes precedence if both switches are set.

Nursery refill records young blocks in a private heap list. Minor mark setup,
block-count aggregation, source selection and retirement use that list and the
newly reserved targets instead of enumerating old blocks. Available regions are
indexed separately, and region reclamation runs only after full collection.
Remembered-card scanning and clearing still use the region directory; scanning
uses `memchr` to skip contiguous clean cards without changing dirty-card order.

Ordinary regions grow on demand and contain 32 KiB blocks with 128-byte lines.
Large objects use independent mappings rounded to 64 KiB, including objects
larger than the former 1 GiB arena. Alignment padding is unmapped immediately;
dead large mappings are removed after reference updates. Block and TLAB
metadata identify their owning region. Generated single-slot barriers perform
four acquire radix loads and mark the region's 512-byte card; range barriers
cover the complete destination range without allocation or a heap lock.
Ordinary full collection keeps the densest region and considers other unpinned
regions with at most one quarter live bytes for evacuation. Targets reuse holes
in live old blocks before empty blocks; reserved targets must occupy fewer empty
regions than the sources they evacuate. Reservation failure or a lack of actual
concentration benefit leaves the live graph in place. Minor and explicit moving
stress retain their block-level selection policies.

After full collection, complete pages outside live object extents are offered to
the OS with `MADV_DONTNEED`. One empty ordinary region is retained; other empty
regions are removed from allocation structures and the page map, then unmapped.
Discard failures preserve the mapping and are retried by a later full collection.
Diagnostic metrics distinguish active block bytes, mapped virtual bytes,
successful/failed discard calls, cumulative advised/unmapped bytes, and sampled
process current/peak RSS. Advice success is not a promise of immediate RSS change.
M34 implementation and measurement status is tracked in
`docs/milestone34/PROGRESS.md`.

Marking runs after all mutators stop. Minor collection and heaps with less than
4 MiB of active blocks use the coordinator alone; larger full collections use
up to four workers including the coordinator, bounded by online CPU count.
`SCOOP_GC_WORKERS=1..8` overrides that policy for comparisons. Internal workers
are created lazily, remain outside the mutator registry, and are joined at
shutdown. Thread creation failure joins workers already created and selects the
same single-worker collector for subsequent collections. Ordinary tasks scan up
to 64 objects, and reference arrays are split into ranges of up to 1024 elements.
Global outstanding work includes active tasks and child publication; an empty
queue does not end marking. Copying, reference updates, release hooks and memory
reclamation remain on the coordinator. The retained live set avoids a second
reachability traversal during full reference updates.

`SCOOP_GC_STATS=1` writes one JSON metrics record to stderr on normal program
exit. Pause times measure the collector phase inside STW, after threads have
parked. Stop-wait time is separate; roots, remembered sets, marking, planning,
copying, reference updates, reclamation and VM returns have separate counters.
Worker CPU and marked-object counts are reported separately from wall time.
The counters are diagnostic and do not impose program limits.

Static TypeDescriptor layouts and scan graphs are checked by the compiler or
artifact reader. Normal runtime operations retain dynamic bounds, count,
exact-type and GC-root checks without repeating full static graph validation.
There is no runtime mode that replays static metadata validation.
`-DSCOOP_GC_VERIFY_HEAP=1` and moving stress can check the current heap and
references after collection. Boxing roots use the descriptor's inline scan
pointer directly, including before the managed-entry handshake.

Normal GC marking and reference updates leave non-heap references unchanged
without repeating the immortal registration lookup. Stress-capable runtimes
(`SCOOP_ENABLE_GC_STRESS=1`) and explicit heap-verification builds retain that
diagnostic lookup, independently of application debug/release profiles. Heap
object-start checks and image-registration, pin and handle checks still apply.

Startup timing is a separate compile-time option: `scoop --enable-startup-stats`
selects a runtime built with `SCOOP_ENABLE_STARTUP_STATS=1`. It reports one
`scoop_startup` JSON record on stderr after eager initialization and before the
root gateway. The record includes seven non-overlapping phase durations and
counts from the existing registration indices. It excludes the native loader,
managed argument construction, main, shutdown, and reporting time. Default
runtimes preprocess out all startup timing, reporting, diagnostic strings and
dedicated count access; environment variables cannot enable them. The option is
independent of debug/release and GC stress, and also applies to `scoop-link`.
