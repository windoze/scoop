# runtime

The C runtime implements a generational moving Immix collector and the
runtime-owned exception ABI:

- `src/boxing.c` and `src/arrays.c` execute descriptor-checked boxing,
  unboxing and array cloning, including zero-sized payloads;
- `src/value_shape.c` and `src/value_scan.c` check managed shape arithmetic,
  scan translation and storage bounds, with an iterative traversal that reuses
  shared subgraphs and detects cycles without expansion or byte quotas;
- `src/gc/heap.c` owns region-external block/object metadata;
- `src/gc/regions.c` owns 16 MiB ordinary regions and independently sized large
  mappings; `page_map.c` publishes their sparse address index, and `cards.c`
  implements the native range barrier;
- `src/gc/allocation.c` owns nursery TLABs and pretenured allocation;
- `src/gc/evacuation_sources.c` selects sparse unpinned regions for ordinary full
  collection; `evacuation_plan.c` reserves target space and checks concentration
  benefit before copying or publishing forwarding; `evacuation.c` owns forwarding
  and current-object traversal;
- `src/gc/reclamation.c` closes collection accounting; `block_sweep.c` releases
  dead objects and rebuilds live lines and reusable holes; `block_release.c`
  retires block metadata and protects stress quarantine; `memory_return.c`
  discards empty pages and removes empty mappings after reference updates;
- `src/gc/collector.c` coordinates minor/full tracing and relocation;
  `collector_roots.c` visits their shared root sources, `scan.c` restricts exact
  descriptor scans to full objects or card ranges, and `remembered.c` finds
  old objects intersecting dirty cards;
- `src/gc/mark.c` retains the first-marked live set for relocation; `mark_objects.c`
  owns atomic marking and batched scans, `mark_queue.c` owns task publication and
  stealing, and `mark_pool.c` coordinates persistent internal marker threads;
- `src/gc/statistics.c` exposes allocation, promotion, scanning, collector phase
  times and per-worker CPU counters;
- `src/gc/roots.c` owns process/image roots, `root_frames.c` owns native
  root frames, `handles.c` owns handles and pins, while `stackmap.c` and
  `stack_roots.c` own LLVM stack-map consumption;
- `src/thread.c` owns thread attachment and registry lifecycle; `src/thread/`
  separates STW collection, managed/native transitions, root frames, and
  debug queries;
- `src/eh.c` owns `ScoopExceptionRecord`, the per-thread caught stack,
  throw/begin/end/rethrow, and stable external-root lifetime;
- `src/eh_personality.c` owns the Scoop personality; `src/eh/` contains
  the bounded LLVM 22.1 catch-all/cleanup LSDA decoder, including tables
  that omit TType for cleanup-only functions;
- `src/platform/` provides the target-selected image, OS/VM and frame/ABI
  components for Darwin/AArch64 and Linux/amd64 with glibc or musl. OS/image
  code and architecture-specific frame/entry code are separate components.

`scoop build` compiles the complete runtime source set carried by the selected
target profile and caches its objects and runtime index for final linking.
`scoop-link` can consume that index and existing `.slib` artifacts without
compiler stages. Runtime behavior is covered by `cargo test --workspace` and
`python3 tests/run_fixtures.py --all`, including real moving-GC programs in
addition to the fake-platform stack-map tests. The complete
contracts live in `docs/specs/SCOOP-RUNTIME-SPEC.md` and
`docs/milestone15/DESIGN.md`; the exception migration and its object/link
qualification gates are specified in `docs/milestone25/DESIGN.md`. Linux
toolchain preparation and verification are documented in
[`docs/milestone28/BUILDING.md`](../docs/milestone28/BUILDING.md).

Generated Scoop programs use only the closed set of Itanium Level-I
`_Unwind_*` entries supplied by Darwin `libSystem` or the selected Linux LLVM
libunwind archive. Linux builds keep separate glibc and musl unwind prefixes.
Programs do not expose EH control-flow entries through the public C FFI
header or link `libc++abi`; one provider supplies the Level-I unwind ABI.

`SCOOP_GC_STRESS_MOVE=1` enables the M15 runtime-only relocation test mode:
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
Build with `-DSCOOP_VERIFY_METADATA=1` to enable that additional validation at
runtime operation boundaries. `scoop_shape_validate` is also available to
explicit metadata checks and tests. Boxing roots use the descriptor's inline
scan pointer directly, including before the managed-entry handshake.
