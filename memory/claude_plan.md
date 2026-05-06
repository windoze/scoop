# Claude Execution Plan

## Scope
- Work through exactly one detailed task: the first task whose heading in the authoritative `TODO-Px.md` file is not prefixed with `[DONE]`.
- Treat `TODO.md` only as the index and synchronize it with the detailed task file if completion markers or ordering change.
- Do not proceed to the next task after the selected task is completed or blocked.

## Execution Plan
1. Read `TODO.md` to identify indexed task files and ordering.
2. Read the referenced `TODO-Px.md` files in index order and select the first detailed task whose heading lacks `[DONE]`.
3. Check recent git context for an explicit unfinished issue relevant to the selected task, without doing broad historical triage.
4. Inspect the selected task details, constraints, validation requirements, and nearby implementation areas.
5. Implement the task as specified, using the smallest correct change and avoiding workarounds or scope weakening.
6. If a blocking spec or implementation gap prevents correct completion, add the minimum prerequisite task in the appropriate detailed TODO file, sync `TODO.md`, update this plan file, commit, and stop.
7. Run relevant tests and quality checks for the touched area; fix failures that are in scope.
8. Mark the selected task heading `[DONE]` in the authoritative `TODO-Px.md`, update its completion record, and sync the same marker in `TODO.md`.
9. Commit all changes for this invocation with a descriptive task-tagged message.
10. Stop after the commit.

## Progress Log
- Initial plan recorded before repository inspection.
- Selected task: `P7-T02Z` in `TODO-P7.md`, the first detailed heading without `[DONE]`.
- Latest commit `779c3427 [P7-T02Zd] Fix finally pending completion origins` is directly relevant as the last prerequisite recorded for `P7-T02Z`; current work will resume `P7-T02Z` rather than opening unrelated triage.
- Repository check found no earlier incomplete detailed task before `P7-T02Z`; later incomplete entries begin at `P7-T03` and `P8`.
- Next key step: run the default refactor run-pass fixture suite to find the next concrete remaining blocker for `P7-T02Z`, then fix it without legacy fallback or fixture weakening.
- Current blocker: `tests/fixtures/run-pass/effect_resume_mixed_multi_escape_direct_indirect.scoop` prints an extra `after_first` / `hello` after the second resume.
- Planned fix: narrow composed call-boundary prefix replay so replay is only used for same-arm mixed direct/indirect replay paths; a later indirect suspend for a different handled effect case must resume the callee continuation without replaying already executed caller prefix statements.
- Follow-up blocker: `effect_resume_mixed_source_path_matrix.scoop` reaches the resumed tail but fails before the nested `Abort.stop` handle arm.
- Planned follow-up fix: keep the P7-T02Y guard against non-current outer handle consumption, but allow handles nested inside the current surface-resume handle route to consume effects produced by the resumed tail.
- Next blocker: `effect_same_op_multi_arm_dispatch_effect_instance.scoop` fails during late lowering because a `Raise.raise(...)` expression leaves a `return local<Nothing>` tail that was incorrectly treated as an `Int` completion payload.
- Planned fix: make segmentation treat `Return` of a `Nothing` local as unreachable normal completion, preserving the nonresuming effect contract instead of manufacturing a completion payload.
- Next blocker: `for_in_custom_iterator_effects.scoop` fails because refactor source-slice fallback reuses legacy plain dynamic dispatch and rejects effect-typed interface signatures even when P4/P5 facts proved the concrete candidates are `NoOutward` plain callables.
- Planned fix: add a refactor-only plain dynamic dispatch entry that may accept an effect-typed dispatch signature when no dynamic-invoke layout was published for the source slice, while keeping the existing legacy/pass MIR guard unchanged.
- Next blocker: `gc_array_class_elements_cross_function.scoop` fails when f-string interpolation reads locals assigned from `scoop.core.size`; the locals keep a generic placeholder source type and are not inferred as word-sized `Int` slots.
- Planned fix: extend MIR local assignment CG type inference for the `scoop.core.size` intrinsic direct call so interpolation receives an integer CG value.
- Next blocker: `gc_continuation_multi_thread_concurrent_alloc_resume.scoop` fails because continuation provenance for `Shared.cellA.k` / `Shared.cellB.k` does not cross function boundaries unless the receiver object is passed as a callee parameter.
- Planned fix: publish cross-callable continuation member routes for stable top-level object/member receivers, so a worker function that rereads `Shared.cellA` can resolve the same `CellA.k` continuation written by `main`.
- Next blocker: `gc_pin_unpin_basic.scoop` fails on `GC.pin` / `GC.unpin` function-value callee lowering and then over-counts heap objects because the `GC` namespace object receiver is unnecessarily materialized.
- Planned fix: lower `GC.pin` / `GC.unpin` as refactor-owned MIR intrinsics and treat top-level namespace receivers for static member functions as elidable callee refs.
- Next blocker: `list_and_mutable_list_basic.scoop` fails in stdlib `MutableArray<Int>.push` because `__scoop_array_builder_push` receives a value local whose source type is still generic after `this.get(i)`.
- Planned fix: use the builder intrinsic contract as the fallback value CG type: `__scoop_array_builder_push` is word-sized `Int`, while `__scoop_array_builder_push_string` is `String`.
