// Shared effect/state-machine analysis now lives at the crate root so backend
// and non-LLVM consumers reuse the same source without depending on a backend
// path. Keep this thin wrapper so the unified skeleton module can continue to
// include the shared analysis into its local visibility scope.
include!("../../../effect_state_machine_analysis.rs");
