# Claude Execution Plan

## Scope

- Authoritative task source: `TODO.md`.
- First incomplete task: `CG-T04a` (`建立 composite transport layout contract 与 verifier`).
- Complete exactly this task, mark it `[DONE]`, commit the result, then stop.
- This file records the actionable plan and progress checkpoints; it does not include private chain-of-thought.

## Task Requirements

- Consume or normalize MIR-T10 composite transport/layout metadata in LLVM codegen.
- Cover size, align, storage kind, trace/copy/drop hook identity, and GC slot map.
- Add a shared verifier/backend gate so composite transport use sites fail fast when missing layout descriptors.
- Point gate diagnostics to the owner-specific follow-up tasks `CG-T04b` through `CG-T04f`.
- Add runtime descriptor plumbing for trace/copy/drop hook registration and call surface.
- Keep specific boxing, enum payload, array element, closure env, and thread payload lowering unsupported for now, but reject each with explicit owner-specific gates.

## Execution Steps

1. Check the latest commit summary for unfinished work directly relevant to `CG-T04a`.
2. Inspect the current MIR-T10 metadata surface, existing LLVM backend gates, codegen gap inventory, and runtime descriptor APIs.
3. Identify existing composite transport use sites for value boxing, enum payloads, arrays, closure env/captures, and cross-thread resume payloads.
4. Implement the smallest shared composite transport layout descriptor and verifier integration needed by later CG-T04 tasks.
5. Add runtime hook registration/call plumbing without fake no-op hooks for traceable values.
6. Add or update targeted tests, including `refactor_llvm_composite_transport_contract` and missing-descriptor negative cases.
7. Run required validation: `cargo test -p scoopc refactor_llvm_composite_transport_contract` and `cargo test -p scoopc codegen_gap_inventory`; run additional directly relevant tests as needed.
8. Update `TODO.md` with `[DONE]` on `CG-T04a` and a completion record listing actual validation.
9. Run formatting/lint checks as needed, inspect the final diff, then commit all relevant changes with a `CG-T04a` message.

## Current Status

- Selected task: `CG-T04a`.
- Initial TODO review complete: `CG-T04a` is the first heading not prefixed with `[DONE]`.
- Latest commit check complete: `Update plan` does not mention a directly relevant unfinished issue.
- Implementation complete for first pass: added C runtime composite transport descriptor/call surface and LLVM codegen-side composite layout descriptor verification/emission for MIR transport metadata.
- Integration fix complete: refactor plain callable codegen now runs the composite transport verifier before lowering body slices, so refactor stage output emits descriptor globals and hook declarations.
- Final validation passed: `cargo test -p scoopc refactor_llvm_composite_transport_contract`, `cargo test -p scoopc codegen_gap_inventory`, `cargo test -p scoop_runtime abi_exports_allowlist`, `cargo test -p scoop_runtime --test gc_immix_nursery`, and `cargo clippy --all-targets -- -D warnings`.
- Note: an exploratory full `cargo test -p scoop_runtime` exceeded the 120s command timeout after many tests had passed; targeted runtime ABI/nursery checks above completed successfully.
- `TODO.md` updated: `CG-T04a` is now `[DONE]` in the task index and heading, with a completion record and validation list.
- Next step: inspect final diff and commit all relevant changes with a `CG-T04a` message.
