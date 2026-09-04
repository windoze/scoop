# runtime

The C runtime implements M15's exact moving Immix collector and M25's
runtime-owned exception ABI:

- `src/gc/heap.c` owns arena-external block/object metadata;
- `src/gc/allocation.c` owns mutator TLAB and large-object allocation;
- `src/gc/evacuation.c` owns forwarding, to-space allocation and current-object
  traversal;
- `src/gc/reclamation.c` owns source retirement, ordinary free-space reuse and
  stress-mode poisoning/quarantine;
- `src/gc/collector.c` owns exact mark/relocate/verify traversal;
- `src/gc/roots.c` owns process/image roots, `root_frames.c` owns native
  root frames, `handles.c` owns handles and pins, while `stackmap.c` and
  `stack_roots.c` own LLVM stack-map consumption;
- `src/thread.c` owns thread attachment and registry lifecycle; `src/thread/`
  separates STW collection, managed/native transitions, root frames, and
  debug queries;
- `src/eh.c` owns `ScoopExceptionRecord`, the per-thread caught stack,
  throw/begin/end/rethrow, and stable external-root lifetime;
- `src/eh_personality.c` owns the bounded LSDA decoder and the Scoop
  personality for the qualified LLVM 22.1 Darwin/AArch64 catch-all/cleanup
  profile;
- `src/platform/` provides the target-selected image, OS/VM and frame/ABI
  components. M15 currently supports Darwin/AArch64 only.

`scoopc` compiles the complete source set carried by the selected target
profile into `target/scoop-rt/libscoop_rt.a` and links it into each Scoop
program. Runtime behavior is covered by `cargo test --workspace`, including
the fake-platform stack-map tests and executable fixture suite. The complete
contracts live in `docs/specs/SCOOP-RUNTIME-SPEC.md` and
`docs/milestone15/DESIGN.md`; the exception migration and its object/link
qualification gates are specified in `docs/milestone25/DESIGN.md`.

Generated Scoop programs use only the closed set of Itanium Level-I
`_Unwind_*` entries supplied by Darwin `libSystem`. They neither expose the EH
control-flow entries through the public C FFI header nor link `libc++abi` or an
explicit `libunwind`.

`SCOOP_GC_STRESS_MOVE=1` enables the M15 runtime-only relocation test mode:
every mutator-visible allocation first performs a full moving collection,
old copies are poisoned, and empty source blocks are permanently protected.
