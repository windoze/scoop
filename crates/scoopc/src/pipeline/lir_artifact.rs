//! Self-contained LIR handoff types for the LLVM codegen boundary.

use std::path::PathBuf;

use crate::effect_lowered::{LateLoweredProgram, LirCallableIndex, LirCallableIndexError};
use crate::llvm::{
    CachedDepArtifactHandoff, EntryMainArgShape, EntryRef, LlvmEmitError, LlvmStageBaseContext,
};
use crate::mir::MaterializedMir;
use crate::opt::OptLevel;
use crate::source::{SourceId, SourceMap};
use crate::stable_id::StableConeKey;
use crate::ty::{RefTypeKind, TypeKind, ValueTypeKind};
use scoopc_ids::{LirCallableHash, LirCallableId};
use scoopc_lir_facts::{LirCallableFacts, LirFacts};

/// A cone-level LIR artifact that carries the current transitional LIR payload.
#[derive(Debug)]
pub struct LirArtifact {
    pub cone: StableConeKey,
    pub program: LateLoweredProgram,
    pub callable_index: LirCallableIndex,
    /// Transitional flat facts carried until P2 folds them into the LIR program.
    pub facts: LirFacts,
    pub base_context: LlvmStageBaseContext,
    /// Transitional MIR overlay fallback retained only for the primary cone until P2 lifts source bodies into LIR.
    pub mir: Option<MaterializedMir>,
    pub object_files: Vec<PathBuf>,
}

impl LirArtifact {
    pub fn new(
        cone: StableConeKey,
        program: LateLoweredProgram,
        facts: LirFacts,
        base_context: LlvmStageBaseContext,
        mir: Option<MaterializedMir>,
        object_files: Vec<PathBuf>,
    ) -> Result<Self, LlvmEmitError> {
        let callable_index =
            LirCallableIndex::from_program(&program).map_err(lir_callable_index_emit_error)?;
        Ok(Self {
            cone,
            program,
            callable_index,
            facts,
            base_context,
            mir,
            object_files,
        })
    }

    pub fn callable_id_for_hash(
        &self,
        hash: LirCallableHash,
    ) -> Result<LirCallableId, LlvmEmitError> {
        self.callable_index
            .id_for_hash(hash)
            .map_err(lir_callable_index_emit_error)
    }
}

/// Complete LLVM codegen input after LIR-stage construction.
#[derive(Debug)]
pub struct CodegenInput {
    pub program: LirArtifact,
    pub abi_shell: Option<LirArtifact>,
    pub deps: Vec<LirArtifact>,
    /// Main-mode entry callable resolved at the LIR boundary; lib-mode emits may leave this empty.
    pub entry: Option<EntryRef>,
    pub entry_source_id: SourceId,
    pub source_map: SourceMap,
    pub opt_level: OptLevel,
}

pub fn resolve_entry_ref(
    entry_source_id: SourceId,
    artifact: &LirArtifact,
    entry_main_fqn: Option<&str>,
    entry_required: bool,
) -> Result<Option<EntryRef>, LlvmEmitError> {
    if !entry_required && entry_main_fqn.is_none() {
        return Ok(None);
    }

    let mut candidates = artifact
        .facts
        .callables
        .iter()
        .filter(|(_, callable)| callable.is_top_level_source_callable())
        .filter(|(_, callable)| match entry_main_fqn {
            Some(entry_main_fqn) => callable.root_fqn() == entry_main_fqn,
            None => callable_source_name(callable.root_fqn()) == "main",
        })
        .filter_map(|(callable_id, callable)| {
            classify_entry_main_arg_shape(artifact.base_context.types(), callable)
                .map(|arg_shape| (*callable_id, callable, arg_shape))
        })
        .collect::<Vec<_>>();

    match candidates.len() {
        0 => {
            if let Some(entry_main_fqn) = entry_main_fqn {
                return Err(LlvmEmitError::Frontend {
                    message: format!("LLVM LIR stage 找不到合法入口 callable `{entry_main_fqn}`"),
                });
            }
            Err(LlvmEmitError::MissingEntryMain)
        }
        1 => {
            let (callable_id, callable, arg_shape) = candidates.pop().expect("len checked above");
            let program_callable =
                artifact
                    .program
                    .callable_by_id(callable_id)
                    .ok_or_else(|| LlvmEmitError::Frontend {
                        message: format!(
                            "LLVM LIR stage 入口 `{}` 缺少 matching LIR callable body（id={:?})",
                            callable.root_fqn(),
                            callable_id
                        ),
                    })?;
            if program_callable.root_fqn() != callable.root_fqn() {
                return Err(LlvmEmitError::Frontend {
                    message: format!(
                        "LLVM LIR stage 入口 `{}` 的 facts/body root 不一致（body `{}`）",
                        callable.root_fqn(),
                        program_callable.root_fqn()
                    ),
                });
            }
            Ok(Some(EntryRef::new(
                entry_source_id,
                callable_id,
                callable.root_fqn().to_string(),
                arg_shape,
            )))
        }
        count => Err(LlvmEmitError::AmbiguousEntryMain {
            entry: entry_main_fqn.unwrap_or("main").to_string(),
            count,
        }),
    }
}

fn classify_entry_main_arg_shape(
    types: &crate::ty::TypeStore,
    callable: &LirCallableFacts,
) -> Option<EntryMainArgShape> {
    if !matches!(
        types.kind(callable.return_ty),
        TypeKind::Value(ValueTypeKind::Unit | ValueTypeKind::Int)
    ) {
        return None;
    }

    match callable.param_tys.as_slice() {
        [] => Some(EntryMainArgShape::None),
        [param_ty] => match types.kind(*param_ty) {
            TypeKind::Ref(RefTypeKind::Nominal(nominal))
                if nominal.fqn == "scoop.core.Array"
                    && nominal.args.len() == 1
                    && matches!(
                        types.kind(nominal.args[0]),
                        TypeKind::Ref(RefTypeKind::String)
                    )
                    && nominal.eff.is_none() =>
            {
                Some(EntryMainArgShape::ArrayString)
            }
            _ => None,
        },
        _ => None,
    }
}

fn callable_source_name(root_fqn: &str) -> &str {
    root_fqn
        .rsplit_once('.')
        .map(|(_, name)| name)
        .unwrap_or(root_fqn)
}

/// Convert a cached dependency handoff into the same LIR artifact shape used by the primary cone.
pub fn lir_artifact_from_dep(dep: CachedDepArtifactHandoff) -> Result<LirArtifact, LlvmEmitError> {
    let (_cone_id, cone, program, facts, type_store, object_files) = dep.into_parts();
    let base_context =
        LlvmStageBaseContext::from_cached_dep_type_store(cone.clone(), &facts, type_store)?;
    LirArtifact::new(cone, program, facts, base_context, None, object_files)
}

fn lir_callable_index_emit_error(error: LirCallableIndexError) -> LlvmEmitError {
    LlvmEmitError::Frontend {
        message: format!("LLVM LIR stage callable id map 无效：{error}"),
    }
}
