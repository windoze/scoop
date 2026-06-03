//! Self-contained LIR handoff types for the LLVM codegen boundary.

use std::path::PathBuf;

use crate::effect_lowered::LateLoweredProgram;
use crate::llvm::LlvmStageBaseContext;
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
    pub facts: LirFacts,
    pub base_context: LlvmStageBaseContext,
    pub mir: MaterializedMir,
    pub object_files: Vec<PathBuf>,
}

/// Complete LLVM codegen input after LIR-stage construction.
#[derive(Debug)]
pub struct CodegenInput {
    pub program: LirArtifact,
    pub abi_shell: Option<LirArtifact>,
    pub deps: Vec<LirArtifact>,
    pub entry: Option<(SourceId, Option<String>)>,
    pub source_map: SourceMap,
    pub opt_level: OptLevel,
}
