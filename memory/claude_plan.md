# Claude Execution Plan

## Scope
- Current task: P10-T02, defining the per-cone build artifact disk layout and `scoopc_cone` read/write API.
- Source of truth: `TODO.md` and `TODO-7.md`.
- Latest commit: `[P10-T01R] Review TypeStore wire format`; it directly completed the prerequisite and does not describe a separate unfinished blocker.
- Existing unrelated dirty files: `run_agent.sh` and `PLUGIN_ABI.md`; I will not revert or include them unless the current task requires it.

## Step-by-step plan
1. Inspect `scoopc_cone` crate dependencies and public module structure.
2. Add a focused artifact module that defines:
   - `ConeArtifact`
   - `ConeArtifactManifest`
   - schema/version metadata
   - object file entries
   - `ConeArtifact::write(&self, dir: &Path)`
   - `ConeArtifact::read(dir: &Path) -> Result<Self>`
3. Use the P10-T01 serde/bincode wire surfaces for `hir_facts.bin`, `mir_facts.bin`, `effect_facts.bin`, `lir_facts.bin`, and `lir_program.bin`.
4. Write `manifest.json` with cone name/version, compiler version, and all relevant fact/LIR schema versions.
5. Preserve the existing ScoopIR consume path; this task only adds the parallel artifact IO API.
6. Add unit tests that write a populated artifact to a temp directory, read it back, verify all fact/LIR products and object/fingerprint bytes, and assert the documented file layout exists.
7. Document the layout and compatibility strategy in crate docs or module docs.
8. Run validation in the required order: `cargo fmt`, `cargo test -p scoopc_cone`, `cargo test --all --all-targets`, `git diff --check`.
9. Mark P10-T02 `[DONE]` in both `TODO.md` and `TODO-7.md` with a completion record.
10. Commit the task changes with the required co-author trailer and stop.

## Progress
- Identified P10-T02 as the first incomplete task.
- Updated this plan before implementing the task.
- Added `scoopc_cone::artifact` with documented per-cone layout, manifest/schema metadata, bincode stage product IO, object file IO, fingerprint files, and round-trip tests.
- Targeted `cargo test -p scoopc_cone` passed once before lint; clippy then found an overly broad constructor, so the API was adjusted to group stage products and fingerprints explicitly.
- `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test -p scoopc_cone`, and `cargo test --all --all-targets` passed.
- Marked P10-T02 `[DONE]` in `TODO.md` and `TODO-7.md`; next step is final diff check and commit.
