use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use crate::cone::SourceConeInfo;
use crate::effect_lowered::LateLoweredProgram;
use crate::effect_lowered::ir::LateLoweredClassCtorInitBody;
use crate::effect_lowered::ordinary_callee::EffectAnalysisFacts;
use crate::effect_lowered::source as source_payload;
use crate::source::{SourceId, SourceMap};
use crate::span::Span;
use crate::stable_id::{StableConeKey, StableTypeParamKey};
use crate::ty::{BuiltinTypes, TypeId, TypeParamType, TypeStore};
use scoopc_lir_facts::{
    LirCallSiteKind, LirFacts, LirTypeContextOwner, LirTypeStableWireFormatDecision,
};

use super::LlvmEmitError;

/// LLVM/backend 仍需的显式 base context。
///
/// `LirFacts.type_context.owner` 指向这个 context：当前 TypeId 仍是同进程
/// effect-owned `TypeStore` identity，cross-process stable wire format 显式推迟到
/// per-cone build artifact serialization。这里不嵌套 HIR/MIR/effect stage wrapper，
/// 只保留 LLVM backend 仍需的显式窄 base contracts。
#[derive(Debug)]
pub struct LlvmStageBaseContext {
    source_cones: HashMap<PathBuf, SourceConeInfo>,
    stable_type_param_keys: HashMap<TypeParamType, StableTypeParamKey>,
    types: TypeStore,
    stable_cone_key: StableConeKey,
    materialized_type_fingerprint: String,
    effect_type_fingerprint: String,
    struct_layouts: source_payload::StructLayoutIndex,
    enum_layouts: source_payload::EnumLayoutIndex,
    top_level_vars: source_payload::TopLevelVarIndex,
    top_level_immutable_values: source_payload::TopLevelImmutableValueIndex,
    top_level_fun_call_sites: source_payload::TopLevelFunCallSiteIndex,
    object_inits: source_payload::ObjectInitIndex,
    class_inits: source_payload::ClassInitIndex,
    class_ctor_init_bodies: HashMap<String, LateLoweredClassCtorInitBody>,
    class_vtables: source_payload::ClassVtableIndex,
    interfaces: source_payload::InterfaceIndex,
    class_itables: source_payload::ClassItableIndex,
    ctor_call_sites: source_payload::CtorCallSiteIndex,
    dispatch_call_contracts: HashMap<LlvmDispatchCallKey, LirCallSiteKind>,
    effect_op_call_sites: source_payload::EffectOpCallSiteIndex,
    continuation_resume_call_sites: source_payload::ContinuationResumeCallSiteIndex,
    when_pat_binding_tys: source_payload::WhenPatBindingTypeIndex,
    nominal_kinds: source_payload::NominalKindIndex,
    direct_supertypes: source_payload::DirectSupertypesIndex,
    builtins: BuiltinTypes,
    callable_sources: HashMap<String, LlvmCallableSourceContract>,
    extern_funs: source_payload::ExternFunIndex,
    native_callable_funs: source_payload::NativeCallableFunIndex,
    effect_analysis_facts: Rc<EffectAnalysisFacts>,
}

#[derive(Debug, Clone)]
pub struct LlvmCallableSourceContract {
    pub source_path: PathBuf,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LlvmDispatchCallKey {
    pub source_path: PathBuf,
    pub span: Span,
    pub receiver_ty: TypeId,
}

impl LlvmDispatchCallKey {
    pub fn new(source_path: impl Into<PathBuf>, span: Span, receiver_ty: TypeId) -> Self {
        Self {
            source_path: source_path.into(),
            span,
            receiver_ty,
        }
    }
}

impl LlvmStageBaseContext {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source_cones: HashMap<PathBuf, SourceConeInfo>,
        stable_type_param_keys: HashMap<TypeParamType, StableTypeParamKey>,
        materialized_types: &TypeStore,
        types: TypeStore,
        stable_cone_key: StableConeKey,
        struct_layouts: source_payload::StructLayoutIndex,
        enum_layouts: source_payload::EnumLayoutIndex,
        top_level_vars: source_payload::TopLevelVarIndex,
        top_level_immutable_values: source_payload::TopLevelImmutableValueIndex,
        top_level_fun_call_sites: source_payload::TopLevelFunCallSiteIndex,
        object_inits: source_payload::ObjectInitIndex,
        class_inits: source_payload::ClassInitIndex,
        class_ctor_init_bodies: HashMap<String, LateLoweredClassCtorInitBody>,
        class_vtables: source_payload::ClassVtableIndex,
        interfaces: source_payload::InterfaceIndex,
        class_itables: source_payload::ClassItableIndex,
        ctor_call_sites: source_payload::CtorCallSiteIndex,
        dispatch_call_contracts: HashMap<LlvmDispatchCallKey, LirCallSiteKind>,
        effect_op_call_sites: source_payload::EffectOpCallSiteIndex,
        continuation_resume_call_sites: source_payload::ContinuationResumeCallSiteIndex,
        when_pat_binding_tys: source_payload::WhenPatBindingTypeIndex,
        nominal_kinds: source_payload::NominalKindIndex,
        direct_supertypes: source_payload::DirectSupertypesIndex,
        builtins: BuiltinTypes,
        callable_sources: HashMap<String, LlvmCallableSourceContract>,
        extern_funs: source_payload::ExternFunIndex,
        native_callable_funs: source_payload::NativeCallableFunIndex,
        effect_analysis_facts: EffectAnalysisFacts,
    ) -> Self {
        let materialized_type_fingerprint = type_store_fingerprint(materialized_types);
        let effect_type_fingerprint = type_store_fingerprint(&types);
        Self {
            source_cones,
            stable_type_param_keys,
            types,
            stable_cone_key,
            materialized_type_fingerprint,
            effect_type_fingerprint,
            struct_layouts,
            enum_layouts,
            top_level_vars,
            top_level_immutable_values,
            top_level_fun_call_sites,
            object_inits,
            class_inits,
            class_ctor_init_bodies,
            class_vtables,
            interfaces,
            class_itables,
            ctor_call_sites,
            dispatch_call_contracts,
            effect_op_call_sites,
            continuation_resume_call_sites,
            when_pat_binding_tys,
            nominal_kinds,
            direct_supertypes,
            builtins,
            callable_sources,
            extern_funs,
            native_callable_funs,
            effect_analysis_facts: Rc::new(effect_analysis_facts),
        }
    }

    pub fn into_type_store(self) -> TypeStore {
        self.types
    }

    pub fn stable_cone_key(&self) -> &StableConeKey {
        &self.stable_cone_key
    }

    pub fn source_cones(&self) -> &HashMap<PathBuf, SourceConeInfo> {
        &self.source_cones
    }

    pub fn stable_type_param_keys(&self) -> &HashMap<TypeParamType, StableTypeParamKey> {
        &self.stable_type_param_keys
    }

    pub fn types(&self) -> &TypeStore {
        &self.types
    }

    pub fn struct_layouts(&self) -> &source_payload::StructLayoutIndex {
        &self.struct_layouts
    }

    pub fn enum_layouts(&self) -> &source_payload::EnumLayoutIndex {
        &self.enum_layouts
    }

    pub fn top_level_vars(&self) -> &source_payload::TopLevelVarIndex {
        &self.top_level_vars
    }

    pub fn top_level_immutable_values(&self) -> &source_payload::TopLevelImmutableValueIndex {
        &self.top_level_immutable_values
    }

    pub fn top_level_fun_call_sites(&self) -> &source_payload::TopLevelFunCallSiteIndex {
        &self.top_level_fun_call_sites
    }

    pub fn object_inits(&self) -> &source_payload::ObjectInitIndex {
        &self.object_inits
    }

    pub fn class_inits(&self) -> &source_payload::ClassInitIndex {
        &self.class_inits
    }

    pub fn class_ctor_init_bodies(&self) -> &HashMap<String, LateLoweredClassCtorInitBody> {
        &self.class_ctor_init_bodies
    }

    pub fn class_vtables(&self) -> &source_payload::ClassVtableIndex {
        &self.class_vtables
    }

    pub fn interfaces(&self) -> &source_payload::InterfaceIndex {
        &self.interfaces
    }

    pub fn class_itables(&self) -> &source_payload::ClassItableIndex {
        &self.class_itables
    }

    pub fn ctor_call_sites(&self) -> &source_payload::CtorCallSiteIndex {
        &self.ctor_call_sites
    }

    pub fn dispatch_call_contracts(&self) -> &HashMap<LlvmDispatchCallKey, LirCallSiteKind> {
        &self.dispatch_call_contracts
    }

    pub fn effect_op_call_sites(&self) -> &source_payload::EffectOpCallSiteIndex {
        &self.effect_op_call_sites
    }

    pub fn continuation_resume_call_sites(
        &self,
    ) -> &source_payload::ContinuationResumeCallSiteIndex {
        &self.continuation_resume_call_sites
    }

    pub fn when_pat_binding_tys(&self) -> &source_payload::WhenPatBindingTypeIndex {
        &self.when_pat_binding_tys
    }

    pub fn nominal_kinds(&self) -> &source_payload::NominalKindIndex {
        &self.nominal_kinds
    }

    pub fn direct_supertypes(&self) -> &source_payload::DirectSupertypesIndex {
        &self.direct_supertypes
    }

    pub fn builtins(&self) -> BuiltinTypes {
        self.builtins
    }

    pub fn callable_sources(&self) -> &HashMap<String, LlvmCallableSourceContract> {
        &self.callable_sources
    }

    pub fn extern_funs(&self) -> &source_payload::ExternFunIndex {
        &self.extern_funs
    }

    pub fn native_callable_funs(&self) -> &source_payload::NativeCallableFunIndex {
        &self.native_callable_funs
    }

    pub fn effect_analysis_facts(&self) -> Rc<EffectAnalysisFacts> {
        Rc::clone(&self.effect_analysis_facts)
    }

    pub fn verify_lir_type_context(
        &self,
        facts: &LirFacts,
        role: &'static str,
    ) -> Result<(), LlvmEmitError> {
        verify_lir_type_context_header(facts, role)?;

        if facts.type_context.materialized_fingerprint != self.materialized_type_fingerprint {
            return Err(LlvmEmitError::Frontend {
                message: format!(
                    "LLVM stage {role} LIR facts materialized TypeStore fingerprint 与 LlvmStageBaseContext 不一致"
                ),
            });
        }

        if facts.type_context.effect_facts_fingerprint != self.effect_type_fingerprint {
            return Err(LlvmEmitError::Frontend {
                message: format!(
                    "LLVM stage {role} LIR facts effect TypeStore fingerprint 与 LlvmStageBaseContext 不一致"
                ),
            });
        }

        Self::verify_lir_type_store_owner(self.types(), facts, role)
    }

    pub fn verify_lir_type_store_owner(
        types: &TypeStore,
        facts: &LirFacts,
        role: &'static str,
    ) -> Result<(), LlvmEmitError> {
        verify_lir_type_context_header(facts, role)?;

        let effect_facts_fingerprint = type_store_fingerprint(types);
        if facts.type_context.effect_facts_fingerprint != effect_facts_fingerprint {
            return Err(LlvmEmitError::Frontend {
                message: format!(
                    "LLVM stage {role} LIR facts effect TypeStore fingerprint 与 handoff TypeStore owner 不一致"
                ),
            });
        }

        Ok(())
    }
}

fn verify_lir_type_context_header(
    facts: &LirFacts,
    role: &'static str,
) -> Result<(), LlvmEmitError> {
    if facts.type_context.owner != LirTypeContextOwner::LirStageBaseContext {
        return Err(LlvmEmitError::Frontend {
            message: format!(
                "LLVM stage {role} LIR facts 使用了非 LlvmStageBaseContext type owner: {:?}",
                facts.type_context.owner
            ),
        });
    }
    if facts.type_context.stable_wire_format.decision != LirTypeStableWireFormatDecision::Deferred
        || facts.type_context.stable_wire_format.owner.is_empty()
    {
        return Err(LlvmEmitError::Frontend {
            message: format!("LLVM stage {role} LIR facts 缺少 TypeId wire-format 推迟决策记录"),
        });
    }
    Ok(())
}

fn type_store_fingerprint(types: &TypeStore) -> String {
    let mut entries = types
        .iter_ids()
        .map(|ty| format!("t{}={}", ty.as_u32(), types.display(ty)))
        .collect::<Vec<_>>();
    entries.sort();
    entries.join("|")
}

/// LLVM codegen stage 的稳定 handoff。
///
/// `.ll/.o/.s` 三类产物都消费同一份 `LIR + LIR facts + LlvmStageBaseContext`，
/// ABI visibility 只额外携带 request-source LIR/LIR facts/TypeStore，不再嵌套 P5 wrapper。
#[derive(Debug)]
pub struct LlvmCodegenStageOutput {
    source_map: SourceMap,
    entry_source_id: SourceId,
    entry_main_fqn: Option<String>,
    opt_level: crate::opt::OptLevel,
    base_context: LlvmStageBaseContext,
    lir: LateLoweredProgram,
    lir_facts: LirFacts,
    abi_visibility_lir: Option<LateLoweredProgram>,
    abi_visibility_lir_facts: Option<LirFacts>,
    abi_visibility_types: Option<TypeStore>,
}

impl LlvmCodegenStageOutput {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source_map: SourceMap,
        entry_source_id: SourceId,
        entry_main_fqn: Option<String>,
        opt_level: crate::opt::OptLevel,
        base_context: LlvmStageBaseContext,
        lir: LateLoweredProgram,
        lir_facts: LirFacts,
        abi_visibility_lir: Option<LateLoweredProgram>,
        abi_visibility_lir_facts: Option<LirFacts>,
        abi_visibility_types: Option<TypeStore>,
    ) -> Self {
        Self {
            source_map,
            entry_source_id,
            entry_main_fqn,
            opt_level,
            base_context,
            lir,
            lir_facts,
            abi_visibility_lir,
            abi_visibility_lir_facts,
            abi_visibility_types,
        }
    }

    pub fn source_map(&self) -> &SourceMap {
        &self.source_map
    }

    pub fn entry_source_id(&self) -> SourceId {
        self.entry_source_id
    }

    pub fn entry_main_fqn(&self) -> Option<&str> {
        self.entry_main_fqn.as_deref()
    }

    pub fn opt_level(&self) -> crate::opt::OptLevel {
        self.opt_level
    }

    pub fn base_context(&self) -> &LlvmStageBaseContext {
        &self.base_context
    }

    pub fn lir(&self) -> &LateLoweredProgram {
        &self.lir
    }

    pub fn lir_facts(&self) -> &LirFacts {
        &self.lir_facts
    }

    pub fn abi_visibility_lir(&self) -> Option<&LateLoweredProgram> {
        self.abi_visibility_lir.as_ref()
    }

    pub fn abi_visibility_lir_facts(&self) -> Option<&LirFacts> {
        self.abi_visibility_lir_facts.as_ref()
    }

    pub fn abi_visibility_types(&self) -> Option<&TypeStore> {
        self.abi_visibility_types.as_ref()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LlvmArtifactKind {
    LlvmIr,
    Object,
    Asm,
}
