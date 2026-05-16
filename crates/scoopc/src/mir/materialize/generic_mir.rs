//! Generic MIR orchestration: drives the per-instance materialization loop and exposes the helper types (substitution context, reachable-fn tracking, member-value-type cache) shared across the rewriting layers.

use super::*;

pub(super) fn materialize_generic_mir(
    generic_file: File,
    types: TypeStore,
    builtins: BuiltinTypes,
    requests: MaterializeRequestSet<'_>,
    opt_level: OptLevel,
) -> MaterializeResult<MaterializedMir> {
    let MaterializeRequestSet {
        monomorph_requests,
        hir_direct_instance_keys_by_fun,
        construction_inputs,
    } = requests;
    let typecheck_types = construction_inputs.typecheck_types;
    let mut materializer = MirInstanceMaterializer::new(
        generic_file,
        types,
        builtins,
        construction_inputs,
        opt_level,
        opt_level.enables_summary_driven_mir_inlining(),
        opt_level.enables_mir_escape_analysis(),
    )?;
    materializer.hir_direct_instance_keys_by_fun = hir_direct_instance_keys_by_fun;
    materializer.load_monomorph_request_site_bindings(typecheck_types, monomorph_requests)?;
    let initial_requests = materializer.seed_requests(typecheck_types, monomorph_requests)?;
    materializer.run(initial_requests)
}

#[derive(Clone)]
pub(super) struct TemplateRootInfo {
    pub(super) template: TemplateKey,
    pub(super) type_param_names: Vec<String>,
    pub(super) eff_param_name: Option<String>,
    pub(super) family: Vec<FunDecl>,
}

#[derive(Clone)]
pub(super) struct TemplateRootCandidate {
    pub(super) request_lookup_key: RequestTemplateKey,
    pub(super) template: TemplateKey,
    pub(super) type_param_names: Vec<String>,
    pub(super) eff_param_name: Option<String>,
    pub(super) signature_key: String,
    pub(super) root_fun: FunDecl,
}

#[derive(Clone)]
pub(super) struct DeclOnlyTemplateCandidate {
    pub(super) request_lookup_key: RequestTemplateKey,
    pub(super) template: TemplateKey,
    pub(super) type_param_names: Vec<String>,
    pub(super) eff_param_name: Option<String>,
    pub(super) signature_key: String,
    pub(super) signature: CallableSignatureInfo,
}

#[derive(Clone)]
pub(super) struct TemplateCatalogCandidate {
    pub(super) template: TemplateKey,
    pub(super) stable_template_key: StableTemplateKey,
    pub(super) signature_key: String,
    pub(super) prefers_materialized_body: bool,
}

#[derive(Clone)]
pub(super) struct TemplateSignatureInfo {
    pub(super) template: TemplateKey,
    pub(super) type_param_names: Vec<String>,
    pub(super) eff_param_name: Option<String>,
    pub(super) fun_ty: TypeId,
    pub(super) return_ty: TypeId,
    pub(super) params: Vec<CallableSignatureParam>,
}

#[derive(Clone, PartialEq, Eq)]
pub(super) struct SiteInstanceBinding {
    pub(super) template: TemplateKey,
    pub(super) type_args: Vec<TypeId>,
    pub(super) eff_args: Vec<EffectRow>,
}

#[derive(Clone)]
pub(super) struct MemberValueTypeInfo {
    pub(super) owner_fqn: String,
    pub(super) owner_type_param_names: Vec<String>,
    pub(super) ty: TypeId,
}

#[derive(Default)]
pub(super) struct InstanceSubstitution {
    pub(super) type_params: HashMap<String, TypeId>,
    pub(super) effect_params: HashMap<String, EffectRow>,
}

pub(super) struct RewriteContext<'a> {
    pub(super) locals: &'a [LocalDecl],
    pub(super) substitution: &'a InstanceSubstitution,
    pub(super) template_source_path: &'a Path,
    pub(super) template_root_fqn: &'a str,
    pub(super) instance_root_fqn: &'a str,
}

#[derive(Clone)]
pub(super) struct ReachableMirFun {
    pub(super) source_path: PathBuf,
    pub(super) fun: FunDecl,
}

#[derive(Clone)]
pub(super) struct PassPublishedOrdinaryCallable {
    pub(super) source_path: PathBuf,
    pub(super) fun: FunDecl,
}

pub(super) fn dispatch_direct_call_args(
    call_span: Span,
    receiver: &Operand,
    args: &[CallArg],
) -> Vec<CallArg> {
    let mut direct_args = Vec::with_capacity(args.len() + 1);
    direct_args.push(CallArg {
        span: call_span,
        name: None,
        value: receiver.clone(),
    });
    direct_args.extend(args.iter().cloned());
    direct_args
}

pub(super) fn collect_member_value_type_infos(file: &File) -> HashMap<String, MemberValueTypeInfo> {
    let mut out = HashMap::new();
    for item in &file.items {
        let Item::Metadata(root) = item else {
            continue;
        };
        collect_member_value_type_infos_from_metadata(root, &mut out);
    }
    out
}

pub(super) fn collect_member_value_type_infos_from_hir_decls(
    decls: &[crate::hir::Decl],
) -> HashMap<String, MemberValueTypeInfo> {
    let mut out = HashMap::new();
    for decl in decls {
        collect_member_value_type_infos_from_hir_decl(decl, &mut out);
    }
    out
}

pub(super) fn collect_member_value_type_infos_from_hir_decl(
    decl: &crate::hir::Decl,
    out: &mut HashMap<String, MemberValueTypeInfo>,
) {
    match decl {
        crate::hir::Decl::Nominal(nominal) => collect_member_value_type_infos_from_hir_members(
            &nominal.fqn,
            &nominal.type_params,
            &nominal.members,
            out,
        ),
        crate::hir::Decl::Object(object) => {
            collect_member_value_type_infos_from_hir_members(&object.fqn, &[], &object.members, out)
        }
        crate::hir::Decl::ExtensionProperty(prop) => {
            out.insert(
                prop.fqn.clone(),
                MemberValueTypeInfo {
                    owner_fqn: prop.fqn.clone(),
                    owner_type_param_names: prop
                        .type_params
                        .iter()
                        .map(|param| param.name.clone())
                        .collect(),
                    ty: prop.ty,
                },
            );
        }
        crate::hir::Decl::TypeAlias(_) => {}
    }
}

pub(super) fn collect_member_value_type_infos_from_hir_members(
    owner_fqn: &str,
    owner_type_params: &[crate::hir::DeclTypeParam],
    members: &[crate::hir::DeclMember],
    out: &mut HashMap<String, MemberValueTypeInfo>,
) {
    let owner_type_param_names = owner_type_params
        .iter()
        .map(|param| param.name.clone())
        .collect::<Vec<_>>();
    for member in members {
        match member {
            crate::hir::DeclMember::Field(field) => {
                out.insert(
                    field.fqn.clone(),
                    MemberValueTypeInfo {
                        owner_fqn: owner_fqn.to_string(),
                        owner_type_param_names: owner_type_param_names.clone(),
                        ty: field.ty,
                    },
                );
            }
            crate::hir::DeclMember::Property(prop) => {
                out.insert(
                    prop.fqn.clone(),
                    MemberValueTypeInfo {
                        owner_fqn: owner_fqn.to_string(),
                        owner_type_param_names: owner_type_param_names.clone(),
                        ty: prop.ty,
                    },
                );
            }
            crate::hir::DeclMember::Nested(decl) => {
                collect_member_value_type_infos_from_hir_decl(decl, out);
            }
            crate::hir::DeclMember::EnumVariant(_)
            | crate::hir::DeclMember::Fun(_)
            | crate::hir::DeclMember::InitBlock { .. } => {}
        }
    }
}

pub(super) fn collect_member_value_type_infos_from_metadata(
    root: &MetadataRoot,
    out: &mut HashMap<String, MemberValueTypeInfo>,
) {
    match root {
        MetadataRoot::Nominal(nominal) => collect_member_value_type_infos_from_members(
            &nominal.fqn,
            &nominal.type_params,
            &nominal.members,
            out,
        ),
        MetadataRoot::Object(object) => {
            collect_member_value_type_infos_from_members(&object.fqn, &[], &object.members, out)
        }
        MetadataRoot::ExtensionProperty(prop) => {
            out.insert(
                prop.fqn.clone(),
                MemberValueTypeInfo {
                    owner_fqn: prop.fqn.clone(),
                    owner_type_param_names: prop
                        .type_params
                        .iter()
                        .map(|param| param.name.clone())
                        .collect(),
                    ty: prop.ty,
                },
            );
        }
        MetadataRoot::TypeAlias(_) => {}
    }
}

pub(super) fn collect_member_value_type_infos_from_members(
    owner_fqn: &str,
    owner_type_params: &[DeclTypeParamMetadata],
    members: &[DeclMemberMetadata],
    out: &mut HashMap<String, MemberValueTypeInfo>,
) {
    let owner_type_param_names = owner_type_params
        .iter()
        .map(|param| param.name.clone())
        .collect::<Vec<_>>();
    for member in members {
        match member {
            DeclMemberMetadata::Field(field) => {
                out.insert(
                    field.fqn.clone(),
                    MemberValueTypeInfo {
                        owner_fqn: owner_fqn.to_string(),
                        owner_type_param_names: owner_type_param_names.clone(),
                        ty: field.ty,
                    },
                );
            }
            DeclMemberMetadata::Property(prop) => {
                out.insert(
                    prop.fqn.clone(),
                    MemberValueTypeInfo {
                        owner_fqn: owner_fqn.to_string(),
                        owner_type_param_names: owner_type_param_names.clone(),
                        ty: prop.ty,
                    },
                );
            }
            DeclMemberMetadata::Nested(root) => {
                collect_member_value_type_infos_from_metadata(root, out);
            }
            DeclMemberMetadata::EnumVariant(_)
            | DeclMemberMetadata::Fun(_)
            | DeclMemberMetadata::InitBlock { .. } => {}
        }
    }
}

pub(super) fn reachable_body_block_indices(body: &Body) -> Vec<usize> {
    match body.reachable_blocks() {
        Ok(blocks) => blocks
            .into_iter()
            .map(|block| block.as_u32() as usize)
            .collect(),
        Err(_) => (0..body.blocks.len()).collect(),
    }
}

pub(super) struct MirInstanceMaterializer {
    pub(super) stable_cone_key: StableConeKey,
    pub(super) types: TypeStore,
    pub(super) builtins: BuiltinTypes,
    pub(super) opt_level: OptLevel,
    pub(super) known_receiver_subclasses: crate::devirtualize::KnownReceiverSubclassIndex,
    pub(super) direct_subclasses: HashMap<String, BTreeSet<String>>,
    pub(super) class_vtables: crate::vtable::ClassVtableIndex,
    pub(super) interfaces: crate::itable::InterfaceIndex,
    pub(super) class_itables: crate::itable::ClassItableIndex,
    pub(super) request_root_funs: Vec<ReachableMirFun>,
    pub(super) hir_direct_instance_keys_by_fun: HashMap<(PathBuf, Span), Vec<InstanceKey>>,
    pub(super) generic_family_fqns: HashSet<String>,
    pub(super) request_templates: HashMap<RequestTemplateKey, TemplateKey>,
    pub(super) roots: HashMap<TemplateKey, TemplateRootInfo>,
    pub(super) template_signatures: HashMap<TemplateKey, TemplateSignatureInfo>,
    pub(super) stable_template_keys: HashMap<TemplateKey, StableTemplateKey>,
    pub(super) nongeneric_callable_signature_keys: HashMap<TemplateKey, String>,
    pub(super) template_symbol_suffixes: HashMap<TemplateKey, String>,
    pub(super) roots_by_fqn: HashMap<String, Vec<TemplateKey>>,
    pub(super) explicit_dispatch_candidate_instances: HashMap<String, Vec<InstanceKey>>,
    pub(super) direct_call_bindings: HashMap<SourceSiteKey, ast::TopLevelFunCallBinding>,
    pub(super) top_level_vars: crate::hir::TopLevelVarIndex,
    pub(super) top_level_consts: crate::hir::TopLevelConstIndex,
    pub(super) top_level_immutable_values: crate::hir::TopLevelImmutableValueIndex,
    pub(super) object_inits: crate::hir::ObjectInitIndex,
    pub(super) member_value_tys: HashMap<String, MemberValueTypeInfo>,
    pub(super) request_sources: HashSet<PathBuf>,
    pub(super) filter_initial_requests_to_reachable_call_sites: bool,
    pub(super) reachable_fun_bodies_by_request: HashMap<RequestTemplateKey, ReachableMirFun>,
    pub(super) reachable_fun_bodies_by_fqn: HashMap<String, Vec<ReachableMirFun>>,
    pub(super) all_fun_bodies_by_fqn: HashMap<String, Vec<FunDecl>>,
    pub(super) call_bindings: HashMap<SourceSiteKey, SiteInstanceBinding>,
    pub(super) value_ref_bindings: HashMap<SourceSiteKey, SiteInstanceBinding>,
    pub(super) reachable_request_call_sites: HashSet<SourceSiteKey>,
    pub(super) reachable_request_stmt_spans: Vec<(PathBuf, Span)>,
    pub(super) scanned_top_level_vars: HashSet<String>,
    pub(super) scanned_top_level_consts: HashSet<String>,
    pub(super) scanned_top_level_immutable_values: HashSet<String>,
    pub(super) scanned_object_inits: HashSet<String>,
    pub(super) scanned_non_generic_funs: HashSet<(PathBuf, Span)>,
    pub(super) caller_side_pass_candidates: Vec<FunDecl>,
    pub(super) pass_published_ordinary_callables: Vec<PassPublishedOrdinaryCallable>,
    pub(super) materialized_direct_call_result_tys: HashMap<String, TypeId>,
    pub(super) enable_summary_driven_inlining: bool,
    pub(super) enable_mir_escape_analysis: bool,
    pub(super) queued: HashSet<InstanceKey>,
    pub(super) queue: VecDeque<InstanceKey>,
    pub(super) materialized: HashMap<InstanceKey, Vec<FunDecl>>,
    pub(super) declaration_only_instances: HashSet<InstanceKey>,
}

pub(super) struct ReachableRvalueScanContext<'a> {
    pub(super) span: Span,
    pub(super) result_ty: Option<TypeId>,
    pub(super) template_source_path: &'a Path,
    pub(super) locals: &'a [LocalDecl],
    pub(super) substitution: &'a InstanceSubstitution,
}

pub(super) struct DirectCallInferenceInput<'a> {
    pub(super) template_source_path: &'a Path,
    pub(super) call_span: Span,
    pub(super) callee_fqn: &'a str,
    pub(super) args: &'a [CallArg],
    pub(super) result_ty: Option<TypeId>,
    pub(super) locals: &'a [LocalDecl],
    pub(super) substitution: &'a InstanceSubstitution,
}

#[derive(Clone, Copy)]
pub(super) struct DirectCallRewriteContext<'a> {
    pub(super) template_source_path: &'a Path,
    pub(super) caller_fqn: &'a str,
    pub(super) block_id: BasicBlockId,
    pub(super) call_span: Span,
    pub(super) result_ty: Option<TypeId>,
    pub(super) locals: &'a [LocalDecl],
    pub(super) substitution: &'a InstanceSubstitution,
}

pub(super) fn nominal_type_fqn(types: &TypeStore, ty: TypeId) -> Option<&str> {
    match types.kind(ty) {
        TypeKind::Ref(RefTypeKind::Nominal(nominal))
        | TypeKind::Value(ValueTypeKind::Nominal(nominal)) => Some(nominal.fqn.as_str()),
        TypeKind::Value(ValueTypeKind::Bool) => Some("scoop.core.Bool"),
        TypeKind::Value(ValueTypeKind::Char) => Some("scoop.core.Char"),
        TypeKind::Value(ValueTypeKind::Int) => Some("scoop.core.Int"),
        TypeKind::Value(ValueTypeKind::UInt) => Some("scoop.core.UInt"),
        TypeKind::Value(ValueTypeKind::Float32) => Some("scoop.core.Float32"),
        TypeKind::Value(ValueTypeKind::Float64) => Some("scoop.core.Float64"),
        TypeKind::Ref(RefTypeKind::String) => Some("scoop.core.String"),
        _ => None,
    }
}

pub(super) fn is_builtin_scalar_or_string_type(types: &TypeStore, ty: TypeId) -> bool {
    matches!(
        types.kind(ty),
        TypeKind::Value(
            ValueTypeKind::Bool
                | ValueTypeKind::Char
                | ValueTypeKind::Int
                | ValueTypeKind::UInt
                | ValueTypeKind::Float32
                | ValueTypeKind::Float64
        ) | TypeKind::Ref(RefTypeKind::String)
    )
}

pub(super) fn collect_interface_slot_targets(
    entries: &[crate::itable::ClassItableEntry],
    owner_fqn: &str,
    slot_index: usize,
    targets: &mut BTreeSet<String>,
) {
    for entry in entries {
        if entry.interface_fqn != owner_fqn {
            continue;
        }
        if let Some(fqn) = entry.method_impl_fqns.get(slot_index)
            && !fqn.is_empty()
        {
            targets.insert(fqn.clone());
        }
    }
}
