use std::collections::HashMap;

use crate::effect_facts_stage::MaterializedEffectFacts;
use crate::effect_lowered::LateLoweredOptOptions;
use crate::effect_lowered::ordinary_callee::{
    EffectAnalysisFacts, EffectFieldFact, EffectFieldOwnerKind, EffectGlobalRootKind,
    EffectReflectionCall,
};
use crate::frontend::CodegenLoweringOutput;
use crate::hir::{self, LoweredHir};
use crate::llvm::{LlvmCallableSourceContract, LlvmEmitError, LlvmStageBaseContext};
use crate::mir::MaterializedMir;
use crate::session::Session;
use crate::source::SourceFile;
use scoopc_hir_facts::HirFacts;
use scoopc_hir_facts::declarations::{FieldOwnerKind, NominalKind};
use scoopc_hir_facts::globals::GlobalRootKind;

use super::{HirStageOutput, LirArtifact, mir_stage};

fn build_callable_source_contracts(
    top_level_funs: &[hir::FunDecl],
    member_funs: &[hir::FunDecl],
) -> HashMap<String, LlvmCallableSourceContract> {
    top_level_funs
        .iter()
        .chain(member_funs.iter())
        .map(|fun| {
            (
                fun.fqn.clone(),
                LlvmCallableSourceContract {
                    source_path: fun.source_path.clone(),
                    span: fun.span,
                },
            )
        })
        .collect()
}

pub(crate) fn build_ordinary_callee_effect_analysis_facts(facts: &HirFacts) -> EffectAnalysisFacts {
    let global_roots = facts
        .globals
        .roots
        .iter()
        .map(|root| {
            let kind = match root.kind {
                GlobalRootKind::TopLevelVal => EffectGlobalRootKind::TopLevelVal,
                GlobalRootKind::TopLevelVar => EffectGlobalRootKind::TopLevelVar,
                GlobalRootKind::ObjectSingleton => EffectGlobalRootKind::ObjectSingleton,
            };
            (root.identity.display_name.clone(), (kind, root.ty))
        })
        .collect();
    let fields = facts
        .declarations
        .fields
        .iter()
        .map(|field| EffectFieldFact {
            owner_kind: match field.owner_kind {
                FieldOwnerKind::Struct => EffectFieldOwnerKind::Struct,
                FieldOwnerKind::Class => EffectFieldOwnerKind::Class,
                FieldOwnerKind::Object => EffectFieldOwnerKind::Object,
                FieldOwnerKind::EnumVariant => EffectFieldOwnerKind::EnumVariant,
            },
            owner: field.owner.as_str().to_string(),
            fqn: field.identity.display_name.clone(),
            ty: field.ty,
        })
        .collect();
    let callable_return_tys = facts
        .declarations
        .callables
        .iter()
        .map(|callable| (callable.identity.display_name.clone(), callable.return_ty))
        .collect();
    let nominal_supertypes = facts
        .declarations
        .nominals
        .iter()
        .filter(|nominal| nominal.kind == NominalKind::Class)
        .map(|nominal| {
            (
                nominal.identity.display_name.clone(),
                nominal
                    .direct_supertypes
                    .iter()
                    .map(|key| key.as_str().to_string())
                    .collect(),
            )
        })
        .collect();
    let reflection_calls = facts
        .source_sites
        .call_sites
        .iter()
        .filter_map(|site| match &site.contract {
            scoopc_hir_facts::source_sites::CallSiteContractKind::Intrinsic {
                kind: scoopc_hir_facts::source_sites::IntrinsicKind::Reflection { name },
                function,
            } => Some((
                crate::effect_lowered::source::CallSite::new(
                    site.identity.source_path.clone(),
                    site.identity.span,
                ),
                EffectReflectionCall {
                    intrinsic_name: name.clone(),
                    type_args: function.type_args.clone(),
                },
            )),
            _ => None,
        })
        .collect();
    EffectAnalysisFacts::from_parts(
        global_roots,
        fields,
        callable_return_tys,
        nominal_supertypes,
        HashMap::new(),
        reflection_calls,
        HashMap::new(),
    )
}

fn build_llvm_stage_base_context_from_lowered_hir(
    lowered_hir: LoweredHir,
    hir_facts: HirFacts,
    materialized_mir: &MaterializedMir,
    effect_facts: MaterializedEffectFacts,
) -> LlvmStageBaseContext {
    let top_level_funs: Vec<hir::FunDecl> = lowered_hir
        .file
        .items
        .into_iter()
        .filter_map(|item| match item {
            hir::Item::Fun(fun) => Some(fun),
            _ => None,
        })
        .collect();
    let effect_analysis_facts = build_ordinary_callee_effect_analysis_facts(&hir_facts);
    let stable_cone_key = materialized_mir.stable_cone_key().clone();
    let types = effect_facts.types().clone();
    let contracts = materialized_mir.backend_contracts();
    let mut enum_layouts = contracts.enum_layouts.clone();
    for (key, value) in lowered_hir.enum_layouts {
        enum_layouts.entry(key).or_insert(value);
    }
    let mut top_level_vars = contracts.top_level_vars.clone();
    for (key, value) in lowered_hir.top_level_vars {
        top_level_vars.entry(key).or_insert(value);
    }
    let mut top_level_immutable_values = contracts.top_level_immutable_values.clone();
    for (key, value) in lowered_hir.top_level_immutable_values {
        top_level_immutable_values.entry(key).or_insert(value);
    }
    let mut object_inits = contracts.object_inits.clone();
    for (key, value) in lowered_hir.object_inits {
        object_inits.entry(key).or_insert(value);
    }
    let mut class_inits = contracts.class_inits.clone();
    for (key, value) in lowered_hir.class_inits {
        class_inits.entry(key).or_insert(value);
    }
    let release_hooks = lowered_hir.release_hooks;
    let mut extern_funs = contracts.extern_funs.clone();
    for (key, value) in lowered_hir.extern_funs {
        extern_funs.entry(key).or_insert(value);
    }
    let mut native_callable_funs = contracts.native_callable_funs.clone();
    for (key, value) in lowered_hir.native_callable_funs {
        native_callable_funs.entry(key).or_insert(value);
    }
    let callable_sources =
        build_callable_source_contracts(&top_level_funs, &lowered_hir.member_funs);

    LlvmStageBaseContext::new(
        lowered_hir.source_cones,
        lowered_hir.stable_type_param_keys,
        &materialized_mir.types,
        types,
        stable_cone_key,
        lowered_hir.struct_layouts,
        enum_layouts,
        top_level_vars,
        top_level_immutable_values,
        object_inits,
        class_inits,
        release_hooks,
        lowered_hir.when_pat_binding_tys,
        lowered_hir.nominal_kinds,
        lowered_hir.interior_mutable_nominals,
        lowered_hir.builtins,
        callable_sources,
        extern_funs,
        native_callable_funs,
        effect_analysis_facts,
    )
}

pub(crate) fn build_lir_artifact(
    session: &Session,
    entry_source: &SourceFile,
    lowered: CodegenLoweringOutput,
    preserve_published_resume_shells: bool,
) -> Result<LirArtifact, LlvmEmitError> {
    let (lowered_hir, materialized_mir, frontend_index, type_env) = lowered.into_parts();
    let source_path = entry_source.path().to_path_buf();
    let base_hir = lowered_hir.clone();
    let typed_hir_output = HirStageOutput::new_with_frontend_artifact(
        lowered_hir,
        &source_path,
        frontend_index,
        type_env,
    )
    .map_err(crate::hir::HirLowerError::from)
    .map_err(|err| stage_error("HIR stage", err))?;
    let hir_facts = typed_hir_output.hir_facts().clone();
    let mir_stage_output = mir_stage::run(typed_hir_output)
        .map_err(|err| stage_error("direct-style MIR", err))?
        .with_materialized_mir(materialized_mir);
    let effect_facts_stage_output =
        super::build_effect_facts_stage_output(session, entry_source, &mir_stage_output)
            .map_err(|err| stage_error("effect facts", err))?;
    let opt_options = if preserve_published_resume_shells {
        LateLoweredOptOptions::preserve_published_resume_shells()
    } else {
        LateLoweredOptOptions::default()
    };
    let output = super::effect_lowering_stage::build_lir_stage_output_from_stage_outputs(
        &mir_stage_output,
        &effect_facts_stage_output,
        opt_options,
    )
    .map_err(|err| stage_error("late lowering", err))?;
    let (_direct_style, materialized_mir) = mir_stage_output.into_parts();
    let cone = materialized_mir.stable_cone_key().clone();
    let effect_facts = effect_facts_stage_output.into_effect_facts();
    let base_context = build_llvm_stage_base_context_from_lowered_hir(
        base_hir,
        hir_facts,
        &materialized_mir,
        effect_facts,
    );
    base_context.verify_lir_type_context(output.lir_facts(), "primary")?;
    let (program, facts) = output.into_parts();
    LirArtifact::new(
        cone,
        program,
        facts,
        base_context,
        Some(materialized_mir),
        Vec::new(),
    )
}

fn stage_error(stage: &'static str, error: impl std::fmt::Display) -> LlvmEmitError {
    LlvmEmitError::Frontend {
        message: format!("LLVM stage `{stage}` 失败：{error}"),
    }
}
