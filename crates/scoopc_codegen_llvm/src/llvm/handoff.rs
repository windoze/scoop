use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use crate::cone::SourceConeInfo;
use crate::effect_lowered::LateLoweredProgram;
use crate::effect_lowered::ordinary_callee::EffectAnalysisFacts;
use crate::effect_lowered::source as source_payload;
use crate::source::{SourceId, SourceMap};
use crate::stable_id::{StableConeKey, StableTypeParamKey};
use crate::ty::{BuiltinTypes, TypeParamType, TypeStore};
use scoop_project_model::ConeId;
use scoopc_ids::LirCallableId;
use scoopc_lir_facts::{LirTypeContextFacts, LirTypeContextOwner, LirTypeStableWireFormatDecision};

use super::LlvmEmitError;

/// Source-level entry main argument shape resolved before LLVM emit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryMainArgShape {
    None,
    ArrayString,
}

/// Resolved executable entry callable carried across the LIR/codegen boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryRef {
    source_id: SourceId,
    callable_id: LirCallableId,
    root_fqn: String,
    arg_shape: EntryMainArgShape,
}

impl EntryRef {
    pub fn new(
        source_id: SourceId,
        callable_id: LirCallableId,
        root_fqn: impl Into<String>,
        arg_shape: EntryMainArgShape,
    ) -> Self {
        Self {
            source_id,
            callable_id,
            root_fqn: root_fqn.into(),
            arg_shape,
        }
    }

    pub fn source_id(&self) -> SourceId {
        self.source_id
    }

    pub fn callable_id(&self) -> LirCallableId {
        self.callable_id
    }

    pub fn root_fqn(&self) -> &str {
        &self.root_fqn
    }

    pub fn arg_shape(&self) -> EntryMainArgShape {
        self.arg_shape
    }
}

/// Deserialized cached dependency cone payload handed to the LLVM stage.
///
/// This is intentionally codegen-owned: `scoopc_cone` reads the artifact format,
/// while LLVM receives only the already decoded LIR/type-store contract and
/// object paths it may later pass to the linker.
#[derive(Debug, Clone)]
pub struct CachedDepArtifactHandoff {
    cone_id: ConeId,
    stable_cone_key: StableConeKey,
    lir: LateLoweredProgram,
    type_store: TypeStore,
    object_files: Vec<PathBuf>,
}

impl CachedDepArtifactHandoff {
    pub fn new(
        cone_id: ConeId,
        stable_cone_key: StableConeKey,
        lir: LateLoweredProgram,
        type_store: TypeStore,
        object_files: Vec<PathBuf>,
    ) -> Self {
        Self {
            cone_id,
            stable_cone_key,
            lir,
            type_store,
            object_files,
        }
    }

    pub fn cone_id(&self) -> ConeId {
        self.cone_id
    }

    pub fn stable_cone_key(&self) -> &StableConeKey {
        &self.stable_cone_key
    }

    pub fn lir(&self) -> &LateLoweredProgram {
        &self.lir
    }

    pub fn type_store(&self) -> &TypeStore {
        &self.type_store
    }

    pub fn object_files(&self) -> &[PathBuf] {
        &self.object_files
    }

    pub fn into_parts(
        self,
    ) -> (
        ConeId,
        StableConeKey,
        LateLoweredProgram,
        TypeStore,
        Vec<PathBuf>,
    ) {
        (
            self.cone_id,
            self.stable_cone_key,
            self.lir,
            self.type_store,
            self.object_files,
        )
    }
}

/// Dependency LIR payload consumed by LLVM codegen after pipeline-level LIR adaptation.
#[derive(Debug, Clone)]
pub struct LlvmDepLirArtifactHandoff {
    stable_cone_key: StableConeKey,
    lir: LateLoweredProgram,
    type_store: TypeStore,
    object_files: Vec<PathBuf>,
}

impl LlvmDepLirArtifactHandoff {
    pub fn new(
        stable_cone_key: StableConeKey,
        lir: LateLoweredProgram,
        type_store: TypeStore,
        object_files: Vec<PathBuf>,
    ) -> Self {
        Self {
            stable_cone_key,
            lir,
            type_store,
            object_files,
        }
    }

    pub fn stable_cone_key(&self) -> &StableConeKey {
        &self.stable_cone_key
    }

    pub fn lir(&self) -> &LateLoweredProgram {
        &self.lir
    }

    pub fn type_store(&self) -> &TypeStore {
        &self.type_store
    }

    pub fn object_files(&self) -> &[PathBuf] {
        &self.object_files
    }
}

/// LLVM/backend 仍需的显式 base context。
///
/// `LateLoweredProgram.type_context.owner` 指向这个 context：per-cone artifacts 通过
/// portable `TypeStore` serialization 恢复跨进程 TypeId 语义；LLVM handoff 只消费
/// 当前进程内重建后的窄 base contracts，不嵌套 HIR/MIR/effect stage wrapper。
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
    object_inits: source_payload::ObjectInitIndex,
    class_inits: source_payload::ClassInitIndex,
    release_hooks: source_payload::ReleaseHookIndex,
    when_pat_binding_tys: source_payload::WhenPatBindingTypeIndex,
    nominal_kinds: source_payload::NominalKindIndex,
    interior_mutable_nominals: source_payload::InteriorMutableIndex,
    builtins: BuiltinTypes,
    callable_sources: HashMap<String, LlvmCallableSourceContract>,
    extern_funs: source_payload::ExternFunIndex,
    native_callable_funs: source_payload::NativeCallableFunIndex,
    effect_analysis_facts: Rc<EffectAnalysisFacts>,
}

#[derive(Debug, Clone)]
pub struct LlvmCallableSourceContract {
    pub source_path: PathBuf,
    pub span: crate::span::Span,
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
        object_inits: source_payload::ObjectInitIndex,
        class_inits: source_payload::ClassInitIndex,
        release_hooks: source_payload::ReleaseHookIndex,
        when_pat_binding_tys: source_payload::WhenPatBindingTypeIndex,
        nominal_kinds: source_payload::NominalKindIndex,
        interior_mutable_nominals: source_payload::InteriorMutableIndex,
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
            object_inits,
            class_inits,
            release_hooks,
            when_pat_binding_tys,
            nominal_kinds,
            interior_mutable_nominals,
            builtins,
            callable_sources,
            extern_funs,
            native_callable_funs,
            effect_analysis_facts: Rc::new(effect_analysis_facts),
        }
    }

    /// Rebuild the narrow base context available for a cached dependency cone.
    pub fn from_cached_dep_type_store(
        stable_cone_key: StableConeKey,
        lir: &LateLoweredProgram,
        types: TypeStore,
    ) -> Result<Self, LlvmEmitError> {
        Self::verify_lir_type_store_owner(&types, lir, "cached dependency")?;
        let builtins = types.builtins().ok_or_else(|| LlvmEmitError::Frontend {
            message: format!(
                "cached dependency cone {}@{} TypeStore 缺少 builtin 类型",
                stable_cone_key.name(),
                stable_cone_key.version()
            ),
        })?;

        Ok(Self {
            source_cones: HashMap::new(),
            stable_type_param_keys: HashMap::new(),
            types,
            stable_cone_key,
            materialized_type_fingerprint: lir.type_context().materialized_fingerprint.clone(),
            effect_type_fingerprint: lir.type_context().effect_facts_fingerprint.clone(),
            struct_layouts: source_payload::StructLayoutIndex::default(),
            enum_layouts: source_payload::EnumLayoutIndex::default(),
            top_level_vars: source_payload::TopLevelVarIndex::default(),
            top_level_immutable_values: source_payload::TopLevelImmutableValueIndex::default(),
            object_inits: source_payload::ObjectInitIndex::default(),
            class_inits: source_payload::ClassInitIndex::default(),
            release_hooks: source_payload::ReleaseHookIndex::default(),
            when_pat_binding_tys: source_payload::WhenPatBindingTypeIndex::default(),
            nominal_kinds: source_payload::NominalKindIndex::default(),
            interior_mutable_nominals: source_payload::InteriorMutableIndex::default(),
            builtins,
            callable_sources: HashMap::new(),
            extern_funs: source_payload::ExternFunIndex::default(),
            native_callable_funs: source_payload::NativeCallableFunIndex::default(),
            effect_analysis_facts: Rc::new(EffectAnalysisFacts::default()),
        })
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

    pub fn object_inits(&self) -> &source_payload::ObjectInitIndex {
        &self.object_inits
    }

    pub fn class_inits(&self) -> &source_payload::ClassInitIndex {
        &self.class_inits
    }

    pub fn release_hooks(&self) -> &source_payload::ReleaseHookIndex {
        &self.release_hooks
    }

    pub fn when_pat_binding_tys(&self) -> &source_payload::WhenPatBindingTypeIndex {
        &self.when_pat_binding_tys
    }

    pub fn nominal_kinds(&self) -> &source_payload::NominalKindIndex {
        &self.nominal_kinds
    }

    pub fn interior_mutable_nominals(&self) -> &source_payload::InteriorMutableIndex {
        &self.interior_mutable_nominals
    }

    pub fn nominal_is_interior_mutable(&self, fqn: &str) -> bool {
        self.interior_mutable_nominals.contains(fqn)
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
        lir: &LateLoweredProgram,
        role: &'static str,
    ) -> Result<(), LlvmEmitError> {
        let type_context = lir.type_context();
        verify_lir_type_context_header(type_context, role)?;

        if type_context.materialized_fingerprint != self.materialized_type_fingerprint {
            return Err(LlvmEmitError::Frontend {
                message: format!(
                    "LLVM stage {role} LIR program materialized TypeStore fingerprint 与 LlvmStageBaseContext 不一致"
                ),
            });
        }

        if type_context.effect_facts_fingerprint != self.effect_type_fingerprint {
            return Err(LlvmEmitError::Frontend {
                message: format!(
                    "LLVM stage {role} LIR program effect TypeStore fingerprint 与 LlvmStageBaseContext 不一致"
                ),
            });
        }

        Self::verify_lir_type_store_owner(self.types(), lir, role)
    }

    pub fn verify_lir_type_store_owner(
        types: &TypeStore,
        lir: &LateLoweredProgram,
        role: &'static str,
    ) -> Result<(), LlvmEmitError> {
        let type_context = lir.type_context();
        verify_lir_type_context_header(type_context, role)?;

        let effect_facts_fingerprint = type_store_fingerprint(types);
        if type_context.effect_facts_fingerprint != effect_facts_fingerprint {
            return Err(LlvmEmitError::Frontend {
                message: format!(
                    "LLVM stage {role} LIR program effect TypeStore fingerprint 与 handoff TypeStore owner 不一致"
                ),
            });
        }

        Ok(())
    }
}

fn verify_lir_type_context_header(
    type_context: &LirTypeContextFacts,
    role: &'static str,
) -> Result<(), LlvmEmitError> {
    if type_context.owner != LirTypeContextOwner::LirStageBaseContext {
        return Err(LlvmEmitError::Frontend {
            message: format!(
                "LLVM stage {role} LIR program 使用了非 LlvmStageBaseContext type owner: {:?}",
                type_context.owner
            ),
        });
    }
    if type_context.stable_wire_format.decision != LirTypeStableWireFormatDecision::Implemented
        || type_context.stable_wire_format.owner.is_empty()
    {
        return Err(LlvmEmitError::Frontend {
            message: format!(
                "LLVM stage {role} LIR program 缺少 TypeId portable wire-format 实现记录"
            ),
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
/// `.ll/.o/.s` 三类产物都消费同一份 `LIR + LlvmStageBaseContext`，
/// ABI visibility 只额外携带 request-source LIR/TypeStore，不再嵌套 P5 wrapper。
#[derive(Debug)]
pub struct LlvmCodegenStageOutput {
    source_map: SourceMap,
    entry_source_id: SourceId,
    entry: Option<EntryRef>,
    opt_level: crate::opt::OptLevel,
    base_context: LlvmStageBaseContext,
    lir: LateLoweredProgram,
    abi_visibility_lir: Option<LateLoweredProgram>,
    abi_visibility_types: Option<TypeStore>,
    cached_dep_artifacts: Vec<LlvmDepLirArtifactHandoff>,
}

impl LlvmCodegenStageOutput {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source_map: SourceMap,
        entry_source_id: SourceId,
        entry: Option<EntryRef>,
        opt_level: crate::opt::OptLevel,
        base_context: LlvmStageBaseContext,
        lir: LateLoweredProgram,
        abi_visibility_lir: Option<LateLoweredProgram>,
        abi_visibility_types: Option<TypeStore>,
        cached_dep_artifacts: Vec<LlvmDepLirArtifactHandoff>,
    ) -> Self {
        Self {
            source_map,
            entry_source_id,
            entry,
            opt_level,
            base_context,
            lir,
            abi_visibility_lir,
            abi_visibility_types,
            cached_dep_artifacts,
        }
    }

    pub fn source_map(&self) -> &SourceMap {
        &self.source_map
    }

    pub fn entry_source_id(&self) -> SourceId {
        self.entry_source_id
    }

    pub fn entry_ref(&self) -> Option<&EntryRef> {
        self.entry.as_ref()
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

    pub fn abi_visibility_lir(&self) -> Option<&LateLoweredProgram> {
        self.abi_visibility_lir.as_ref()
    }

    pub fn abi_visibility_types(&self) -> Option<&TypeStore> {
        self.abi_visibility_types.as_ref()
    }

    pub fn cached_dep_artifacts(&self) -> &[LlvmDepLirArtifactHandoff] {
        &self.cached_dep_artifacts
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LlvmArtifactKind {
    LlvmIr,
    Object,
    Asm,
}
