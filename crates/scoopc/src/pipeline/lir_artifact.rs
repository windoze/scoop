//! Self-contained LIR handoff types for the LLVM codegen boundary.

use std::path::PathBuf;

use crate::effect_lowered::LateLoweredProgram;
use crate::llvm::{CachedDepArtifactHandoff, LlvmEmitError, LlvmStageBaseContext};
use crate::mir::MaterializedMir;
use crate::opt::OptLevel;
use crate::source::{SourceId, SourceMap};
use crate::stable_id::StableConeKey;
use scoopc_lir_facts::LirFacts;

/// A cone-level LIR artifact that carries the current transitional LIR payload.
#[derive(Debug)]
pub struct LirArtifact {
    pub cone: StableConeKey,
    pub program: LateLoweredProgram,
    /// Transitional flat facts carried until P2 folds them into the LIR program.
    pub facts: LirFacts,
    pub base_context: LlvmStageBaseContext,
    /// Transitional MIR overlay fallback retained only for the primary cone until P2 lifts source bodies into LIR.
    pub mir: Option<MaterializedMir>,
    pub object_files: Vec<PathBuf>,
}

/// Complete LLVM codegen input after LIR-stage construction.
#[derive(Debug)]
pub struct CodegenInput {
    pub program: LirArtifact,
    pub abi_shell: Option<LirArtifact>,
    pub deps: Vec<LirArtifact>,
    /// Temporary entry placeholder; T1-06 replaces this with a resolved LIR entry ref.
    pub entry: Option<(SourceId, Option<String>)>,
    pub source_map: SourceMap,
    pub opt_level: OptLevel,
}

/// Convert a cached dependency handoff into the same LIR artifact shape used by the primary cone.
pub fn lir_artifact_from_dep(dep: CachedDepArtifactHandoff) -> Result<LirArtifact, LlvmEmitError> {
    let (_cone_id, cone, program, facts, type_store, object_files) = dep.into_parts();
    let base_context =
        LlvmStageBaseContext::from_cached_dep_type_store(cone.clone(), &facts, type_store)?;
    Ok(LirArtifact {
        cone,
        program,
        facts,
        base_context,
        mir: None,
        object_files,
    })
}
