# runtime

The C runtime implements M31's generational moving Immix collector and M25's
runtime-owned exception ABI:

- `src/boxing.c` and `src/arrays.c` execute descriptor-checked boxing,
  unboxing and array cloning, including zero-sized payloads;
- `src/value_shape.c` and `src/value_scan.c` check managed shape arithmetic,
  scan translation and storage bounds, with an iterative traversal that reuses
  shared subgraphs and detects cycles without expansion or byte quotas;
- `src/gc/heap.c` owns arena-external block/object metadata;
- `src/gc/allocation.c` owns nursery TLABs and pretenured allocation;
- `src/gc/evacuation_plan.c` reserves to-space before copying or publishing
  forwarding; `evacuation.c` owns forwarding and current-object traversal;
- `src/gc/reclamation.c` owns source retirement, ordinary free-space reuse and
  stress-mode poisoning/quarantine;
- `src/gc/collector.c` coordinates minor/full tracing and relocation;
  `collector_roots.c` visits their shared root sources, `scan.c` restricts exact
  descriptor scans to full objects or card ranges, and `remembered.c` finds
  old objects intersecting dirty cards;
- `src/gc/statistics.c` exposes allocation, promotion, scanning and pause counters;
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

`SCOOP_GC_STATS=1` writes one JSON metrics record to stderr on normal program
exit. Pause times measure the collector phase inside STW, after threads have
parked. The counters are diagnostic and do not impose program limits.

Static TypeDescriptor layouts and scan graphs are checked by the compiler or
artifact reader. Normal runtime operations retain dynamic bounds, count,
exact-type and GC-root checks without repeating full static graph validation.
Build with `-DSCOOP_VERIFY_METADATA=1` to enable that additional validation at
runtime operation boundaries. `scoop_shape_validate` is also available to
explicit metadata checks and tests. Boxing roots use the descriptor's inline
scan pointer directly, including before the managed-entry handshake.
