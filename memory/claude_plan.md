# Claude Execution Plan

## Scope
- Current task: P10-T01, solve TypeId cross-process stable wire format.
- Source of truth: TODO.md / TODO-7.md.

## Selected approach
- Used TODO方案 B: portable TypeStore serialization.
- Keep TypeId as local indices inside facts/LIR, but serialize the authoritative TypeStore/type graph so a downstream process can reconstruct valid local TypeId mappings before consuming serialized facts.
- Added schema-version fields to top-level fact/LIR products.
- Added serde/bincode round-trip coverage for TypeStore, four fact products, and LIR program.

## Progress
- Selected P10-T01 as the first incomplete task.
- Latest commit is P9-T09R and directly hands off to P10; no extra prerequisite from commit message.
- Existing unrelated dirty files observed: run_agent.sh and PLUGIN_ABI.md; these were not part of the task and will not be reverted.
- Implemented portable TypeStore serde, schema versioning, serde derives for fact/LIR wire surfaces, and bincode round-trip tests.
- Fixed MIR placeholder/string fallout and source inventory guard from making placeholder reasons wire-friendly.
- Validation passed: cargo fmt, clippy -D warnings, targeted wire tests, full Rust tests, full fixture suite, dependency-gate, and git diff --check.
- Marked P10-T01 [DONE] in TODO.md and TODO-7.md with completion record.
