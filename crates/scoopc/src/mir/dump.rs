//! Stable MIR and materialized MIR dump renderers.

use std::collections::HashMap;
use std::path::Path;

use crate::dump_support::{
    IndentWriter, LocalEntityKey, format_debug, format_effect_row, format_type, normalize_dump_path,
};
use crate::stable_id::{StableCanonicalKey, StableSymbolKey};
use crate::ty::TypeStore;

use super::{
    AccessorMetadata, AggregateTransportField, AggregateTransportMetadata,
    ArrayElementTransportMetadata, BasicBlock, BasicBlockId, Body, CallAbiHandoffMetadata, CallArg,
    CallKind, CallTransportMetadata, ConstValue, CtorMetadata, CtorParamMetadata,
    DeclMemberMetadata, DeclTypeParamMetadata, DispatchMetadata, EnumVariantMetadata,
    ExtensionPropertyMetadata, ExternGlobalRoot, FieldMetadata, File, FunDecl,
    GcIntrinsicTransportMetadata, HandlerArm, InitializerDependency, InitializerRoot, Item,
    LocalDecl, LocalId, MaterializedMir, MemberAccessMetadata, MemberFunMetadata, MemberTarget,
    MetadataRoot, MirBoxingIntent, MirTransportRequirements, NominalMetadata, ObjectMetadata,
    Operand, Param, Pattern, PatternBindingStep, PerformArg, PerformMetadata, PropertyMetadata,
    ResumeMetadata, RuntimeCastFailure, RuntimeCastMetadata, RuntimeCastResult,
    RuntimePatternTypeTestMetadata, RuntimeTypeDescriptorKey, RuntimeTypeDescriptorKind,
    RuntimeTypeParameterizedMatch, RuntimeTypeTestMetadata, Rvalue, SiteId, Statement,
    StatementKind, StoredContinuationRoutePublication, StoredContinuationValueRoute,
    StructLitField, SupertypeMetadata, Terminator, TerminatorKind, TopLevelRef, TypeAliasMetadata,
    TypeMetadataLiteral, UnwindAction, ValueTransportMetadata,
};

pub(crate) fn stable_dump_file(file: &File, types: &TypeStore) -> String {
    let mut renderer = MirDumpRenderer::new(types);
    renderer.render_file(file);
    renderer.finish()
}

pub(crate) fn stable_dump_materialized(materialized: &MaterializedMir) -> String {
    let mut renderer = MirDumpRenderer::new(&materialized.types);
    renderer.line("MaterializedMir {");
    renderer.out.push_indent();
    renderer.line("instances: [");
    renderer.out.push_indent();
    let mut instances = materialized.instance_keys.iter().collect::<Vec<_>>();
    instances
        .sort_by_key(|instance| renderer.materialized_instance_display(materialized, instance));
    for instance in instances {
        let stable_key = materialized
            .authoritative_stable_instance_key(instance)
            .expect("materialized instance must have a stable key");
        let display = renderer.materialized_instance_display(materialized, instance);
        let exported = materialized
            .instance_exported_fun_symbol(instance)
            .unwrap_or_default();
        let label = crate::stable_id::stable_dump_label("instance", &stable_key.canonical_text());
        renderer.line(&format!(
            "MaterializedInstance {{ label: {}, display_fqn: {}, exported_symbol: {} }},",
            label,
            format_debug(&display),
            format_debug(&exported),
        ));
    }
    renderer.out.pop_indent();
    renderer.line("],");
    renderer.line("file:");
    renderer.out.push_indent();
    renderer.render_file(&materialized.file);
    renderer.out.pop_indent();
    renderer.out.pop_indent();
    renderer.line("}");
    renderer.finish()
}

struct MirDumpRenderer<'a> {
    types: &'a TypeStore,
    out: IndentWriter,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct BodyLabels {
    locals: Vec<String>,
    blocks: Vec<String>,
    sites: HashMap<SiteId, String>,
}

impl BodyLabels {
    pub(crate) fn local_label(&self, local: LocalId) -> String {
        self.locals[local.as_u32() as usize].clone()
    }

    pub(crate) fn block_label(&self, block: BasicBlockId) -> String {
        self.blocks[block.as_u32() as usize].clone()
    }

    pub(crate) fn site_label(&self, site: SiteId) -> String {
        self.sites
            .get(&site)
            .cloned()
            .unwrap_or_else(|| format!("site_missing#{}", site.as_u32()))
    }
}

pub(crate) fn build_body_labels_for_dump(
    owner: &str,
    body: &Body,
    types: &TypeStore,
) -> BodyLabels {
    MirDumpRenderer::new(types).build_body_labels(owner, body)
}

struct BodyRenderCtx<'a> {
    labels: BodyLabels,
    _owner: &'a str,
}

impl<'a> MirDumpRenderer<'a> {
    fn new(types: &'a TypeStore) -> Self {
        Self {
            types,
            out: IndentWriter::new(),
        }
    }

    fn finish(self) -> String {
        self.out.finish()
    }

    fn render_file(&mut self, file: &File) {
        self.open_struct("File");
        self.open_list_field("items");
        for item in &file.items {
            self.render_item(item);
        }
        self.close_list_field();
        self.close_struct("");
    }

    fn render_item(&mut self, item: &Item) {
        match item {
            Item::Fun(fun) => self.render_variant("Fun", |this| {
                this.render_fun_decl(fun);
            }),
            Item::InitializerRoot(root) => self.render_variant("InitializerRoot", |this| {
                this.render_initializer_root(root);
            }),
            Item::ExternGlobal(root) => self.render_variant("ExternGlobal", |this| {
                this.render_extern_global_root(root);
            }),
            Item::Metadata(root) => self.render_variant("Metadata", |this| {
                this.render_metadata_root(root);
            }),
            Item::Todo { span, kind } => {
                self.open_struct("Todo");
                self.field_debug("span", span);
                self.field_debug("kind", kind);
                self.close_struct(",");
            }
        }
    }

    fn render_initializer_root(&mut self, root: &InitializerRoot) {
        self.open_struct("InitializerRoot");
        self.field_debug("span", &root.span);
        self.field_debug("fqn", &root.fqn);
        self.field_debug("source_path", &normalize_dump_path(&root.source_path));
        self.field_debug("kind", &root.kind);
        self.field_option_text("ty", root.ty.map(|ty| self.type_text(ty)));
        self.field_option_text(
            "initializer_transport",
            root.initializer_transport
                .as_ref()
                .map(|transport| self.value_transport_text(None, transport)),
        );
        self.field_bool("has_initializer", root.has_initializer);
        self.field_raw(
            "dependencies",
            &self.list_text(
                root.dependencies
                    .iter()
                    .map(|dep| self.initializer_dependency_text(dep))
                    .collect(),
            ),
        );
        self.field_raw(
            "hidden_effects",
            &format_effect_row(self.types, &root.hidden_effects),
        );
        self.close_struct(",");
    }

    fn render_extern_global_root(&mut self, root: &ExternGlobalRoot) {
        self.open_struct("ExternGlobalRoot");
        self.field_debug("span", &root.span);
        self.field_debug("fqn", &root.fqn);
        self.field_debug("source_path", &normalize_dump_path(&root.source_path));
        self.field_raw("ty", &self.type_text(root.ty));
        self.field_bool("mutable", root.mutable);
        self.field_debug("symbol", &root.symbol);
        self.field_debug("linkage", &root.linkage);
        self.field_debug("storage", &root.storage);
        self.field_bool("initializer_absent", root.initializer_absent);
        self.field_bool("unsafe_required", root.unsafe_required);
        self.close_struct(",");
    }

    fn render_metadata_root(&mut self, root: &MetadataRoot) {
        match root {
            MetadataRoot::TypeAlias(alias) => self.render_variant("TypeAlias", |this| {
                this.render_type_alias_metadata(alias);
            }),
            MetadataRoot::Nominal(nominal) => self.render_variant("Nominal", |this| {
                this.render_nominal_metadata(nominal);
            }),
            MetadataRoot::Object(object) => self.render_variant("Object", |this| {
                this.render_object_metadata(object);
            }),
            MetadataRoot::ExtensionProperty(prop) => {
                self.render_variant("ExtensionProperty", |this| {
                    this.render_extension_property_metadata(prop);
                })
            }
        }
    }

    fn render_type_alias_metadata(&mut self, alias: &TypeAliasMetadata) {
        self.open_struct("TypeAliasMetadata");
        self.field_debug("span", &alias.span);
        self.field_debug("fqn", &alias.fqn);
        self.field_debug("name", &alias.name);
        self.field_raw(
            "type_params",
            &self.list_text(
                alias
                    .type_params
                    .iter()
                    .map(|param| self.decl_type_param_text(param))
                    .collect(),
            ),
        );
        self.field_raw("ty", &self.type_text(alias.ty));
        self.close_struct(",");
    }

    fn render_nominal_metadata(&mut self, nominal: &NominalMetadata) {
        self.open_struct("NominalMetadata");
        self.field_debug("span", &nominal.span);
        self.field_debug("fqn", &nominal.fqn);
        self.field_debug("name", &nominal.name);
        self.field_debug("kind", &nominal.kind);
        self.field_raw(
            "type_params",
            &self.list_text(
                nominal
                    .type_params
                    .iter()
                    .map(|param| self.decl_type_param_text(param))
                    .collect(),
            ),
        );
        self.field_raw(
            "supertypes",
            &self.list_text(
                nominal
                    .supertypes
                    .iter()
                    .map(|supertype| self.supertype_metadata_text(supertype))
                    .collect(),
            ),
        );
        self.field_raw(
            "interfaces",
            &self.list_text(nominal.interfaces.iter().map(format_debug).collect()),
        );
        self.field_raw(
            "constructors",
            &self.list_text(
                nominal
                    .constructors
                    .iter()
                    .map(|ctor| self.ctor_metadata_text(ctor))
                    .collect(),
            ),
        );
        self.field_raw(
            "members",
            &self.list_text(
                nominal
                    .members
                    .iter()
                    .map(|member| self.decl_member_metadata_text(member))
                    .collect(),
            ),
        );
        self.close_struct(",");
    }

    fn render_object_metadata(&mut self, object: &ObjectMetadata) {
        self.open_struct("ObjectMetadata");
        self.field_debug("span", &object.span);
        self.field_debug("fqn", &object.fqn);
        self.field_debug("name", &object.name);
        self.field_debug("kind", &object.kind);
        self.field_raw(
            "supertypes",
            &self.list_text(
                object
                    .supertypes
                    .iter()
                    .map(|supertype| self.supertype_metadata_text(supertype))
                    .collect(),
            ),
        );
        self.field_raw(
            "interfaces",
            &self.list_text(object.interfaces.iter().map(format_debug).collect()),
        );
        self.field_debug("initializer_root", &object.initializer_root);
        self.field_raw(
            "members",
            &self.list_text(
                object
                    .members
                    .iter()
                    .map(|member| self.decl_member_metadata_text(member))
                    .collect(),
            ),
        );
        self.close_struct(",");
    }

    fn render_extension_property_metadata(&mut self, prop: &ExtensionPropertyMetadata) {
        self.open_struct("ExtensionPropertyMetadata");
        self.field_debug("span", &prop.span);
        self.field_debug("fqn", &prop.fqn);
        self.field_debug("name", &prop.name);
        self.field_bool("mutable", prop.mutable);
        self.field_raw(
            "type_params",
            &self.list_text(
                prop.type_params
                    .iter()
                    .map(|param| self.decl_type_param_text(param))
                    .collect(),
            ),
        );
        self.field_raw("receiver_ty", &self.type_text(prop.receiver_ty));
        self.field_raw("ty", &self.type_text(prop.ty));
        self.field_option_text(
            "getter",
            prop.getter
                .as_ref()
                .map(|accessor| self.accessor_text(accessor)),
        );
        self.field_option_text(
            "setter",
            prop.setter
                .as_ref()
                .map(|accessor| self.accessor_text(accessor)),
        );
        self.close_struct(",");
    }

    fn render_fun_decl(&mut self, fun: &FunDecl) {
        self.open_struct("FunDecl");
        self.field_debug("span", &fun.span);
        self.field_debug("fqn", &fun.fqn);
        self.field_debug("name", &fun.name);
        self.field_raw("ty", &self.type_text(fun.ty));
        if let Some(body) = fun.body.as_ref() {
            let ctx = BodyRenderCtx {
                labels: self.build_body_labels(&fun.fqn, body),
                _owner: &fun.fqn,
            };
            self.field_raw(
                "params",
                &self.list_text(
                    fun.params
                        .iter()
                        .map(|param| self.param_text(&ctx, param))
                        .collect(),
                ),
            );
            self.field_raw("return_ty", &self.type_text(fun.return_ty));
            self.line("body: Some(");
            self.out.push_indent();
            self.render_body(&ctx, body);
            self.out.pop_indent();
            self.line("),");
        } else {
            self.field_raw(
                "params",
                &self.list_text(
                    fun.params
                        .iter()
                        .map(|param| self.param_without_body_text(param))
                        .collect(),
                ),
            );
            self.field_raw("return_ty", &self.type_text(fun.return_ty));
            self.field_raw("body", "None");
        }
        self.close_struct(",");
    }

    fn render_body(&mut self, ctx: &BodyRenderCtx<'_>, body: &Body) {
        self.open_struct("Body");
        self.field_raw(
            "locals",
            &self.list_text(
                body.locals
                    .iter()
                    .enumerate()
                    .map(|(index, local)| self.local_decl_text(&ctx.labels, index, local))
                    .collect(),
            ),
        );
        self.open_list_field("blocks");
        for (index, block) in body.blocks.iter().enumerate() {
            self.render_basic_block(ctx, body, BasicBlockId::from_raw(index as u32), block);
        }
        self.close_list_field();
        self.field_raw("start", &ctx.labels.blocks[body.start.as_u32() as usize]);
        self.close_struct("");
    }

    fn render_basic_block(
        &mut self,
        ctx: &BodyRenderCtx<'_>,
        _body: &Body,
        block_id: BasicBlockId,
        block: &BasicBlock,
    ) {
        self.open_struct("BasicBlock");
        self.field_raw("label", &ctx.labels.blocks[block_id.as_u32() as usize]);
        self.field_bool("is_cleanup", block.is_cleanup);
        self.field_raw(
            "stmts",
            &self.list_text(
                block
                    .stmts
                    .iter()
                    .map(|stmt| self.statement_text(ctx, stmt))
                    .collect(),
            ),
        );
        self.field_raw("terminator", &self.terminator_text(ctx, &block.terminator));
        self.close_struct(",");
    }

    fn build_body_labels(&self, owner: &str, body: &Body) -> BodyLabels {
        let mut local_counts = HashMap::new();
        let locals = body
            .locals
            .iter()
            .map(|local| {
                let name = local.name.as_deref().unwrap_or("_");
                let signature = format!(
                    "local|{owner}|{}|{}|{}|{}|{}",
                    local.span.start,
                    local.span.end,
                    name,
                    self.type_text(local.ty),
                    format_debug(&local.source),
                );
                let ordinal = next_ordinal(&mut local_counts, &signature);
                LocalEntityKey::new(
                    owner,
                    Path::new(""),
                    local.span,
                    format!("local:{:?}", local.source),
                    name,
                    ordinal,
                )
                .label("local")
            })
            .collect::<Vec<_>>();

        let mut block_counts = HashMap::new();
        let blocks = body
            .blocks
            .iter()
            .enumerate()
            .map(|(index, block)| {
                let signature = format!(
                    "block|{owner}|{}|{}|{}|{}",
                    block.terminator.span.start,
                    block.terminator.span.end,
                    block.is_cleanup,
                    self.block_signature(block),
                );
                let ordinal = next_ordinal(&mut block_counts, &signature);
                let readable = if index == body.start.as_u32() as usize {
                    "entry"
                } else if block.is_cleanup {
                    "cleanup"
                } else {
                    "block"
                };
                LocalEntityKey::new(
                    owner,
                    Path::new(""),
                    block.terminator.span,
                    "block",
                    readable,
                    ordinal,
                )
                .label("bb")
            })
            .collect::<Vec<_>>();

        let mut site_counts = HashMap::new();
        let mut sites = HashMap::new();
        for block in &body.blocks {
            for stmt in &block.stmts {
                if let StatementKind::Assign { value, .. } = &stmt.kind
                    && let Some((site_id, kind)) = self.rvalue_site(value)
                {
                    let signature = format!(
                        "site|{owner}|{}|{}|{}",
                        stmt.span.start, stmt.span.end, kind
                    );
                    let ordinal = next_ordinal(&mut site_counts, &signature);
                    let label = LocalEntityKey::new(
                        owner,
                        Path::new(""),
                        stmt.span,
                        format!("site:{kind}"),
                        kind,
                        ordinal,
                    )
                    .label("site");
                    sites.insert(site_id, label);
                }
            }
            if let Some((site_id, kind)) = self.terminator_site(&block.terminator.kind) {
                let signature = format!(
                    "site|{owner}|{}|{}|{}",
                    block.terminator.span.start, block.terminator.span.end, kind,
                );
                let ordinal = next_ordinal(&mut site_counts, &signature);
                let label = LocalEntityKey::new(
                    owner,
                    Path::new(""),
                    block.terminator.span,
                    format!("site:{kind}"),
                    kind,
                    ordinal,
                )
                .label("site");
                sites.insert(site_id, label);
            }
        }

        BodyLabels {
            locals,
            blocks,
            sites,
        }
    }

    fn rvalue_site(&self, value: &Rvalue) -> Option<(SiteId, &'static str)> {
        match value {
            Rvalue::TopLevelRef(TopLevelRef {
                site_id: Some(site_id),
                ..
            }) => Some((*site_id, "top_level_ref")),
            Rvalue::MemberAccess {
                site_id: Some(site_id),
                ..
            } => Some((*site_id, "member_access")),
            Rvalue::ClassCtor { site_id, .. } => Some((*site_id, "class_ctor")),
            Rvalue::Call {
                site_id,
                kind: CallKind::Direct { .. },
                ..
            } => Some((*site_id, "call_direct")),
            Rvalue::Call {
                site_id,
                kind: CallKind::Closure { .. },
                ..
            } => Some((*site_id, "call_closure")),
            Rvalue::Call {
                site_id,
                kind: CallKind::FunValue { .. },
                ..
            } => Some((*site_id, "call_fun_value")),
            Rvalue::Call {
                site_id,
                kind: CallKind::FunPtr { .. },
                ..
            } => Some((*site_id, "call_fun_ptr")),
            Rvalue::Call {
                site_id,
                kind: CallKind::Virtual { .. },
                ..
            } => Some((*site_id, "call_virtual")),
            Rvalue::Call {
                site_id,
                kind: CallKind::Interface { .. },
                ..
            } => Some((*site_id, "call_interface")),
            Rvalue::Call {
                site_id,
                kind: CallKind::Resume { .. },
                ..
            } => Some((*site_id, "call_resume")),
            _ => None,
        }
    }

    fn terminator_site(&self, kind: &TerminatorKind) -> Option<(SiteId, &'static str)> {
        match kind {
            TerminatorKind::Perform { site_id, .. } => Some((*site_id, "perform")),
            TerminatorKind::Handle { site_id, .. } => Some((*site_id, "handle")),
            _ => None,
        }
    }

    fn block_signature(&self, block: &BasicBlock) -> String {
        let stmt_bits = block
            .stmts
            .iter()
            .map(|stmt| {
                format!(
                    "{}..{}:{}",
                    stmt.span.start,
                    stmt.span.end,
                    self.statement_tag(&stmt.kind)
                )
            })
            .collect::<Vec<_>>()
            .join("|");
        format!(
            "stmts=[{stmt_bits}]::term={}::unwind={}",
            self.terminator_tag(&block.terminator.kind),
            self.unwind_tag(&block.terminator.unwind),
        )
    }

    fn statement_tag(&self, kind: &StatementKind) -> &'static str {
        match kind {
            StatementKind::Nop => "nop",
            StatementKind::Assign { value, .. } => self.rvalue_tag(value),
            StatementKind::StoreMember { .. } => "store_member",
            StatementKind::StoreTopLevelVar { .. } => "store_top_level_var",
            StatementKind::Todo(_) => "todo",
        }
    }

    fn rvalue_tag(&self, value: &Rvalue) -> &'static str {
        match value {
            Rvalue::Use(_) => "use",
            Rvalue::Transport { .. } => "transport",
            Rvalue::TopLevelRef(_) => "top_level_ref",
            Rvalue::UnresolvedName { .. } => "unresolved_name",
            Rvalue::TypeCheck { .. } => "type_check",
            Rvalue::Cast { .. } => "cast",
            Rvalue::MemberAccess { .. } => "member_access",
            Rvalue::EnumVariant { .. } => "enum_variant",
            Rvalue::ClassCtor { .. } => "class_ctor",
            Rvalue::Call { .. } => "call",
            Rvalue::MakeTuple { .. } => "make_tuple",
            Rvalue::StructLit { .. } => "struct_lit",
            Rvalue::SizeOf { .. } => "size_of",
            Rvalue::KindOf { .. } => "kind_of",
            Rvalue::AlignOf { .. } => "align_of",
            Rvalue::DescOf { .. } => "desc_of",
            Rvalue::TypeMetadataLiteral(_) => "type_metadata_literal",
            Rvalue::InterpolatedString { .. } => "interpolated_string",
            Rvalue::TupleGet { .. } => "tuple_get",
            Rvalue::PatternMatch { .. } => "pattern_match",
            Rvalue::PatternExtract { .. } => "pattern_extract",
            Rvalue::MakeClosure { .. } => "make_closure",
            Rvalue::PerformResult { .. } => "perform_result",
            Rvalue::Todo(_) => "todo",
        }
    }

    fn terminator_tag(&self, kind: &TerminatorKind) -> &'static str {
        match kind {
            TerminatorKind::Return { .. } => "return",
            TerminatorKind::ResumeUnwind => "resume_unwind",
            TerminatorKind::Goto { .. } => "goto",
            TerminatorKind::CondBr { .. } => "cond_br",
            TerminatorKind::Unreachable => "unreachable",
            TerminatorKind::Perform { .. } => "perform",
            TerminatorKind::Handle { .. } => "handle",
            TerminatorKind::Todo(_) => "todo",
        }
    }

    fn unwind_tag(&self, unwind: &UnwindAction) -> &'static str {
        match unwind {
            UnwindAction::NoUnwind => "no_unwind",
            UnwindAction::Propagate => "propagate",
            UnwindAction::Cleanup { .. } => "cleanup",
            UnwindAction::Todo(_) => "todo",
        }
    }

    fn param_text(&self, ctx: &BodyRenderCtx<'_>, param: &Param) -> String {
        self.inline_struct(
            "Param",
            vec![
                ("span", format_debug(&param.span)),
                ("name", format_debug(&param.name)),
                ("ty", self.type_text(param.ty)),
                ("local", self.local_label(&ctx.labels, param.local)),
            ],
        )
    }

    fn param_without_body_text(&self, param: &Param) -> String {
        self.inline_struct(
            "Param",
            vec![
                ("span", format_debug(&param.span)),
                ("name", format_debug(&param.name)),
                ("ty", self.type_text(param.ty)),
            ],
        )
    }

    fn local_decl_text(&self, labels: &BodyLabels, index: usize, local: &LocalDecl) -> String {
        self.inline_struct(
            "LocalDecl",
            vec![
                ("label", labels.locals[index].clone()),
                ("span", format_debug(&local.span)),
                (
                    "name",
                    self.option_text(local.name.as_ref().map(format_debug)),
                ),
                ("ty", self.type_text(local.ty)),
                ("source", format_debug(&local.source)),
            ],
        )
    }

    fn statement_text(&self, ctx: &BodyRenderCtx<'_>, stmt: &Statement) -> String {
        self.inline_struct(
            "Statement",
            vec![
                ("span", format_debug(&stmt.span)),
                ("kind", self.statement_kind_text(ctx, &stmt.kind)),
            ],
        )
    }

    fn statement_kind_text(&self, ctx: &BodyRenderCtx<'_>, kind: &StatementKind) -> String {
        match kind {
            StatementKind::Nop => "Nop".to_string(),
            StatementKind::Assign { target, value } => self.inline_struct(
                "Assign",
                vec![
                    ("target", self.local_label(&ctx.labels, *target)),
                    ("value", self.rvalue_text(ctx, value)),
                ],
            ),
            StatementKind::StoreMember {
                receiver,
                member,
                value,
                value_ty,
                continuation_route,
            } => self.inline_struct(
                "StoreMember",
                vec![
                    ("receiver", self.operand_text(ctx, receiver)),
                    ("member", self.member_access_metadata_text(member)),
                    ("value", self.operand_text(ctx, value)),
                    ("value_ty", self.type_text(*value_ty)),
                    (
                        "continuation_route",
                        self.stored_continuation_route_text(ctx, continuation_route),
                    ),
                ],
            ),
            StatementKind::StoreTopLevelVar {
                fqn,
                value,
                value_ty,
            } => self.inline_struct(
                "StoreTopLevelVar",
                vec![
                    ("fqn", format_debug(fqn)),
                    ("value", self.operand_text(ctx, value)),
                    ("value_ty", self.type_text(*value_ty)),
                ],
            ),
            StatementKind::Todo(reason) => format!("Todo({reason:?})"),
        }
    }

    fn terminator_text(&self, ctx: &BodyRenderCtx<'_>, terminator: &Terminator) -> String {
        self.inline_struct(
            "Terminator",
            vec![
                ("span", format_debug(&terminator.span)),
                ("kind", self.terminator_kind_text(ctx, &terminator.kind)),
                ("unwind", self.unwind_text(ctx, &terminator.unwind)),
            ],
        )
    }

    fn terminator_kind_text(&self, ctx: &BodyRenderCtx<'_>, kind: &TerminatorKind) -> String {
        match kind {
            TerminatorKind::Return { value } => self.inline_struct(
                "Return",
                vec![(
                    "value",
                    self.option_text(value.as_ref().map(|value| self.operand_text(ctx, value))),
                )],
            ),
            TerminatorKind::ResumeUnwind => "ResumeUnwind".to_string(),
            TerminatorKind::Goto { target } => self.inline_struct(
                "Goto",
                vec![("target", self.block_label(&ctx.labels, *target))],
            ),
            TerminatorKind::CondBr {
                cond,
                then_target,
                else_target,
            } => self.inline_struct(
                "CondBr",
                vec![
                    ("cond", self.operand_text(ctx, cond)),
                    ("then_target", self.block_label(&ctx.labels, *then_target)),
                    ("else_target", self.block_label(&ctx.labels, *else_target)),
                ],
            ),
            TerminatorKind::Unreachable => "Unreachable".to_string(),
            TerminatorKind::Perform {
                site_id,
                op_fqn,
                metadata,
                args,
                resume_target,
            } => self.inline_struct(
                "Perform",
                vec![
                    ("site_id", self.site_label(&ctx.labels, *site_id)),
                    ("op_fqn", format_debug(op_fqn)),
                    ("metadata", self.perform_metadata_text(ctx, metadata)),
                    (
                        "args",
                        self.list_text(
                            args.iter()
                                .map(|arg| self.perform_arg_text(ctx, arg))
                                .collect(),
                        ),
                    ),
                    (
                        "resume_target",
                        self.block_label(&ctx.labels, *resume_target),
                    ),
                ],
            ),
            TerminatorKind::Handle {
                site_id,
                metadata,
                arms,
                has_finally,
                body_target,
                arm_targets,
                finally_target,
                exit_target,
            } => self.inline_struct(
                "Handle",
                vec![
                    ("site_id", self.site_label(&ctx.labels, *site_id)),
                    ("metadata", self.handle_metadata_text(metadata)),
                    (
                        "arms",
                        self.list_text(
                            arms.iter()
                                .map(|arm| self.handler_arm_text(ctx, arm))
                                .collect(),
                        ),
                    ),
                    ("has_finally", has_finally.to_string()),
                    ("body_target", self.block_label(&ctx.labels, *body_target)),
                    (
                        "arm_targets",
                        self.list_text(
                            arm_targets
                                .iter()
                                .map(|target| self.block_label(&ctx.labels, *target))
                                .collect(),
                        ),
                    ),
                    (
                        "finally_target",
                        self.option_text(
                            finally_target.map(|target| self.block_label(&ctx.labels, target)),
                        ),
                    ),
                    ("exit_target", self.block_label(&ctx.labels, *exit_target)),
                ],
            ),
            TerminatorKind::Todo(reason) => format!("Todo({reason:?})"),
        }
    }

    fn unwind_text(&self, ctx: &BodyRenderCtx<'_>, unwind: &UnwindAction) -> String {
        match unwind {
            UnwindAction::NoUnwind => "NoUnwind".to_string(),
            UnwindAction::Propagate => "Propagate".to_string(),
            UnwindAction::Cleanup { target } => self.inline_struct(
                "Cleanup",
                vec![("target", self.block_label(&ctx.labels, *target))],
            ),
            UnwindAction::Todo(reason) => format!("Todo({reason:?})"),
        }
    }

    fn rvalue_text(&self, ctx: &BodyRenderCtx<'_>, value: &Rvalue) -> String {
        match value {
            Rvalue::Use(operand) => self.inline_enum("Use", self.operand_text(ctx, operand)),
            Rvalue::Transport { value, transport } => self.inline_struct(
                "Transport",
                vec![
                    ("value", self.operand_text(ctx, value)),
                    ("transport", self.value_transport_text(Some(ctx), transport)),
                ],
            ),
            Rvalue::TopLevelRef(top) => self.top_level_ref_text(ctx, top),
            Rvalue::UnresolvedName { name } => {
                self.inline_struct("UnresolvedName", vec![("name", format_debug(name))])
            }
            Rvalue::TypeCheck {
                value,
                op,
                test_ty,
                metadata,
            } => self.inline_struct(
                "TypeCheck",
                vec![
                    ("value", self.operand_text(ctx, value)),
                    ("op", format_debug(op)),
                    ("test_ty", self.type_text(*test_ty)),
                    ("metadata", self.runtime_type_test_text(metadata)),
                ],
            ),
            Rvalue::Cast {
                value,
                op,
                target_ty,
                metadata,
            } => self.inline_struct(
                "Cast",
                vec![
                    ("value", self.operand_text(ctx, value)),
                    ("op", format_debug(op)),
                    ("target_ty", self.type_text(*target_ty)),
                    ("metadata", self.runtime_cast_text(metadata)),
                ],
            ),
            Rvalue::MemberAccess {
                site_id,
                receiver,
                member,
            } => self.inline_struct(
                "MemberAccess",
                vec![
                    (
                        "site_id",
                        self.option_text(
                            site_id.map(|site_id| self.site_label(&ctx.labels, site_id)),
                        ),
                    ),
                    ("receiver", self.operand_text(ctx, receiver)),
                    ("member", self.member_access_metadata_text(member)),
                ],
            ),
            Rvalue::EnumVariant {
                enum_ty,
                variant_name,
                args,
                payload,
            } => self.inline_struct(
                "EnumVariant",
                vec![
                    ("enum_ty", self.type_text(*enum_ty)),
                    ("variant_name", format_debug(variant_name)),
                    (
                        "args",
                        self.list_text(
                            args.iter()
                                .map(|arg| self.call_arg_text(ctx, arg))
                                .collect(),
                        ),
                    ),
                    ("payload", self.aggregate_transport_text(Some(ctx), payload)),
                ],
            ),
            Rvalue::ClassCtor {
                site_id,
                class_fqn,
                ctor,
                args,
                hidden_effects,
            } => self.inline_struct(
                "ClassCtor",
                vec![
                    ("site_id", self.site_label(&ctx.labels, *site_id)),
                    ("class_fqn", format_debug(class_fqn)),
                    ("ctor", format_debug(ctor)),
                    (
                        "args",
                        self.list_text(
                            args.iter()
                                .map(|arg| self.call_arg_text(ctx, arg))
                                .collect(),
                        ),
                    ),
                    (
                        "hidden_effects",
                        format_effect_row(self.types, hidden_effects),
                    ),
                ],
            ),
            Rvalue::Call {
                site_id,
                kind,
                args,
                transport,
            } => self.inline_struct(
                "Call",
                vec![
                    ("site_id", self.site_label(&ctx.labels, *site_id)),
                    ("kind", self.call_kind_text(ctx, kind)),
                    (
                        "args",
                        self.list_text(
                            args.iter()
                                .map(|arg| self.call_arg_text(ctx, arg))
                                .collect(),
                        ),
                    ),
                    ("transport", self.call_transport_text(ctx, transport)),
                ],
            ),
            Rvalue::MakeTuple {
                elements,
                transport,
            } => self.inline_struct(
                "MakeTuple",
                vec![
                    (
                        "elements",
                        self.list_text(
                            elements
                                .iter()
                                .map(|operand| self.operand_text(ctx, operand))
                                .collect(),
                        ),
                    ),
                    (
                        "transport",
                        self.aggregate_transport_text(Some(ctx), transport),
                    ),
                ],
            ),
            Rvalue::StructLit { fields, transport } => self.inline_struct(
                "StructLit",
                vec![
                    (
                        "fields",
                        self.list_text(
                            fields
                                .iter()
                                .map(|field| self.struct_lit_field_text(ctx, field))
                                .collect(),
                        ),
                    ),
                    (
                        "transport",
                        self.aggregate_transport_text(Some(ctx), transport),
                    ),
                ],
            ),
            Rvalue::SizeOf { value_ty } => {
                self.inline_struct("SizeOf", vec![("value_ty", self.type_text(*value_ty))])
            }
            Rvalue::KindOf { value_ty } => {
                self.inline_struct("KindOf", vec![("value_ty", self.type_text(*value_ty))])
            }
            Rvalue::AlignOf { value_ty } => {
                self.inline_struct("AlignOf", vec![("value_ty", self.type_text(*value_ty))])
            }
            Rvalue::DescOf { value_ty } => {
                self.inline_struct("DescOf", vec![("value_ty", self.type_text(*value_ty))])
            }
            Rvalue::TypeMetadataLiteral(literal) => self.type_metadata_literal_text(literal),
            Rvalue::InterpolatedString { raw, parts } => self.inline_struct(
                "InterpolatedString",
                vec![
                    ("raw", raw.to_string()),
                    (
                        "parts",
                        self.list_text(
                            parts
                                .iter()
                                .map(|part| self.interpolated_string_part_text(ctx, part))
                                .collect(),
                        ),
                    ),
                ],
            ),
            Rvalue::TupleGet { tuple, index } => self.inline_struct(
                "TupleGet",
                vec![
                    ("tuple", self.operand_text(ctx, tuple)),
                    ("index", index.to_string()),
                ],
            ),
            Rvalue::PatternMatch { subject, pattern } => self.inline_struct(
                "PatternMatch",
                vec![
                    ("subject", self.operand_text(ctx, subject)),
                    ("pattern", self.pattern_text(pattern)),
                ],
            ),
            Rvalue::PatternExtract { subject, path } => self.inline_struct(
                "PatternExtract",
                vec![
                    ("subject", self.operand_text(ctx, subject)),
                    (
                        "path",
                        self.list_text(
                            path.iter()
                                .map(|step| self.pattern_step_text(step))
                                .collect(),
                        ),
                    ),
                ],
            ),
            Rvalue::MakeClosure {
                env,
                fn_ptr,
                env_contract,
            } => self.inline_struct(
                "MakeClosure",
                vec![
                    ("env", self.operand_text(ctx, env)),
                    ("fn_ptr", format_debug(fn_ptr)),
                    (
                        "env_contract",
                        self.closure_env_transport_text(Some(ctx), env_contract),
                    ),
                ],
            ),
            Rvalue::PerformResult { op_fqn, effect_ty } => self.inline_struct(
                "PerformResult",
                vec![
                    ("op_fqn", format_debug(op_fqn)),
                    ("effect_ty", self.type_text(*effect_ty)),
                ],
            ),
            Rvalue::Todo(reason) => format!("Todo({reason:?})"),
        }
    }

    fn operand_text(&self, ctx: &BodyRenderCtx<'_>, operand: &Operand) -> String {
        match operand {
            Operand::Local(local) => {
                self.inline_enum("Local", self.local_label(&ctx.labels, *local))
            }
            Operand::Const(value) => self.inline_enum("Const", self.const_text(value)),
        }
    }

    fn const_text(&self, value: &ConstValue) -> String {
        match value {
            ConstValue::Bool(value) => format!("Bool({value})"),
            ConstValue::Char => "Char".to_string(),
            ConstValue::Unit => "Unit".to_string(),
            ConstValue::Int => "Int".to_string(),
            ConstValue::SynthInt(value) => format!("SynthInt({value})"),
            ConstValue::Float64 => "Float64".to_string(),
            ConstValue::Float32 => "Float32".to_string(),
            ConstValue::String => "String".to_string(),
            ConstValue::SynthString(value) => format!("SynthString({})", format_debug(value)),
        }
    }

    fn call_arg_text(&self, ctx: &BodyRenderCtx<'_>, arg: &CallArg) -> String {
        self.inline_struct(
            "CallArg",
            vec![
                ("span", format_debug(&arg.span)),
                (
                    "name",
                    self.option_text(arg.name.as_ref().map(format_debug)),
                ),
                ("value", self.operand_text(ctx, &arg.value)),
            ],
        )
    }

    fn perform_arg_text(&self, ctx: &BodyRenderCtx<'_>, arg: &PerformArg) -> String {
        self.inline_struct(
            "PerformArg",
            vec![
                ("span", format_debug(&arg.span)),
                ("source_arg_index", arg.source_arg_index.to_string()),
                (
                    "name",
                    self.option_text(arg.name.as_ref().map(format_debug)),
                ),
                ("value", self.operand_text(ctx, &arg.value)),
            ],
        )
    }

    fn top_level_ref_text(&self, ctx: &BodyRenderCtx<'_>, top: &TopLevelRef) -> String {
        self.inline_struct(
            "TopLevelRef",
            vec![
                ("fqn", format_debug(&top.fqn)),
                (
                    "site_id",
                    self.option_text(
                        top.site_id
                            .map(|site_id| self.site_label(&ctx.labels, site_id)),
                    ),
                ),
                (
                    "hidden_effects",
                    format_effect_row(self.types, &top.hidden_effects),
                ),
            ],
        )
    }

    fn call_kind_text(&self, ctx: &BodyRenderCtx<'_>, kind: &CallKind) -> String {
        match kind {
            CallKind::Direct { callee_fqn } => {
                self.inline_struct("Direct", vec![("callee_fqn", format_debug(callee_fqn))])
            }
            CallKind::Closure { callee, fn_ptr } => self.inline_struct(
                "Closure",
                vec![
                    ("callee", self.operand_text(ctx, callee)),
                    ("fn_ptr", format_debug(fn_ptr)),
                ],
            ),
            CallKind::FunValue { callee } => {
                self.inline_struct("FunValue", vec![("callee", self.operand_text(ctx, callee))])
            }
            CallKind::FunPtr { callee } => {
                self.inline_struct("FunPtr", vec![("callee", self.operand_text(ctx, callee))])
            }
            CallKind::Virtual { receiver, dispatch } => self.inline_struct(
                "Virtual",
                vec![
                    ("receiver", self.operand_text(ctx, receiver)),
                    ("dispatch", self.dispatch_metadata_text(dispatch)),
                ],
            ),
            CallKind::Interface { receiver, dispatch } => self.inline_struct(
                "Interface",
                vec![
                    ("receiver", self.operand_text(ctx, receiver)),
                    ("dispatch", self.dispatch_metadata_text(dispatch)),
                ],
            ),
            CallKind::Resume {
                continuation,
                resume,
            } => self.inline_struct(
                "Resume",
                vec![
                    ("continuation", self.operand_text(ctx, continuation)),
                    ("resume", self.resume_metadata_text(resume)),
                ],
            ),
        }
    }

    fn dispatch_metadata_text(&self, metadata: &DispatchMetadata) -> String {
        self.inline_struct(
            "DispatchMetadata",
            vec![
                ("owner_fqn", format_debug(&metadata.owner_fqn)),
                ("member_name", format_debug(&metadata.member_name)),
                ("member_fqn", format_debug(&metadata.member_fqn)),
                (
                    "member_decl_span",
                    self.option_text(metadata.member_decl_span.map(|span| format_debug(&span))),
                ),
                ("receiver_ty", self.type_text(metadata.receiver_ty)),
            ],
        )
    }

    fn resume_metadata_text(&self, metadata: &ResumeMetadata) -> String {
        self.inline_struct(
            "ResumeMetadata",
            vec![
                ("continuation_ty", self.type_text(metadata.continuation_ty)),
                ("resume_ty", self.type_text(metadata.resume_ty)),
                ("answer_ty", self.type_text(metadata.answer_ty)),
                ("return_ty", self.type_text(metadata.return_ty)),
                (
                    "out_effects",
                    format_effect_row(self.types, &metadata.out_effects),
                ),
                (
                    "runtime_error_effect_ty",
                    self.option_text(
                        metadata
                            .runtime_error_effect_ty
                            .map(|ty| self.type_text(ty)),
                    ),
                ),
                ("suspends_outward", metadata.suspends_outward.to_string()),
            ],
        )
    }

    fn handle_metadata_text(&self, metadata: &super::HandleMetadata) -> String {
        self.inline_struct(
            "HandleMetadata",
            vec![
                ("result_ty", self.type_text(metadata.result_ty)),
                ("body_result_ty", self.type_text(metadata.body_result_ty)),
                (
                    "finally_result_ty",
                    self.option_text(metadata.finally_result_ty.map(|ty| self.type_text(ty))),
                ),
            ],
        )
    }

    fn handler_arm_text(&self, ctx: &BodyRenderCtx<'_>, arm: &HandlerArm) -> String {
        self.inline_struct(
            "HandlerArm",
            vec![
                ("op_fqn", format_debug(&arm.op_fqn)),
                (
                    "op_type_args",
                    self.list_text(
                        arm.op_type_args
                            .iter()
                            .map(|&ty| self.type_text(ty))
                            .collect(),
                    ),
                ),
                ("binder_count", arm.binder_count.to_string()),
                (
                    "binder_locals",
                    self.list_text(
                        arm.binder_locals
                            .iter()
                            .map(|local| self.local_label(&ctx.labels, *local))
                            .collect(),
                    ),
                ),
                (
                    "continuation_local",
                    self.option_text(
                        arm.continuation_local
                            .map(|local| self.local_label(&ctx.labels, local)),
                    ),
                ),
                ("handled_effect_ty", self.type_text(arm.handled_effect_ty)),
                (
                    "payload_tuple_ty",
                    self.option_text(arm.payload_tuple_ty.map(|ty| self.type_text(ty))),
                ),
                (
                    "payload_component_tys",
                    self.list_text(
                        arm.payload_component_tys
                            .iter()
                            .map(|&ty| self.type_text(ty))
                            .collect(),
                    ),
                ),
                ("body_ty", self.type_text(arm.body_ty)),
                ("kind", format_debug(&arm.kind)),
            ],
        )
    }

    fn perform_metadata_text(&self, ctx: &BodyRenderCtx<'_>, metadata: &PerformMetadata) -> String {
        self.inline_struct(
            "PerformMetadata",
            vec![
                ("effect_ty", self.type_text(metadata.effect_ty)),
                (
                    "op_type_args",
                    self.list_text(
                        metadata
                            .op_type_args
                            .iter()
                            .map(|&ty| self.type_text(ty))
                            .collect(),
                    ),
                ),
                ("result_ty", self.type_text(metadata.result_ty)),
                (
                    "payload_tuple_ty",
                    self.option_text(metadata.payload_tuple_ty.map(|ty| self.type_text(ty))),
                ),
                (
                    "payload_component_tys",
                    self.list_text(
                        metadata
                            .payload_component_tys
                            .iter()
                            .map(|&ty| self.type_text(ty))
                            .collect(),
                    ),
                ),
                (
                    "payload_transport",
                    self.list_text(
                        metadata
                            .payload_transport
                            .iter()
                            .map(|transport| self.value_transport_text(Some(ctx), transport))
                            .collect(),
                    ),
                ),
                (
                    "arg_mapping",
                    self.list_text(
                        metadata
                            .arg_mapping
                            .iter()
                            .map(|idx| idx.to_string())
                            .collect(),
                    ),
                ),
            ],
        )
    }

    fn member_access_metadata_text(&self, metadata: &MemberAccessMetadata) -> String {
        self.inline_struct(
            "MemberAccessMetadata",
            vec![
                ("name", format_debug(&metadata.name)),
                ("receiver_ty", self.type_text(metadata.receiver_ty)),
                (
                    "resolved",
                    self.option_text(
                        metadata
                            .resolved
                            .as_ref()
                            .map(|resolved| self.member_target_text(resolved)),
                    ),
                ),
                (
                    "hidden_effects",
                    format_effect_row(self.types, &metadata.hidden_effects),
                ),
            ],
        )
    }

    fn member_target_text(&self, target: &MemberTarget) -> String {
        match target {
            MemberTarget::Value { fqn } => {
                self.inline_struct("Value", vec![("fqn", format_debug(fqn))])
            }
            MemberTarget::Fun { fqn } => {
                self.inline_struct("Fun", vec![("fqn", format_debug(fqn))])
            }
            MemberTarget::ExtensionValue { fqn } => {
                self.inline_struct("ExtensionValue", vec![("fqn", format_debug(fqn))])
            }
            MemberTarget::ExtensionFun { fqn } => {
                self.inline_struct("ExtensionFun", vec![("fqn", format_debug(fqn))])
            }
        }
    }

    fn stored_continuation_route_text(
        &self,
        ctx: &BodyRenderCtx<'_>,
        route: &StoredContinuationRoutePublication,
    ) -> String {
        match route {
            StoredContinuationRoutePublication::None => "None".to_string(),
            StoredContinuationRoutePublication::Ambiguous => "Ambiguous".to_string(),
            StoredContinuationRoutePublication::Unique(route) => self.inline_struct(
                "Unique",
                vec![(
                    "route",
                    self.stored_continuation_value_route_text(ctx, route),
                )],
            ),
        }
    }

    fn stored_continuation_value_route_text(
        &self,
        ctx: &BodyRenderCtx<'_>,
        route: &StoredContinuationValueRoute,
    ) -> String {
        self.inline_struct(
            "StoredContinuationValueRoute",
            vec![
                (
                    "source_local",
                    self.local_label(&ctx.labels, route.source_local),
                ),
                ("source_ty", self.type_text(route.source_ty)),
                (
                    "path",
                    self.list_text(
                        route
                            .path
                            .iter()
                            .map(|step| self.pattern_step_text(step))
                            .collect(),
                    ),
                ),
            ],
        )
    }

    fn pattern_step_text(&self, step: &PatternBindingStep) -> String {
        match step {
            PatternBindingStep::TupleIndex(index) => {
                self.inline_struct("TupleIndex", vec![("index", index.to_string())])
            }
            PatternBindingStep::VariantField {
                variant,
                field_index,
            } => self.inline_struct(
                "VariantField",
                vec![
                    ("variant", format_debug(variant)),
                    ("field_index", field_index.to_string()),
                ],
            ),
        }
    }

    fn pattern_text(&self, pattern: &Pattern) -> String {
        match pattern {
            Pattern::Else => "Else".to_string(),
            Pattern::Or { pats } => self.inline_struct(
                "Or",
                vec![(
                    "pats",
                    self.list_text(pats.iter().map(|pat| self.pattern_text(pat)).collect()),
                )],
            ),
            Pattern::Wildcard => "Wildcard".to_string(),
            Pattern::Rest => "Rest".to_string(),
            Pattern::Is { ty, metadata } => self.inline_struct(
                "Is",
                vec![
                    ("ty", self.type_text(*ty)),
                    ("metadata", self.runtime_pattern_type_test_text(metadata)),
                ],
            ),
            Pattern::Bind { name, ty } => self.inline_struct(
                "Bind",
                vec![("name", format_debug(name)), ("ty", self.type_text(*ty))],
            ),
            Pattern::Tuple { elements } => self.inline_struct(
                "Tuple",
                vec![(
                    "elements",
                    self.list_text(elements.iter().map(|pat| self.pattern_text(pat)).collect()),
                )],
            ),
            Pattern::Variant { name, args } => self.inline_struct(
                "Variant",
                vec![
                    ("name", format_debug(name)),
                    (
                        "args",
                        self.list_text(args.iter().map(|pat| self.pattern_text(pat)).collect()),
                    ),
                ],
            ),
            Pattern::IntLit { raw } => {
                self.inline_struct("IntLit", vec![("raw", format_debug(raw))])
            }
            Pattern::CharLit { value } => {
                self.inline_struct("CharLit", vec![("value", format_debug(value))])
            }
            Pattern::StringLit { value } => {
                self.inline_struct("StringLit", vec![("value", format_debug(value))])
            }
            Pattern::BoolLit { value } => {
                self.inline_struct("BoolLit", vec![("value", value.to_string())])
            }
        }
    }

    fn runtime_type_test_text(&self, metadata: &RuntimeTypeTestMetadata) -> String {
        self.inline_struct(
            "RuntimeTypeTestMetadata",
            vec![
                ("source_ty", self.type_text(metadata.source_ty)),
                ("target_ty", self.type_text(metadata.target_ty)),
                (
                    "descriptor",
                    self.runtime_type_descriptor_key_text(&metadata.descriptor),
                ),
                ("static_fold", format_debug(&metadata.static_fold)),
                (
                    "parameterized",
                    self.runtime_type_parameterized_match_text(&metadata.parameterized),
                ),
            ],
        )
    }

    fn runtime_pattern_type_test_text(&self, metadata: &RuntimePatternTypeTestMetadata) -> String {
        self.inline_struct(
            "RuntimePatternTypeTestMetadata",
            vec![
                ("subject_ty", self.type_text(metadata.subject_ty)),
                ("target_ty", self.type_text(metadata.target_ty)),
                (
                    "descriptor",
                    self.runtime_type_descriptor_key_text(&metadata.descriptor),
                ),
                ("match_kind", format_debug(&metadata.match_kind)),
                ("static_fold", format_debug(&metadata.static_fold)),
                (
                    "parameterized",
                    self.runtime_type_parameterized_match_text(&metadata.parameterized),
                ),
            ],
        )
    }

    fn runtime_type_descriptor_key_text(&self, key: &RuntimeTypeDescriptorKey) -> String {
        self.inline_struct(
            "RuntimeTypeDescriptorKey",
            vec![
                ("ty", self.type_text(key.ty)),
                ("kind", self.runtime_type_descriptor_kind_text(&key.kind)),
            ],
        )
    }

    fn runtime_type_descriptor_kind_text(&self, kind: &RuntimeTypeDescriptorKind) -> String {
        match kind {
            RuntimeTypeDescriptorKind::Any => "Any".to_string(),
            RuntimeTypeDescriptorKind::String => "String".to_string(),
            RuntimeTypeDescriptorKind::Function => "Function".to_string(),
            RuntimeTypeDescriptorKind::Option => "Option".to_string(),
            RuntimeTypeDescriptorKind::Tuple => "Tuple".to_string(),
            RuntimeTypeDescriptorKind::Value => "Value".to_string(),
            RuntimeTypeDescriptorKind::TypeParam => "TypeParam".to_string(),
            RuntimeTypeDescriptorKind::StarProjection => "StarProjection".to_string(),
            RuntimeTypeDescriptorKind::Union => "Union".to_string(),
            RuntimeTypeDescriptorKind::Nominal { fqn, kind } => self.inline_struct(
                "Nominal",
                vec![
                    ("fqn", format_debug(fqn)),
                    ("kind", self.option_text(kind.as_ref().map(format_debug))),
                ],
            ),
        }
    }

    fn runtime_type_parameterized_match_text(
        &self,
        match_: &RuntimeTypeParameterizedMatch,
    ) -> String {
        match match_ {
            RuntimeTypeParameterizedMatch::None => "None".to_string(),
            RuntimeTypeParameterizedMatch::Nominal {
                type_args,
                effect_arg,
            } => self.inline_struct(
                "Nominal",
                vec![
                    (
                        "type_args",
                        self.list_text(type_args.iter().map(|&ty| self.type_text(ty)).collect()),
                    ),
                    (
                        "effect_arg",
                        self.option_text(
                            effect_arg
                                .as_ref()
                                .map(|row| format_effect_row(self.types, row)),
                        ),
                    ),
                ],
            ),
            RuntimeTypeParameterizedMatch::Function {
                receiver,
                params,
                return_ty,
                effects,
                effects_closed,
            } => self.inline_struct(
                "Function",
                vec![
                    (
                        "receiver",
                        self.option_text(receiver.map(|ty| self.type_text(ty))),
                    ),
                    (
                        "params",
                        self.list_text(params.iter().map(|&ty| self.type_text(ty)).collect()),
                    ),
                    ("return_ty", self.type_text(*return_ty)),
                    ("effects", format_effect_row(self.types, effects)),
                    ("effects_closed", effects_closed.to_string()),
                ],
            ),
            RuntimeTypeParameterizedMatch::Option { payload_ty } => {
                self.inline_struct("Option", vec![("payload_ty", self.type_text(*payload_ty))])
            }
            RuntimeTypeParameterizedMatch::Tuple { element_tys } => self.inline_struct(
                "Tuple",
                vec![(
                    "element_tys",
                    self.list_text(element_tys.iter().map(|&ty| self.type_text(ty)).collect()),
                )],
            ),
            RuntimeTypeParameterizedMatch::Union { variants } => self.inline_struct(
                "Union",
                vec![(
                    "variants",
                    self.list_text(variants.iter().map(|&ty| self.type_text(ty)).collect()),
                )],
            ),
            RuntimeTypeParameterizedMatch::StarProjection { read_ty } => self.inline_struct(
                "StarProjection",
                vec![("read_ty", self.type_text(*read_ty))],
            ),
        }
    }

    fn runtime_cast_text(&self, metadata: &RuntimeCastMetadata) -> String {
        self.inline_struct(
            "RuntimeCastMetadata",
            vec![
                ("test", self.runtime_type_test_text(&metadata.test)),
                ("failure", self.runtime_cast_failure_text(&metadata.failure)),
                ("result", self.runtime_cast_result_text(&metadata.result)),
            ],
        )
    }

    fn runtime_cast_failure_text(&self, failure: &RuntimeCastFailure) -> String {
        match failure {
            RuntimeCastFailure::Raise {
                effect_ty,
                error_fqn,
            } => self.inline_struct(
                "Raise",
                vec![
                    (
                        "effect_ty",
                        self.option_text(effect_ty.map(|ty| self.type_text(ty))),
                    ),
                    ("error_fqn", format_debug(error_fqn)),
                ],
            ),
            RuntimeCastFailure::ReturnNone => "ReturnNone".to_string(),
        }
    }

    fn runtime_cast_result_text(&self, result: &RuntimeCastResult) -> String {
        match result {
            RuntimeCastResult::Target { ty } => {
                self.inline_struct("Target", vec![("ty", self.type_text(*ty))])
            }
            RuntimeCastResult::Option { option_ty, some_ty } => self.inline_struct(
                "Option",
                vec![
                    ("option_ty", self.type_text(*option_ty)),
                    ("some_ty", self.type_text(*some_ty)),
                ],
            ),
        }
    }

    fn type_metadata_literal_text(&self, literal: &TypeMetadataLiteral) -> String {
        self.inline_struct(
            "TypeMetadataLiteral",
            vec![
                ("source_ty", self.type_text(literal.source_ty)),
                (
                    "source_fqn",
                    self.option_text(literal.source_fqn.as_ref().map(format_debug)),
                ),
                ("kind", format_debug(&literal.kind)),
            ],
        )
    }

    fn interpolated_string_part_text(
        &self,
        ctx: &BodyRenderCtx<'_>,
        part: &super::InterpolatedStringPart,
    ) -> String {
        match part {
            super::InterpolatedStringPart::Text { span } => {
                self.inline_struct("Text", vec![("span", format_debug(span))])
            }
            super::InterpolatedStringPart::Expr { span, value, ty } => self.inline_struct(
                "Expr",
                vec![
                    ("span", format_debug(span)),
                    ("value", self.operand_text(ctx, value)),
                    ("ty", self.type_text(*ty)),
                ],
            ),
        }
    }

    fn struct_lit_field_text(&self, ctx: &BodyRenderCtx<'_>, field: &StructLitField) -> String {
        self.inline_struct(
            "StructLitField",
            vec![
                ("span", format_debug(&field.span)),
                ("name", format_debug(&field.name)),
                ("value", self.operand_text(ctx, &field.value)),
            ],
        )
    }

    fn value_transport_text(
        &self,
        _ctx: Option<&BodyRenderCtx<'_>>,
        metadata: &ValueTransportMetadata,
    ) -> String {
        self.inline_struct(
            "ValueTransportMetadata",
            vec![
                ("source_ty", self.type_text(metadata.source_ty)),
                ("kind", format_debug(&metadata.kind)),
                (
                    "requirements",
                    self.transport_requirements_text(&metadata.requirements),
                ),
                (
                    "boxing",
                    self.option_text(
                        metadata
                            .boxing
                            .as_ref()
                            .map(|boxing| self.boxing_intent_text(boxing)),
                    ),
                ),
            ],
        )
    }

    fn transport_requirements_text(&self, req: &MirTransportRequirements) -> String {
        self.inline_struct(
            "MirTransportRequirements",
            vec![
                ("trace", req.trace.to_string()),
                ("copy", req.copy.to_string()),
                ("drop", req.drop.to_string()),
            ],
        )
    }

    fn boxing_intent_text(&self, intent: &MirBoxingIntent) -> String {
        self.inline_struct(
            "MirBoxingIntent",
            vec![
                ("source_ty", self.type_text(intent.source_ty)),
                (
                    "target_ty",
                    self.option_text(intent.target_ty.map(|ty| self.type_text(ty))),
                ),
                ("reason", format_debug(&intent.reason)),
            ],
        )
    }

    fn aggregate_transport_text(
        &self,
        ctx: Option<&BodyRenderCtx<'_>>,
        metadata: &AggregateTransportMetadata,
    ) -> String {
        self.inline_struct(
            "AggregateTransportMetadata",
            vec![
                ("aggregate_ty", self.type_text(metadata.aggregate_ty)),
                ("kind", format_debug(&metadata.kind)),
                (
                    "fields",
                    self.list_text(
                        metadata
                            .fields
                            .iter()
                            .map(|field| self.aggregate_transport_field_text(ctx, field))
                            .collect(),
                    ),
                ),
            ],
        )
    }

    fn aggregate_transport_field_text(
        &self,
        ctx: Option<&BodyRenderCtx<'_>>,
        field: &AggregateTransportField,
    ) -> String {
        self.inline_struct(
            "AggregateTransportField",
            vec![
                ("index", field.index.to_string()),
                (
                    "name",
                    self.option_text(field.name.as_ref().map(format_debug)),
                ),
                ("ty", self.type_text(field.ty)),
                (
                    "transport",
                    self.value_transport_text(ctx, &field.transport),
                ),
            ],
        )
    }

    fn closure_env_transport_text(
        &self,
        ctx: Option<&BodyRenderCtx<'_>>,
        metadata: &super::ClosureEnvTransportMetadata,
    ) -> String {
        self.inline_struct(
            "ClosureEnvTransportMetadata",
            vec![
                ("env_ty", self.type_text(metadata.env_ty)),
                (
                    "captures",
                    self.list_text(
                        metadata
                            .captures
                            .iter()
                            .map(|capture| self.closure_capture_transport_text(ctx, capture))
                            .collect(),
                    ),
                ),
            ],
        )
    }

    fn closure_capture_transport_text(
        &self,
        ctx: Option<&BodyRenderCtx<'_>>,
        capture: &super::ClosureCaptureTransportMetadata,
    ) -> String {
        let source_local = ctx
            .map(|ctx| self.local_label(&ctx.labels, capture.source_local))
            .unwrap_or_else(|| format!("local?{}", capture.source_local.as_u32()));
        self.inline_struct(
            "ClosureCaptureTransportMetadata",
            vec![
                ("name", format_debug(&capture.name)),
                ("decl_span", format_debug(&capture.decl_span)),
                ("mutable", capture.mutable.to_string()),
                ("source_local", source_local),
                (
                    "transport",
                    self.value_transport_text(ctx, &capture.transport),
                ),
            ],
        )
    }

    fn array_transport_text(
        &self,
        ctx: &BodyRenderCtx<'_>,
        metadata: &ArrayElementTransportMetadata,
    ) -> String {
        self.inline_struct(
            "ArrayElementTransportMetadata",
            vec![
                ("operation", format_debug(&metadata.operation)),
                ("array_ty", self.type_text(metadata.array_ty)),
                ("element_ty", self.type_text(metadata.element_ty)),
                ("mutable", metadata.mutable.to_string()),
                (
                    "element",
                    self.value_transport_text(Some(ctx), &metadata.element),
                ),
            ],
        )
    }

    fn gc_transport_text(
        &self,
        ctx: &BodyRenderCtx<'_>,
        metadata: &GcIntrinsicTransportMetadata,
    ) -> String {
        self.inline_struct(
            "GcIntrinsicTransportMetadata",
            vec![
                ("callee_fqn", format_debug(&metadata.callee_fqn)),
                ("operation", format_debug(&metadata.operation)),
                ("root_lifetime", format_debug(&metadata.root_lifetime)),
                ("pairing", format_debug(&metadata.pairing)),
                ("unsafe_required", metadata.unsafe_required.to_string()),
                ("subject_ty", self.type_text(metadata.subject_ty)),
                (
                    "token_ty",
                    self.option_text(metadata.token_ty.map(|ty| self.type_text(ty))),
                ),
                (
                    "subject",
                    self.value_transport_text(Some(ctx), &metadata.subject),
                ),
            ],
        )
    }

    fn call_abi_text(&self, metadata: &CallAbiHandoffMetadata) -> String {
        self.inline_struct(
            "CallAbiHandoffMetadata",
            vec![
                (
                    "callable_abi_kind",
                    format_debug(&metadata.callable_abi_kind),
                ),
                (
                    "resolved_outward_cases",
                    self.list_text(
                        metadata
                            .resolved_outward_cases
                            .iter()
                            .map(format_debug)
                            .collect(),
                    ),
                ),
                ("impl_plan", format_debug(&metadata.impl_plan)),
                ("adapter_required", metadata.adapter_required.to_string()),
            ],
        )
    }

    fn call_transport_text(
        &self,
        ctx: &BodyRenderCtx<'_>,
        metadata: &CallTransportMetadata,
    ) -> String {
        self.inline_struct(
            "CallTransportMetadata",
            vec![
                (
                    "result",
                    self.value_transport_text(Some(ctx), &metadata.result),
                ),
                (
                    "aggregate_return",
                    self.option_text(
                        metadata
                            .aggregate_return
                            .as_ref()
                            .map(|transport| self.value_transport_text(Some(ctx), transport)),
                    ),
                ),
                (
                    "array",
                    self.option_text(
                        metadata
                            .array
                            .as_ref()
                            .map(|array| self.array_transport_text(ctx, array)),
                    ),
                ),
                (
                    "gc",
                    self.option_text(
                        metadata
                            .gc
                            .as_ref()
                            .map(|gc| self.gc_transport_text(ctx, gc)),
                    ),
                ),
                ("abi", self.call_abi_text(&metadata.abi)),
            ],
        )
    }

    fn decl_type_param_text(&self, param: &DeclTypeParamMetadata) -> String {
        self.inline_struct(
            "DeclTypeParamMetadata",
            vec![
                ("span", format_debug(&param.span)),
                ("name", format_debug(&param.name)),
                (
                    "variance",
                    self.option_text(param.variance.as_ref().map(format_debug)),
                ),
                ("ty", self.type_text(param.ty)),
            ],
        )
    }

    fn supertype_metadata_text(&self, metadata: &SupertypeMetadata) -> String {
        self.inline_struct(
            "SupertypeMetadata",
            vec![
                ("span", format_debug(&metadata.span)),
                (
                    "fqn",
                    self.option_text(metadata.fqn.as_ref().map(format_debug)),
                ),
                ("ty", self.type_text(metadata.ty)),
                ("ctor_arg_count", metadata.ctor_arg_count.to_string()),
            ],
        )
    }

    fn ctor_metadata_text(&self, metadata: &CtorMetadata) -> String {
        self.inline_struct(
            "CtorMetadata",
            vec![
                ("span", format_debug(&metadata.span)),
                ("kind", format_debug(&metadata.kind)),
                (
                    "params",
                    self.list_text(
                        metadata
                            .params
                            .iter()
                            .map(|param| self.ctor_param_text(param))
                            .collect(),
                    ),
                ),
                (
                    "delegation",
                    self.option_text(metadata.delegation.as_ref().map(format_debug)),
                ),
            ],
        )
    }

    fn ctor_param_text(&self, param: &CtorParamMetadata) -> String {
        self.inline_struct(
            "CtorParamMetadata",
            vec![
                ("span", format_debug(&param.span)),
                ("name", format_debug(&param.name)),
                ("ty", self.type_text(param.ty)),
                ("has_default", param.has_default.to_string()),
                (
                    "property",
                    self.option_text(param.property.as_ref().map(format_debug)),
                ),
            ],
        )
    }

    fn decl_member_metadata_text(&self, member: &DeclMemberMetadata) -> String {
        match member {
            DeclMemberMetadata::Field(field) => {
                self.inline_enum("Field", self.field_metadata_text(field))
            }
            DeclMemberMetadata::Property(prop) => {
                self.inline_enum("Property", self.property_metadata_text(prop))
            }
            DeclMemberMetadata::Fun(fun) => self.inline_enum("Fun", self.member_fun_text(fun)),
            DeclMemberMetadata::EnumVariant(variant) => {
                self.inline_enum("EnumVariant", self.enum_variant_text(variant))
            }
            DeclMemberMetadata::InitBlock { span } => {
                self.inline_struct("InitBlock", vec![("span", format_debug(span))])
            }
            DeclMemberMetadata::Nested(root) => {
                self.inline_enum("Nested", self.metadata_root_text(root))
            }
        }
    }

    fn metadata_root_text(&self, root: &MetadataRoot) -> String {
        match root {
            MetadataRoot::TypeAlias(alias) => {
                self.inline_enum("TypeAlias", self.type_alias_metadata_text(alias))
            }
            MetadataRoot::Nominal(nominal) => {
                self.inline_enum("Nominal", self.nominal_metadata_text(nominal))
            }
            MetadataRoot::Object(object) => {
                self.inline_enum("Object", self.object_metadata_text(object))
            }
            MetadataRoot::ExtensionProperty(prop) => self.inline_enum(
                "ExtensionProperty",
                self.extension_property_metadata_text(prop),
            ),
        }
    }

    fn type_alias_metadata_text(&self, alias: &TypeAliasMetadata) -> String {
        self.inline_struct(
            "TypeAliasMetadata",
            vec![
                ("span", format_debug(&alias.span)),
                ("fqn", format_debug(&alias.fqn)),
                ("name", format_debug(&alias.name)),
                (
                    "type_params",
                    self.list_text(
                        alias
                            .type_params
                            .iter()
                            .map(|param| self.decl_type_param_text(param))
                            .collect(),
                    ),
                ),
                ("ty", self.type_text(alias.ty)),
            ],
        )
    }

    fn nominal_metadata_text(&self, nominal: &NominalMetadata) -> String {
        self.inline_struct(
            "NominalMetadata",
            vec![
                ("span", format_debug(&nominal.span)),
                ("fqn", format_debug(&nominal.fqn)),
                ("name", format_debug(&nominal.name)),
                ("kind", format_debug(&nominal.kind)),
                (
                    "type_params",
                    self.list_text(
                        nominal
                            .type_params
                            .iter()
                            .map(|param| self.decl_type_param_text(param))
                            .collect(),
                    ),
                ),
                (
                    "supertypes",
                    self.list_text(
                        nominal
                            .supertypes
                            .iter()
                            .map(|supertype| self.supertype_metadata_text(supertype))
                            .collect(),
                    ),
                ),
                (
                    "interfaces",
                    self.list_text(nominal.interfaces.iter().map(format_debug).collect()),
                ),
                (
                    "constructors",
                    self.list_text(
                        nominal
                            .constructors
                            .iter()
                            .map(|ctor| self.ctor_metadata_text(ctor))
                            .collect(),
                    ),
                ),
                (
                    "members",
                    self.list_text(
                        nominal
                            .members
                            .iter()
                            .map(|member| self.decl_member_metadata_text(member))
                            .collect(),
                    ),
                ),
            ],
        )
    }

    fn object_metadata_text(&self, object: &ObjectMetadata) -> String {
        self.inline_struct(
            "ObjectMetadata",
            vec![
                ("span", format_debug(&object.span)),
                ("fqn", format_debug(&object.fqn)),
                ("name", format_debug(&object.name)),
                ("kind", format_debug(&object.kind)),
                (
                    "supertypes",
                    self.list_text(
                        object
                            .supertypes
                            .iter()
                            .map(|supertype| self.supertype_metadata_text(supertype))
                            .collect(),
                    ),
                ),
                (
                    "interfaces",
                    self.list_text(object.interfaces.iter().map(format_debug).collect()),
                ),
                ("initializer_root", format_debug(&object.initializer_root)),
                (
                    "members",
                    self.list_text(
                        object
                            .members
                            .iter()
                            .map(|member| self.decl_member_metadata_text(member))
                            .collect(),
                    ),
                ),
            ],
        )
    }

    fn extension_property_metadata_text(&self, prop: &ExtensionPropertyMetadata) -> String {
        self.inline_struct(
            "ExtensionPropertyMetadata",
            vec![
                ("span", format_debug(&prop.span)),
                ("fqn", format_debug(&prop.fqn)),
                ("name", format_debug(&prop.name)),
                ("mutable", prop.mutable.to_string()),
                (
                    "type_params",
                    self.list_text(
                        prop.type_params
                            .iter()
                            .map(|param| self.decl_type_param_text(param))
                            .collect(),
                    ),
                ),
                ("receiver_ty", self.type_text(prop.receiver_ty)),
                ("ty", self.type_text(prop.ty)),
                (
                    "getter",
                    self.option_text(
                        prop.getter
                            .as_ref()
                            .map(|accessor| self.accessor_text(accessor)),
                    ),
                ),
                (
                    "setter",
                    self.option_text(
                        prop.setter
                            .as_ref()
                            .map(|accessor| self.accessor_text(accessor)),
                    ),
                ),
            ],
        )
    }

    fn field_metadata_text(&self, field: &FieldMetadata) -> String {
        self.inline_struct(
            "FieldMetadata",
            vec![
                ("span", format_debug(&field.span)),
                ("fqn", format_debug(&field.fqn)),
                ("name", format_debug(&field.name)),
                ("mutable", field.mutable.to_string()),
                ("ty", self.type_text(field.ty)),
                ("origin", format_debug(&field.origin)),
            ],
        )
    }

    fn property_metadata_text(&self, prop: &PropertyMetadata) -> String {
        self.inline_struct(
            "PropertyMetadata",
            vec![
                ("span", format_debug(&prop.span)),
                ("fqn", format_debug(&prop.fqn)),
                ("name", format_debug(&prop.name)),
                ("mutable", prop.mutable.to_string()),
                ("ty", self.type_text(prop.ty)),
                ("has_backing_field", prop.has_backing_field.to_string()),
                (
                    "getter",
                    self.option_text(
                        prop.getter
                            .as_ref()
                            .map(|accessor| self.accessor_text(accessor)),
                    ),
                ),
                (
                    "setter",
                    self.option_text(
                        prop.setter
                            .as_ref()
                            .map(|accessor| self.accessor_text(accessor)),
                    ),
                ),
            ],
        )
    }

    fn accessor_text(&self, accessor: &AccessorMetadata) -> String {
        self.inline_struct(
            "AccessorMetadata",
            vec![
                ("span", format_debug(&accessor.span)),
                ("fqn", format_debug(&accessor.fqn)),
            ],
        )
    }

    fn member_fun_text(&self, fun: &MemberFunMetadata) -> String {
        self.inline_struct(
            "MemberFunMetadata",
            vec![
                ("span", format_debug(&fun.span)),
                ("fqn", format_debug(&fun.fqn)),
                ("name", format_debug(&fun.name)),
                (
                    "type_params",
                    self.list_text(
                        fun.type_params
                            .iter()
                            .map(|param| self.decl_type_param_text(param))
                            .collect(),
                    ),
                ),
                (
                    "params",
                    self.list_text(
                        fun.params
                            .iter()
                            .map(|param| self.ctor_param_text(param))
                            .collect(),
                    ),
                ),
                ("return_ty", self.type_text(fun.return_ty)),
            ],
        )
    }

    fn enum_variant_text(&self, variant: &EnumVariantMetadata) -> String {
        self.inline_struct(
            "EnumVariantMetadata",
            vec![
                ("span", format_debug(&variant.span)),
                ("fqn", format_debug(&variant.fqn)),
                ("name", format_debug(&variant.name)),
                (
                    "fields",
                    self.list_text(
                        variant
                            .fields
                            .iter()
                            .map(|field| self.field_metadata_text(field))
                            .collect(),
                    ),
                ),
            ],
        )
    }

    fn initializer_dependency_text(&self, dep: &InitializerDependency) -> String {
        self.inline_struct(
            "InitializerDependency",
            vec![
                ("fqn", format_debug(&dep.fqn)),
                ("kind", format_debug(&dep.kind)),
            ],
        )
    }

    fn materialized_instance_display(
        &self,
        materialized: &MaterializedMir,
        instance: &super::InstanceKey,
    ) -> String {
        let stable_key = materialized
            .authoritative_stable_instance_key(instance)
            .expect("materialized instance must have a stable key");
        let mut args = stable_key.canonical_type_args().to_vec();
        args.extend(
            stable_key
                .canonical_effect_args()
                .iter()
                .map(|row| format!("eff {row}")),
        );
        if args.is_empty() {
            stable_key.readable_path().to_string()
        } else {
            format!("{}::<{}>", stable_key.readable_path(), args.join(", "))
        }
    }

    fn local_label(&self, labels: &BodyLabels, local: LocalId) -> String {
        labels.locals[local.as_u32() as usize].clone()
    }

    fn block_label(&self, labels: &BodyLabels, block: BasicBlockId) -> String {
        labels.blocks[block.as_u32() as usize].clone()
    }

    fn site_label(&self, labels: &BodyLabels, site: SiteId) -> String {
        labels
            .sites
            .get(&site)
            .cloned()
            .unwrap_or_else(|| format!("site_missing#{}", site.as_u32()))
    }

    fn type_text(&self, ty: crate::ty::TypeId) -> String {
        format_type(self.types, ty)
    }

    fn inline_struct(&self, name: &str, fields: Vec<(&str, String)>) -> String {
        let body = fields
            .into_iter()
            .map(|(name, value)| format!("{name}: {value}"))
            .collect::<Vec<_>>()
            .join(", ");
        format!("{name} {{ {body} }}")
    }

    fn inline_enum(&self, name: &str, value: String) -> String {
        format!("{name}({value})")
    }

    fn list_text(&self, items: Vec<String>) -> String {
        format!("[{}]", items.join(", "))
    }

    fn option_text(&self, value: Option<String>) -> String {
        value
            .map(|value| format!("Some({value})"))
            .unwrap_or_else(|| "None".to_string())
    }

    fn open_struct(&mut self, name: &str) {
        self.line(&format!("{name} {{"));
        self.out.push_indent();
    }

    fn close_struct(&mut self, suffix: &str) {
        self.out.pop_indent();
        self.line(&format!("}}{suffix}"));
    }

    fn open_list_field(&mut self, name: &str) {
        self.line(&format!("{name}: ["));
        self.out.push_indent();
    }

    fn close_list_field(&mut self) {
        self.out.pop_indent();
        self.line("],");
    }

    fn render_variant(&mut self, name: &str, render: impl FnOnce(&mut Self)) {
        self.line(&format!("{name}("));
        self.out.push_indent();
        render(self);
        self.out.pop_indent();
        self.line("),");
    }

    fn field_debug<T>(&mut self, name: &str, value: &T)
    where
        T: std::fmt::Debug + ?Sized,
    {
        self.line(&format!("{name}: {},", format_debug(value)));
    }

    fn field_raw(&mut self, name: &str, value: &str) {
        self.line(&format!("{name}: {value},"));
    }

    fn field_option_text(&mut self, name: &str, value: Option<String>) {
        self.field_raw(name, &self.option_text(value));
    }

    fn field_bool(&mut self, name: &str, value: bool) {
        self.line(&format!("{name}: {value},"));
    }

    fn line(&mut self, text: &str) {
        self.out.line(text);
    }
}

fn next_ordinal(map: &mut HashMap<String, usize>, signature: &str) -> usize {
    let ordinal = map.get(signature).copied().unwrap_or(0);
    map.insert(signature.to_string(), ordinal + 1);
    ordinal
}
