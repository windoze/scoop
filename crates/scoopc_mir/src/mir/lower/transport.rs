//! Top-level fun param/return collection, MIR transport kind helpers, erasure/boxing reasons, decl metadata lowering.

#![allow(dead_code)]

use super::*;

pub(in crate::mir::lower) fn collect_top_level_fun_return_tys(
    file: &hir::File,
    member_funs: &[hir::FunDecl],
) -> HashMap<String, TypeId> {
    let mut return_tys = HashMap::new();
    for item in &file.items {
        if let hir::Item::Fun(fun) = item {
            return_tys.insert(fun.fqn.clone(), fun.return_ty);
        }
    }
    for fun in member_funs {
        return_tys.insert(fun.fqn.clone(), fun.return_ty);
    }
    return_tys
}

pub(in crate::mir::lower) fn collect_top_level_fun_param_tys(
    file: &hir::File,
    member_funs: &[hir::FunDecl],
) -> HashMap<String, Vec<TypeId>> {
    let mut param_tys = HashMap::new();
    for item in &file.items {
        if let hir::Item::Fun(fun) = item {
            param_tys.insert(
                fun.fqn.clone(),
                fun.params.iter().map(|param| param.ty).collect(),
            );
        }
    }
    for fun in member_funs {
        param_tys.insert(
            fun.fqn.clone(),
            fun.params.iter().map(|param| param.ty).collect(),
        );
    }
    param_tys
}

pub(in crate::mir::lower) fn mir_transport_kind_for_ty(
    types: &TypeStore,
    facts: &MirLoweringFacts,
    ty: TypeId,
) -> MirTransportKind {
    match types.kind(ty) {
        TypeKind::Ref(RefTypeKind::Function(_)) => MirTransportKind::FunctionValue,
        TypeKind::Ref(_) => MirTransportKind::Reference,
        TypeKind::Value(ValueTypeKind::Tuple(_)) => MirTransportKind::Tuple,
        TypeKind::Value(ValueTypeKind::Option(_)) => MirTransportKind::EnumPayload,
        TypeKind::Value(ValueTypeKind::Nominal(_))
            if is_builtin_scalar_nominal_value_type(types, ty) =>
        {
            MirTransportKind::Scalar
        }
        TypeKind::Value(ValueTypeKind::Nominal(nominal))
            if facts.nominal_kind(&nominal.fqn) == Some(ast::TypeKind::Enum) =>
        {
            MirTransportKind::EnumPayload
        }
        TypeKind::Value(ValueTypeKind::Nominal(_)) => MirTransportKind::Struct,
        TypeKind::Value(_) => MirTransportKind::Scalar,
        TypeKind::Param(_) | TypeKind::StarProjection(_) => MirTransportKind::Unknown,
    }
}

pub(in crate::mir::lower) fn mir_type_requires_trace(types: &TypeStore, ty: TypeId) -> bool {
    crate::mir::transport::mir_transport_trace_requirement_for_type(types, ty)
}

pub(crate) fn mir_is_aggregate_transport_ty(types: &TypeStore, ty: TypeId) -> bool {
    match types.kind(ty) {
        TypeKind::Value(ValueTypeKind::Tuple(_) | ValueTypeKind::Option(_)) => true,
        TypeKind::Value(ValueTypeKind::Nominal(_)) => {
            !is_builtin_scalar_nominal_value_type(types, ty)
        }
        _ => false,
    }
}

pub(crate) fn mir_transport_requirements(
    types: &TypeStore,
    ty: TypeId,
) -> MirTransportRequirements {
    let trace = mir_type_requires_trace(types, ty);
    MirTransportRequirements {
        trace,
        copy: true,
        drop: trace || mir_is_aggregate_transport_ty(types, ty),
    }
}

pub(in crate::mir) fn erasure_boxing_reason(
    builtins: BuiltinTypes,
    types: &TypeStore,
    source_ty: TypeId,
    target_ty: TypeId,
) -> Option<MirBoxingReason> {
    if source_ty == target_ty || !matches!(types.kind(source_ty), TypeKind::Value(_)) {
        return None;
    }
    if target_ty == builtins.any {
        return Some(MirBoxingReason::AnyErasure);
    }
    match types.kind(target_ty) {
        TypeKind::Ref(_) | TypeKind::Param(_) | TypeKind::StarProjection(_) => {
            Some(MirBoxingReason::RefErasure)
        }
        TypeKind::Value(_) => None,
    }
}

pub(in crate::mir::lower) fn value_erasure_transport(
    builtins: BuiltinTypes,
    types: &TypeStore,
    facts: &MirLoweringFacts,
    source_ty: TypeId,
    target_ty: TypeId,
) -> Option<ValueTransportMetadata> {
    let reason = erasure_boxing_reason(builtins, types, source_ty, target_ty)?;
    Some(ValueTransportMetadata {
        source_ty,
        kind: mir_transport_kind_for_ty(types, facts, source_ty),
        requirements: mir_transport_requirements(types, source_ty),
        boxing: Some(MirBoxingIntent {
            source_ty,
            target_ty: Some(target_ty),
            reason,
        }),
    })
}

pub(in crate::mir::lower) fn lower_initializer_root_kind(
    kind: TopLevelInitRootKind,
) -> InitializerRootKind {
    match kind {
        TopLevelInitRootKind::RuntimeImmutableVal => InitializerRootKind::RuntimeImmutableVal,
        TopLevelInitRootKind::RuntimeMutableVar { storage } => {
            InitializerRootKind::RuntimeMutableVar { storage }
        }
        TopLevelInitRootKind::ObjectSingleton => InitializerRootKind::ObjectSingleton,
    }
}

pub(in crate::mir::lower) fn lower_initializer_dependency(
    dependency: &TopLevelInitDependency,
) -> InitializerDependency {
    InitializerDependency {
        fqn: dependency.fqn().to_string(),
        kind: match dependency.kind() {
            TopLevelInitDependencyKind::TopLevelValue => InitializerDependencyKind::TopLevelValue,
            TopLevelInitDependencyKind::ObjectSingleton => {
                InitializerDependencyKind::ObjectSingleton
            }
        },
    }
}

pub(in crate::mir::lower) fn lower_extern_global_root(
    contract: &ExternGlobalContract,
) -> ExternGlobalRoot {
    ExternGlobalRoot {
        span: contract.span(),
        fqn: contract.fqn().to_string(),
        source_path: contract.source_path().to_path_buf(),
        ty: contract.ty(),
        mutable: contract.mutable(),
        symbol: contract.symbol().to_string(),
        linkage: contract.linkage(),
        storage: contract.storage(),
        initializer_absent: contract.initializer_absent(),
        unsafe_required: contract.unsafe_required(),
    }
}

pub(in crate::mir::lower) fn lower_decl_metadata(decl: &hir::Decl) -> MetadataRoot {
    match decl {
        hir::Decl::TypeAlias(alias) => MetadataRoot::TypeAlias(TypeAliasMetadata {
            span: alias.span,
            fqn: alias.fqn.clone(),
            name: alias.name.clone(),
            type_params: alias
                .type_params
                .iter()
                .map(lower_decl_type_param_metadata)
                .collect(),
            ty: alias.ty,
        }),
        hir::Decl::Nominal(nominal) => MetadataRoot::Nominal(NominalMetadata {
            span: nominal.span,
            fqn: nominal.fqn.clone(),
            name: nominal.name.clone(),
            kind: nominal.kind,
            is_interior_mutable: nominal.is_interior_mutable,
            type_params: nominal
                .type_params
                .iter()
                .map(lower_decl_type_param_metadata)
                .collect(),
            has_eff_param: nominal.has_eff_param,
            supertypes: nominal
                .supertypes
                .iter()
                .map(lower_supertype_metadata)
                .collect(),
            interfaces: nominal.interfaces.clone(),
            constructors: nominal
                .constructors
                .iter()
                .map(lower_ctor_metadata)
                .collect(),
            members: nominal
                .members
                .iter()
                .map(lower_decl_member_metadata)
                .collect(),
        }),
        hir::Decl::Object(object) => MetadataRoot::Object(ObjectMetadata {
            span: object.span,
            fqn: object.fqn.clone(),
            name: object.name.clone(),
            kind: object.kind,
            supertypes: object
                .supertypes
                .iter()
                .map(lower_supertype_metadata)
                .collect(),
            interfaces: object.interfaces.clone(),
            initializer_root: object.initializer_root.clone(),
            members: object
                .members
                .iter()
                .map(lower_decl_member_metadata)
                .collect(),
        }),
        hir::Decl::ExtensionProperty(prop) => {
            MetadataRoot::ExtensionProperty(ExtensionPropertyMetadata {
                span: prop.span,
                fqn: prop.fqn.clone(),
                name: prop.name.clone(),
                type_params: prop
                    .type_params
                    .iter()
                    .map(lower_decl_type_param_metadata)
                    .collect(),
                mutable: prop.mutable,
                receiver_ty: prop.receiver_ty,
                ty: prop.ty,
                getter: prop.getter.as_ref().map(lower_accessor_metadata),
                setter: prop.setter.as_ref().map(lower_accessor_metadata),
            })
        }
    }
}

pub(in crate::mir::lower) fn lower_decl_type_param_metadata(
    param: &hir::DeclTypeParam,
) -> DeclTypeParamMetadata {
    DeclTypeParamMetadata {
        span: param.span,
        name: param.name.clone(),
        variance: param.variance,
        ty: param.ty,
    }
}

pub(in crate::mir::lower) fn lower_supertype_metadata(
    supertype: &hir::SupertypeDecl,
) -> SupertypeMetadata {
    SupertypeMetadata {
        span: supertype.span,
        fqn: supertype.fqn.clone(),
        ty: supertype.ty,
        ctor_arg_count: supertype.ctor_arg_count,
    }
}

pub(in crate::mir::lower) fn lower_ctor_metadata(ctor: &hir::CtorDecl) -> CtorMetadata {
    CtorMetadata {
        span: ctor.span,
        kind: ctor.kind,
        params: ctor.params.iter().map(lower_ctor_param_metadata).collect(),
        delegation: ctor.delegation,
    }
}

pub(in crate::mir::lower) fn lower_ctor_param_metadata(
    param: &hir::CtorParamDecl,
) -> CtorParamMetadata {
    CtorParamMetadata {
        span: param.span,
        name: param.name.clone(),
        ty: param.ty,
        has_default: param.has_default,
        property: param.property,
    }
}

pub(in crate::mir::lower) fn lower_decl_member_metadata(
    member: &hir::DeclMember,
) -> DeclMemberMetadata {
    match member {
        hir::DeclMember::Field(field) => DeclMemberMetadata::Field(lower_field_metadata(field)),
        hir::DeclMember::Property(prop) => DeclMemberMetadata::Property(PropertyMetadata {
            span: prop.span,
            fqn: prop.fqn.clone(),
            name: prop.name.clone(),
            mutable: prop.mutable,
            ty: prop.ty,
            has_backing_field: prop.has_backing_field,
            getter: prop.getter.as_ref().map(lower_accessor_metadata),
            setter: prop.setter.as_ref().map(lower_accessor_metadata),
        }),
        hir::DeclMember::Fun(fun) => DeclMemberMetadata::Fun(MemberFunMetadata {
            span: fun.span,
            fqn: fun.fqn.clone(),
            name: fun.name.clone(),
            type_params: fun
                .type_params
                .iter()
                .map(lower_decl_type_param_metadata)
                .collect(),
            params: fun.params.iter().map(lower_ctor_param_metadata).collect(),
            return_ty: fun.return_ty,
        }),
        hir::DeclMember::EnumVariant(variant) => {
            DeclMemberMetadata::EnumVariant(EnumVariantMetadata {
                span: variant.span,
                fqn: variant.fqn.clone(),
                name: variant.name.clone(),
                fields: variant.fields.iter().map(lower_field_metadata).collect(),
            })
        }
        hir::DeclMember::InitBlock { span } => DeclMemberMetadata::InitBlock { span: *span },
        hir::DeclMember::Nested(decl) => {
            DeclMemberMetadata::Nested(Box::new(lower_decl_metadata(decl)))
        }
    }
}

pub(in crate::mir::lower) fn lower_field_metadata(field: &hir::FieldDecl) -> FieldMetadata {
    FieldMetadata {
        span: field.span,
        fqn: field.fqn.clone(),
        name: field.name.clone(),
        mutable: field.mutable,
        ty: field.ty,
        origin: field.origin,
    }
}

pub(in crate::mir::lower) fn lower_accessor_metadata(
    accessor: &hir::AccessorContract,
) -> AccessorMetadata {
    AccessorMetadata {
        span: accessor.span,
        fqn: accessor.fqn.clone(),
    }
}

/// 函数体 lowering：负责为单个函数构造 `Body`、管理 locals、并生成显式 CFG。
#[derive(Debug)]
pub(in crate::mir::lower) struct FnLowering<'a> {
    pub(in crate::mir::lower) builtins: BuiltinTypes,
    pub(in crate::mir::lower) types: &'a mut TypeStore,
    pub(in crate::mir::lower) facts: &'a MirLoweringFacts,
    pub(in crate::mir::lower) top_level_fun_return_tys: HashMap<String, TypeId>,
    pub(in crate::mir::lower) top_level_fun_param_tys: HashMap<String, Vec<TypeId>>,
    pub(in crate::mir::lower) owner_fqn: String,
    pub(in crate::mir::lower) source_path: std::path::PathBuf,
    pub(in crate::mir::lower) current_return_ty: TypeId,
    pub(in crate::mir::lower) body: Body,
    pub(in crate::mir::lower) current_bb: BasicBlockId,
    pub(in crate::mir::lower) next_temp: u32,
    pub(in crate::mir::lower) next_site_id: u32,
    pub(in crate::mir::lower) symbol_locals: HashMap<hir::SymbolId, LocalId>,
    /// 值 local 的最小 provenance。
    ///
    /// 当前阶段主要为 call / member / unresolved callee / pattern canonicalization 保留最小来源信息；
    /// 一旦出现多路径/多来源冲突，就保守退化为 `UnknownCallable`。
    pub(in crate::mir::lower) value_origins: HashMap<LocalId, ValueOrigin>,
    pub(in crate::mir::lower) cleanup_scopes: Vec<CleanupScope>,
    pub(in crate::mir::lower) loop_stack: Vec<LoopContext>,
    pub(in crate::mir::lower) nested_funs: Vec<FunDecl>,
}

/// 当前函数内的一个 loop 语境（用于 `break/continue` lowering）。
#[derive(Debug, Clone, Copy)]
pub(in crate::mir::lower) struct LoopContext {
    pub(in crate::mir::lower) break_target: BasicBlockId,
    pub(in crate::mir::lower) continue_target: BasicBlockId,
    pub(in crate::mir::lower) cleanup_depth: usize,
}

/// 当前 lowering 语境里“离开该作用域前必须执行”的 finally/cleanup 块。
#[derive(Debug, Clone)]
pub(in crate::mir::lower) struct CleanupScope {
    pub(in crate::mir::lower) finally: hir::Block,
}

/// 一个 local 当前可观察到的最小 provenance。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::mir::lower) enum ValueOrigin {
    Closure { fn_ptr: String },
    TopLevelRef { fqn: String },
    MemberAccess { member: MemberAccessMetadata },
    UnresolvedName { name: String },
    UnknownCallable,
}

pub(in crate::mir::lower) fn gc_intrinsic_callee_from_origin(
    origin: Option<&ValueOrigin>,
) -> Option<&str> {
    let Some(ValueOrigin::MemberAccess {
        member:
            MemberAccessMetadata {
                resolved: Some(MemberTarget::Fun { fqn } | MemberTarget::ExtensionFun { fqn }),
                ..
            },
    }) = origin
    else {
        return None;
    };
    matches!(
        fqn.as_str(),
        "scoop.core.GC.pin"
            | "scoop.core.GC.unpin"
            | "scoop.core.GC.handleNew"
            | "scoop.core.GC.handleGet"
            | "scoop.core.GC.handleDrop"
    )
    .then_some(fqn.as_str())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::mir::lower) enum StoredContinuationRouteError {
    MissingSourceLocal,
    Ambiguous,
}

/// 一个 closure 捕获的外部局部变量在 env tuple 中的布局信息（T0711）。
#[derive(Debug, Clone)]
pub(in crate::mir::lower) struct ClosureCaptureLayout {
    pub(in crate::mir::lower) id: hir::SymbolId,
    pub(in crate::mir::lower) name: String,
    pub(in crate::mir::lower) decl_span: Span,
    pub(in crate::mir::lower) ty: TypeId,
    pub(in crate::mir::lower) mutable: bool,
    /// 在“创建该 closure 的函数”中，对应被捕获值的 local。
    pub(in crate::mir::lower) source_local: LocalId,
}

#[derive(Debug, Clone)]
pub(in crate::mir::lower) struct WhenPatternBinding {
    pub(in crate::mir::lower) id: hir::SymbolId,
    pub(in crate::mir::lower) span: Span,
    pub(in crate::mir::lower) name: String,
    pub(in crate::mir::lower) ty: TypeId,
    pub(in crate::mir::lower) path: Vec<PatternBindingStep>,
}
