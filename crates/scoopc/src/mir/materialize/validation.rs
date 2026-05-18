//! Validation pass that walks a materialized MIR and verifies every artifact (instance keys, items, payload contracts, transports, terminators, patterns, type metadata) against the published contract before any consumer is allowed to read it.

use super::*;

#[derive(Clone, Copy)]
pub(super) struct MaterializedValidationContext<'a> {
    pub(super) fqn: &'a str,
    pub(super) block: Option<BasicBlockId>,
    pub(super) span: Span,
    pub(super) surface: &'static str,
}

impl<'a> MaterializedValidationContext<'a> {
    pub(super) fn with_surface(self, surface: &'static str) -> Self {
        Self { surface, ..self }
    }
}

#[derive(Clone, Copy)]
pub(super) struct MaterializedRootSets<'a> {
    pub(super) known_roots: &'a HashSet<String>,
    pub(super) generic_templates: &'a HashSet<String>,
}

pub(super) fn validate_materialized_mir(materialized: &MaterializedMir) -> MaterializeResult<()> {
    let known_roots = collect_materialized_known_roots(materialized);
    let generic_templates = materialized
        .instance_keys
        .iter()
        .map(|key| key.template.fqn.clone())
        .collect::<HashSet<_>>();

    for key in &materialized.instance_keys {
        validate_materialized_instance_key(materialized, key)?;
    }

    for item in &materialized.file.items {
        validate_materialized_item(materialized, item, &known_roots, &generic_templates)?;
    }

    let pass_view = materialized.pass_view();
    let mut seen = HashSet::new();
    for family in pass_view.instances() {
        for fun in family.callable_bodies() {
            if seen.insert(fun.fqn.clone()) {
                validate_materialized_fun(materialized, fun, &known_roots, &generic_templates)?;
            }
        }
    }

    Ok(())
}

pub(super) fn collect_materialized_known_roots(materialized: &MaterializedMir) -> HashSet<String> {
    let mut roots = HashSet::new();
    for item in &materialized.file.items {
        match item {
            Item::Fun(fun) => {
                roots.insert(fun.fqn.clone());
            }
            Item::InitializerRoot(root) => {
                roots.insert(root.fqn.clone());
            }
            Item::ExternGlobal(root) => {
                roots.insert(root.fqn.clone());
            }
            Item::Metadata(root) => {
                roots.insert(root.fqn().to_string());
            }
            Item::Todo { .. } => {}
        }
    }
    let pass_view = materialized.pass_view();
    for family in pass_view.instances() {
        roots.insert(family.root_fqn().to_string());
        roots.extend(family.callable_fqns().map(str::to_string));
    }
    roots
}

fn materialized_type_contract_err(
    fqn: &str,
    block: Option<BasicBlockId>,
    span: Span,
    surface: &'static str,
    detail: &'static str,
) -> Box<MirMaterializeError> {
    materialize_err(MirMaterializeError::MaterializedMirValidation {
        fqn: fqn.to_string(),
        error: super::super::MirValidationError::TypeContract {
            fqn: fqn.to_string(),
            block,
            span,
            surface,
            detail,
        },
    })
}

pub(super) fn validate_materialized_instance_key(
    materialized: &MaterializedMir,
    key: &InstanceKey,
) -> MaterializeResult<()> {
    for &ty in &key.type_args {
        validate_materialized_type(
            materialized,
            MaterializedValidationContext {
                fqn: &key.template.fqn,
                block: None,
                span: key.template.decl_span,
                surface: "instance type arg",
            },
            ty,
        )?;
    }
    for row in &key.eff_args {
        validate_materialized_effect_row(
            materialized,
            MaterializedValidationContext {
                fqn: &key.template.fqn,
                block: None,
                span: key.template.decl_span,
                surface: "instance effect arg",
            },
            row,
        )?;
    }
    Ok(())
}

pub(super) fn validate_materialized_item(
    materialized: &MaterializedMir,
    item: &Item,
    known_roots: &HashSet<String>,
    generic_templates: &HashSet<String>,
) -> MaterializeResult<()> {
    match item {
        Item::Fun(fun) => {
            validate_materialized_fun(materialized, fun, known_roots, generic_templates)
        }
        Item::InitializerRoot(root) => validate_materialized_initializer_root(materialized, root),
        Item::ExternGlobal(root) => validate_materialized_extern_global_root(materialized, root),
        Item::Metadata(root) => validate_materialized_metadata_root(materialized, root),
        Item::Todo { span, kind } => Err(materialize_err(MirMaterializeError::MaterializedTodo {
            fqn: "<file>".to_string(),
            block: None,
            span: *span,
            category: MirPlaceholderCategory::Item,
            reason: kind,
        })),
    }
}

pub(super) fn validate_materialized_initializer_root(
    materialized: &MaterializedMir,
    root: &InitializerRoot,
) -> MaterializeResult<()> {
    if let Some(ty) = root.ty {
        validate_materialized_type(
            materialized,
            MaterializedValidationContext {
                fqn: &root.fqn,
                block: None,
                span: root.span,
                surface: "initializer root type",
            },
            ty,
        )?;
    }
    if let Some(transport) = &root.initializer_transport {
        validate_materialized_value_transport(
            materialized,
            &root.fqn,
            BasicBlockId::from_raw(0),
            root.span,
            "initializer value transport",
            transport,
        )?;
    }
    validate_materialized_effect_row(
        materialized,
        MaterializedValidationContext {
            fqn: &root.fqn,
            block: None,
            span: root.span,
            surface: "initializer root hidden effects",
        },
        &root.hidden_effects,
    )
}

pub(super) fn validate_materialized_extern_global_root(
    materialized: &MaterializedMir,
    root: &ExternGlobalRoot,
) -> MaterializeResult<()> {
    validate_materialized_type(
        materialized,
        MaterializedValidationContext {
            fqn: &root.fqn,
            block: None,
            span: root.span,
            surface: "extern global type",
        },
        root.ty,
    )
}

pub(super) fn validate_materialized_metadata_root(
    materialized: &MaterializedMir,
    root: &MetadataRoot,
) -> MaterializeResult<()> {
    match root {
        MetadataRoot::TypeAlias(alias) => {
            validate_materialized_typealias_metadata(materialized, alias)
        }
        MetadataRoot::Nominal(nominal) => {
            validate_materialized_nominal_metadata(materialized, nominal)
        }
        MetadataRoot::Object(object) => validate_materialized_object_metadata(materialized, object),
        MetadataRoot::ExtensionProperty(prop) => {
            validate_materialized_extension_property_metadata(materialized, prop)
        }
    }
}

pub(super) fn validate_materialized_typealias_metadata(
    materialized: &MaterializedMir,
    alias: &TypeAliasMetadata,
) -> MaterializeResult<()> {
    validate_materialized_decl_type_params(materialized, &alias.fqn, &alias.type_params)?;
    validate_materialized_type(
        materialized,
        MaterializedValidationContext {
            fqn: &alias.fqn,
            block: None,
            span: alias.span,
            surface: "typealias target type",
        },
        alias.ty,
    )
}

pub(super) fn validate_materialized_nominal_metadata(
    materialized: &MaterializedMir,
    nominal: &NominalMetadata,
) -> MaterializeResult<()> {
    validate_materialized_decl_type_params(materialized, &nominal.fqn, &nominal.type_params)?;
    for supertype in &nominal.supertypes {
        validate_materialized_supertype_metadata(materialized, &nominal.fqn, supertype)?;
    }
    for ctor in &nominal.constructors {
        for param in &ctor.params {
            validate_materialized_type(
                materialized,
                MaterializedValidationContext {
                    fqn: &nominal.fqn,
                    block: None,
                    span: param.span,
                    surface: "constructor parameter type",
                },
                param.ty,
            )?;
        }
    }
    validate_materialized_decl_members(materialized, &nominal.fqn, &nominal.members)
}

pub(super) fn validate_materialized_object_metadata(
    materialized: &MaterializedMir,
    object: &ObjectMetadata,
) -> MaterializeResult<()> {
    for supertype in &object.supertypes {
        validate_materialized_supertype_metadata(materialized, &object.fqn, supertype)?;
    }
    validate_materialized_decl_members(materialized, &object.fqn, &object.members)
}

pub(super) fn validate_materialized_extension_property_metadata(
    materialized: &MaterializedMir,
    prop: &ExtensionPropertyMetadata,
) -> MaterializeResult<()> {
    validate_materialized_decl_type_params(materialized, &prop.fqn, &prop.type_params)?;
    validate_materialized_type(
        materialized,
        MaterializedValidationContext {
            fqn: &prop.fqn,
            block: None,
            span: prop.span,
            surface: "extension receiver type",
        },
        prop.receiver_ty,
    )?;
    validate_materialized_type(
        materialized,
        MaterializedValidationContext {
            fqn: &prop.fqn,
            block: None,
            span: prop.span,
            surface: "extension property type",
        },
        prop.ty,
    )
}

pub(super) fn validate_materialized_decl_type_params(
    materialized: &MaterializedMir,
    fqn: &str,
    params: &[DeclTypeParamMetadata],
) -> MaterializeResult<()> {
    for param in params {
        validate_materialized_type(
            materialized,
            MaterializedValidationContext {
                fqn,
                block: None,
                span: param.span,
                surface: "declaration type parameter",
            },
            param.ty,
        )?;
    }
    Ok(())
}

pub(super) fn validate_materialized_supertype_metadata(
    materialized: &MaterializedMir,
    fqn: &str,
    supertype: &SupertypeMetadata,
) -> MaterializeResult<()> {
    validate_materialized_type(
        materialized,
        MaterializedValidationContext {
            fqn,
            block: None,
            span: supertype.span,
            surface: "supertype metadata",
        },
        supertype.ty,
    )
}

pub(super) fn validate_materialized_decl_members(
    materialized: &MaterializedMir,
    owner_fqn: &str,
    members: &[DeclMemberMetadata],
) -> MaterializeResult<()> {
    for member in members {
        match member {
            DeclMemberMetadata::Field(field) => {
                validate_materialized_field_metadata(materialized, owner_fqn, field)?;
            }
            DeclMemberMetadata::Property(prop) => {
                validate_materialized_property_metadata(materialized, owner_fqn, prop)?;
            }
            DeclMemberMetadata::Fun(fun) => {
                validate_materialized_member_fun_metadata(materialized, owner_fqn, fun)?;
            }
            DeclMemberMetadata::EnumVariant(variant) => {
                for field in &variant.fields {
                    validate_materialized_field_metadata(materialized, owner_fqn, field)?;
                }
            }
            DeclMemberMetadata::InitBlock { .. } => {}
            DeclMemberMetadata::Nested(root) => {
                validate_materialized_metadata_root(materialized, root)?;
            }
        }
    }
    Ok(())
}

pub(super) fn validate_materialized_field_metadata(
    materialized: &MaterializedMir,
    owner_fqn: &str,
    field: &FieldMetadata,
) -> MaterializeResult<()> {
    validate_materialized_type(
        materialized,
        MaterializedValidationContext {
            fqn: owner_fqn,
            block: None,
            span: field.span,
            surface: "field type",
        },
        field.ty,
    )
}

pub(super) fn validate_materialized_property_metadata(
    materialized: &MaterializedMir,
    owner_fqn: &str,
    prop: &PropertyMetadata,
) -> MaterializeResult<()> {
    validate_materialized_type(
        materialized,
        MaterializedValidationContext {
            fqn: owner_fqn,
            block: None,
            span: prop.span,
            surface: "property type",
        },
        prop.ty,
    )
}

pub(super) fn validate_materialized_member_fun_metadata(
    materialized: &MaterializedMir,
    owner_fqn: &str,
    fun: &MemberFunMetadata,
) -> MaterializeResult<()> {
    validate_materialized_decl_type_params(materialized, owner_fqn, &fun.type_params)?;
    for param in &fun.params {
        validate_materialized_type(
            materialized,
            MaterializedValidationContext {
                fqn: owner_fqn,
                block: None,
                span: param.span,
                surface: "member function parameter type",
            },
            param.ty,
        )?;
    }
    validate_materialized_type(
        materialized,
        MaterializedValidationContext {
            fqn: owner_fqn,
            block: None,
            span: fun.span,
            surface: "member function return type",
        },
        fun.return_ty,
    )
}

pub(super) fn validate_materialized_fun(
    materialized: &MaterializedMir,
    fun: &FunDecl,
    known_roots: &HashSet<String>,
    generic_templates: &HashSet<String>,
) -> MaterializeResult<()> {
    validate_materialized_type(
        materialized,
        MaterializedValidationContext {
            fqn: &fun.fqn,
            block: None,
            span: fun.span,
            surface: "function type",
        },
        fun.ty,
    )?;
    validate_materialized_type(
        materialized,
        MaterializedValidationContext {
            fqn: &fun.fqn,
            block: None,
            span: fun.span,
            surface: "return type",
        },
        fun.return_ty,
    )?;
    for param in &fun.params {
        validate_materialized_param(materialized, &fun.fqn, param)?;
    }

    let Some(body) = &fun.body else {
        return Ok(());
    };
    body.validate_direct_style().map_err(|error| {
        materialize_err(MirMaterializeError::MaterializedMirValidation {
            fqn: fun.fqn.clone(),
            error,
        })
    })?;
    validate_materialized_signature_locals(fun, body)?;

    for local in &body.locals {
        validate_materialized_type(
            materialized,
            MaterializedValidationContext {
                fqn: &fun.fqn,
                block: None,
                span: local.span,
                surface: "frame slot",
            },
            local.ty,
        )?;
    }

    for (block_index, block) in body.blocks.iter().enumerate() {
        let block_id = BasicBlockId::from_raw(block_index as u32);
        for stmt in &block.stmts {
            validate_materialized_statement(
                materialized,
                &fun.fqn,
                block_id,
                &body.locals,
                stmt,
                MaterializedRootSets {
                    known_roots,
                    generic_templates,
                },
            )?;
        }
        validate_materialized_unwind_action(
            block.terminator.span,
            &fun.fqn,
            block_id,
            &block.terminator.unwind,
        )?;
        validate_materialized_terminator(
            materialized,
            fun,
            block_id,
            &body.locals,
            &block.terminator,
        )?;
    }

    Ok(())
}

pub(super) fn validate_materialized_signature_locals(
    fun: &FunDecl,
    body: &Body,
) -> MaterializeResult<()> {
    for param in &fun.params {
        let Some(local) = body.locals.get(param.local.as_u32() as usize) else {
            return Err(materialized_type_contract_err(
                &fun.fqn,
                None,
                param.span,
                "parameter local",
                "parameter local is outside the body local table",
            ));
        };
        if local.ty != param.ty {
            return Err(materialized_type_contract_err(
                &fun.fqn,
                None,
                param.span,
                "parameter type",
                "parameter type and parameter local type disagree",
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_materialized_param(
    materialized: &MaterializedMir,
    fqn: &str,
    param: &Param,
) -> MaterializeResult<()> {
    validate_materialized_type(
        materialized,
        MaterializedValidationContext {
            fqn,
            block: None,
            span: param.span,
            surface: "parameter type",
        },
        param.ty,
    )
}

pub(super) fn validate_materialized_statement(
    materialized: &MaterializedMir,
    fqn: &str,
    block: BasicBlockId,
    locals: &[LocalDecl],
    stmt: &Statement,
    root_sets: MaterializedRootSets<'_>,
) -> MaterializeResult<()> {
    match &stmt.kind {
        StatementKind::Assign { target, value } => {
            validate_materialized_local(
                fqn,
                Some(block),
                stmt.span,
                "assignment target",
                locals,
                *target,
            )?;
            validate_materialized_rvalue(
                materialized,
                fqn,
                block,
                stmt.span,
                locals,
                value,
                root_sets,
            )
        }
        StatementKind::StoreMember {
            receiver,
            member,
            value,
            value_ty,
            continuation_route,
        } => {
            validate_materialized_operand(
                materialized,
                fqn,
                block,
                stmt.span,
                "member store receiver",
                locals,
                receiver,
            )?;
            validate_materialized_member_metadata(materialized, fqn, block, stmt.span, member)?;
            let receiver_ty = materialized_operand_ty(
                materialized,
                fqn,
                block,
                stmt.span,
                "member store receiver",
                locals,
                receiver,
            )?;
            if receiver_ty.is_some_and(|ty| ty != member.receiver_ty) {
                return Err(materialized_type_contract_err(
                    fqn,
                    Some(block),
                    stmt.span,
                    "member store receiver",
                    "receiver operand type and member receiver type disagree",
                ));
            }
            validate_materialized_operand(
                materialized,
                fqn,
                block,
                stmt.span,
                "member store value",
                locals,
                value,
            )?;
            let operand_ty = materialized_operand_ty(
                materialized,
                fqn,
                block,
                stmt.span,
                "member store value",
                locals,
                value,
            )?;
            if operand_ty.is_some_and(|ty| ty != *value_ty) {
                return Err(materialized_type_contract_err(
                    fqn,
                    Some(block),
                    stmt.span,
                    "member store value",
                    "value operand type and published value type disagree",
                ));
            }
            validate_materialized_type(
                materialized,
                MaterializedValidationContext {
                    fqn,
                    block: Some(block),
                    span: stmt.span,
                    surface: "member store value type",
                },
                *value_ty,
            )?;
            if let crate::mir::StoredContinuationRoutePublication::Unique(route) =
                continuation_route
            {
                validate_materialized_type(
                    materialized,
                    MaterializedValidationContext {
                        fqn,
                        block: Some(block),
                        span: stmt.span,
                        surface: "stored continuation source type",
                    },
                    route.source_ty,
                )?;
            }
            Ok(())
        }
        StatementKind::StoreTopLevelVar {
            value, value_ty, ..
        } => {
            validate_materialized_operand(
                materialized,
                fqn,
                block,
                stmt.span,
                "top-level store value",
                locals,
                value,
            )?;
            validate_materialized_type(
                materialized,
                MaterializedValidationContext {
                    fqn,
                    block: Some(block),
                    span: stmt.span,
                    surface: "top-level store value type",
                },
                *value_ty,
            )
        }
        StatementKind::Todo(reason) => {
            Err(materialize_err(MirMaterializeError::MaterializedTodo {
                fqn: fqn.to_string(),
                block: Some(block),
                span: stmt.span,
                category: MirPlaceholderCategory::Statement,
                reason,
            }))
        }
        StatementKind::Nop => Ok(()),
    }
}

pub(super) fn validate_materialized_rvalue(
    materialized: &MaterializedMir,
    fqn: &str,
    block: BasicBlockId,
    span: Span,
    locals: &[LocalDecl],
    value: &Rvalue,
    root_sets: MaterializedRootSets<'_>,
) -> MaterializeResult<()> {
    match value {
        Rvalue::Use(operand) => validate_materialized_operand(
            materialized,
            fqn,
            block,
            span,
            "source value",
            locals,
            operand,
        ),
        Rvalue::Transport { value, transport } => {
            validate_materialized_operand(
                materialized,
                fqn,
                block,
                span,
                "transport value",
                locals,
                value,
            )?;
            validate_materialized_value_transport(
                materialized,
                fqn,
                block,
                span,
                "value erasure transport",
                transport,
            )
        }
        Rvalue::TopLevelRef(top) => {
            validate_materialized_top_level_ref(materialized, fqn, block, span, top, root_sets)
        }
        Rvalue::UnresolvedName { .. } => Ok(()),
        Rvalue::TypeCheck {
            value,
            test_ty,
            metadata,
            ..
        } => {
            validate_materialized_operand(
                materialized,
                fqn,
                block,
                span,
                "typecheck value",
                locals,
                value,
            )?;
            validate_materialized_type(
                materialized,
                MaterializedValidationContext {
                    fqn,
                    block: Some(block),
                    span,
                    surface: "typecheck target type",
                },
                *test_ty,
            )?;
            validate_materialized_type_test_metadata(
                materialized,
                MaterializedValidationContext {
                    fqn,
                    block: Some(block),
                    span,
                    surface: "typecheck metadata",
                },
                metadata,
            )
        }
        Rvalue::Cast {
            value,
            target_ty,
            metadata,
            ..
        } => {
            validate_materialized_operand(
                materialized,
                fqn,
                block,
                span,
                "cast value",
                locals,
                value,
            )?;
            validate_materialized_type(
                materialized,
                MaterializedValidationContext {
                    fqn,
                    block: Some(block),
                    span,
                    surface: "cast target type",
                },
                *target_ty,
            )?;
            validate_materialized_cast_metadata(
                materialized,
                MaterializedValidationContext {
                    fqn,
                    block: Some(block),
                    span,
                    surface: "cast metadata",
                },
                metadata,
            )
        }
        Rvalue::MemberAccess {
            receiver, member, ..
        } => {
            validate_materialized_operand(
                materialized,
                fqn,
                block,
                span,
                "member receiver",
                locals,
                receiver,
            )?;
            validate_materialized_member_metadata(materialized, fqn, block, span, member)?;
            let receiver_ty = materialized_operand_ty(
                materialized,
                fqn,
                block,
                span,
                "member receiver",
                locals,
                receiver,
            )?;
            if receiver_ty.is_some_and(|ty| ty != member.receiver_ty) {
                return Err(materialized_type_contract_err(
                    fqn,
                    Some(block),
                    span,
                    "member receiver",
                    "receiver operand type and member receiver type disagree",
                ));
            }
            Ok(())
        }
        Rvalue::EnumVariant {
            enum_ty,
            args,
            payload,
            ..
        } => {
            validate_materialized_type(
                materialized,
                MaterializedValidationContext {
                    fqn,
                    block: Some(block),
                    span,
                    surface: "enum transport type",
                },
                *enum_ty,
            )?;
            validate_materialized_call_args(materialized, fqn, block, span, locals, args)?;
            validate_materialized_aggregate_transport(
                materialized,
                fqn,
                block,
                span,
                "enum payload transport",
                payload,
            )
        }
        Rvalue::ClassCtor {
            ctor,
            args,
            hidden_effects,
            ..
        } => {
            if ctor.ordered_param_count != args.len() {
                return Err(materialized_type_contract_err(
                    fqn,
                    Some(block),
                    span,
                    "class constructor arguments",
                    "ordered parameter count and lowered argument count disagree",
                ));
            }
            if let Some(arg) = args.iter().find(|arg| arg.name.is_some()) {
                return Err(materialized_type_contract_err(
                    fqn,
                    Some(block),
                    arg.span,
                    "class constructor arguments",
                    "materialized constructor arguments must be positional",
                ));
            }
            validate_materialized_effect_row(
                materialized,
                MaterializedValidationContext {
                    fqn,
                    block: Some(block),
                    span,
                    surface: "class constructor hidden effects",
                },
                hidden_effects,
            )?;
            validate_materialized_call_args(materialized, fqn, block, span, locals, args)
        }
        Rvalue::Call {
            kind,
            args,
            transport,
            ..
        } => {
            validate_materialized_call_args(materialized, fqn, block, span, locals, args)?;
            validate_materialized_call_kind(
                materialized,
                fqn,
                block,
                span,
                locals,
                kind,
                root_sets,
            )?;
            validate_materialized_call_transport(materialized, fqn, block, span, transport)
        }
        Rvalue::MakeTuple {
            elements,
            transport,
        } => {
            validate_materialized_operands(
                materialized,
                fqn,
                block,
                span,
                "tuple aggregate element",
                locals,
                elements,
            )?;
            validate_materialized_aggregate_transport(
                materialized,
                fqn,
                block,
                span,
                "tuple aggregate transport",
                transport,
            )
        }
        Rvalue::StructLit { fields, transport } => {
            for field in fields {
                validate_materialized_struct_lit_field(
                    materialized,
                    fqn,
                    block,
                    span,
                    locals,
                    field,
                )?;
            }
            validate_materialized_aggregate_transport(
                materialized,
                fqn,
                block,
                span,
                "struct aggregate transport",
                transport,
            )
        }
        Rvalue::SizeOf { value_ty } => validate_materialized_type(
            materialized,
            MaterializedValidationContext {
                fqn,
                block: Some(block),
                span,
                surface: "sizeof type argument",
            },
            *value_ty,
        ),
        Rvalue::KindOf { value_ty } => validate_materialized_type(
            materialized,
            MaterializedValidationContext {
                fqn,
                block: Some(block),
                span,
                surface: "kindof type argument",
            },
            *value_ty,
        ),
        Rvalue::AlignOf { value_ty } => validate_materialized_type(
            materialized,
            MaterializedValidationContext {
                fqn,
                block: Some(block),
                span,
                surface: "alignof type argument",
            },
            *value_ty,
        ),
        Rvalue::DescOf { value_ty } => validate_materialized_type(
            materialized,
            MaterializedValidationContext {
                fqn,
                block: Some(block),
                span,
                surface: "descof type argument",
            },
            *value_ty,
        ),
        Rvalue::TypeMetadataLiteral(metadata) => {
            validate_materialized_type_metadata_literal(materialized, fqn, block, span, metadata)
        }
        Rvalue::InterpolatedString { .. } => Err(materialized_type_contract_err(
            fqn,
            Some(block),
            span,
            "interpolated string",
            "interpolated strings must be desugared before MIR codegen",
        )),
        Rvalue::TupleGet { tuple, .. } => validate_materialized_operand(
            materialized,
            fqn,
            block,
            span,
            "tuple get source",
            locals,
            tuple,
        ),
        Rvalue::PatternMatch { subject, pattern } => {
            validate_materialized_operand(
                materialized,
                fqn,
                block,
                span,
                "pattern subject",
                locals,
                subject,
            )?;
            validate_materialized_pattern(materialized, fqn, block, span, pattern)
        }
        Rvalue::PatternExtract { subject, .. } => validate_materialized_operand(
            materialized,
            fqn,
            block,
            span,
            "pattern extract subject",
            locals,
            subject,
        ),
        Rvalue::MakeClosure {
            env,
            fn_ptr,
            env_contract,
        } => {
            validate_materialized_operand(
                materialized,
                fqn,
                block,
                span,
                "closure env",
                locals,
                env,
            )?;
            validate_materialized_call_target(
                fqn,
                Some(block),
                span,
                fn_ptr,
                root_sets.known_roots,
                root_sets.generic_templates,
            )?;
            validate_materialized_closure_env_contract(materialized, fqn, block, span, env_contract)
        }
        Rvalue::PerformResult { effect_ty, .. } => validate_materialized_type(
            materialized,
            MaterializedValidationContext {
                fqn,
                block: Some(block),
                span,
                surface: "perform result effect type",
            },
            *effect_ty,
        ),
        Rvalue::Todo(reason) => Err(materialize_err(MirMaterializeError::MaterializedTodo {
            fqn: fqn.to_string(),
            block: Some(block),
            span,
            category: MirPlaceholderCategory::Rvalue,
            reason,
        })),
    }
}

pub(super) fn validate_materialized_type_metadata_literal(
    materialized: &MaterializedMir,
    fqn: &str,
    block: BasicBlockId,
    span: Span,
    metadata: &TypeMetadataLiteral,
) -> MaterializeResult<()> {
    validate_materialized_type(
        materialized,
        MaterializedValidationContext {
            fqn,
            block: Some(block),
            span,
            surface: "type metadata literal source type",
        },
        metadata.source_ty,
    )
}

pub(super) fn validate_materialized_value_transport(
    materialized: &MaterializedMir,
    fqn: &str,
    block: BasicBlockId,
    span: Span,
    surface: &'static str,
    metadata: &ValueTransportMetadata,
) -> MaterializeResult<()> {
    validate_materialized_type(
        materialized,
        MaterializedValidationContext {
            fqn,
            block: Some(block),
            span,
            surface,
        },
        metadata.source_ty,
    )?;
    if let Some(boxing) = &metadata.boxing {
        validate_materialized_type(
            materialized,
            MaterializedValidationContext {
                fqn,
                block: Some(block),
                span,
                surface: "transport boxing source type",
            },
            boxing.source_ty,
        )?;
        if let Some(target_ty) = boxing.target_ty {
            validate_materialized_type(
                materialized,
                MaterializedValidationContext {
                    fqn,
                    block: Some(block),
                    span,
                    surface: "transport boxing target type",
                },
                target_ty,
            )?;
        }
    }
    Ok(())
}

pub(super) fn validate_materialized_aggregate_transport(
    materialized: &MaterializedMir,
    fqn: &str,
    block: BasicBlockId,
    span: Span,
    surface: &'static str,
    metadata: &AggregateTransportMetadata,
) -> MaterializeResult<()> {
    validate_materialized_type(
        materialized,
        MaterializedValidationContext {
            fqn,
            block: Some(block),
            span,
            surface,
        },
        metadata.aggregate_ty,
    )?;
    for field in &metadata.fields {
        validate_materialized_aggregate_transport_field(materialized, fqn, block, span, field)?;
    }
    Ok(())
}

pub(super) fn validate_materialized_aggregate_transport_field(
    materialized: &MaterializedMir,
    fqn: &str,
    block: BasicBlockId,
    span: Span,
    field: &AggregateTransportField,
) -> MaterializeResult<()> {
    validate_materialized_type(
        materialized,
        MaterializedValidationContext {
            fqn,
            block: Some(block),
            span,
            surface: "aggregate transport field type",
        },
        field.ty,
    )?;
    validate_materialized_value_transport(
        materialized,
        fqn,
        block,
        span,
        "aggregate transport field value",
        &field.transport,
    )
}

pub(super) fn validate_materialized_closure_env_contract(
    materialized: &MaterializedMir,
    fqn: &str,
    block: BasicBlockId,
    span: Span,
    contract: &ClosureEnvTransportMetadata,
) -> MaterializeResult<()> {
    validate_materialized_type(
        materialized,
        MaterializedValidationContext {
            fqn,
            block: Some(block),
            span,
            surface: "closure env type",
        },
        contract.env_ty,
    )?;
    for capture in &contract.captures {
        validate_materialized_closure_capture_transport(materialized, fqn, block, span, capture)?;
    }
    Ok(())
}

pub(super) fn validate_materialized_closure_capture_transport(
    materialized: &MaterializedMir,
    fqn: &str,
    block: BasicBlockId,
    span: Span,
    capture: &ClosureCaptureTransportMetadata,
) -> MaterializeResult<()> {
    let _ = capture.source_local;
    validate_materialized_value_transport(
        materialized,
        fqn,
        block,
        span,
        "closure capture transport",
        &capture.transport,
    )
}

pub(super) fn validate_materialized_call_transport(
    materialized: &MaterializedMir,
    fqn: &str,
    block: BasicBlockId,
    span: Span,
    transport: &CallTransportMetadata,
) -> MaterializeResult<()> {
    validate_materialized_value_transport(
        materialized,
        fqn,
        block,
        span,
        "call result transport",
        &transport.result,
    )?;
    if let Some(aggregate_return) = &transport.aggregate_return {
        validate_materialized_value_transport(
            materialized,
            fqn,
            block,
            span,
            "call aggregate return transport",
            aggregate_return,
        )?;
    }
    if let Some(array) = &transport.array {
        validate_materialized_array_transport(materialized, fqn, block, span, array)?;
    }
    if let Some(gc) = &transport.gc {
        validate_materialized_gc_intrinsic_transport(materialized, fqn, block, span, gc)?;
    }
    Ok(())
}

pub(super) fn validate_materialized_gc_intrinsic_transport(
    materialized: &MaterializedMir,
    fqn: &str,
    block: BasicBlockId,
    span: Span,
    gc: &GcIntrinsicTransportMetadata,
) -> MaterializeResult<()> {
    validate_materialized_type(
        materialized,
        MaterializedValidationContext {
            fqn,
            block: Some(block),
            span,
            surface: "GC intrinsic subject type",
        },
        gc.subject_ty,
    )?;
    if let Some(token_ty) = gc.token_ty {
        validate_materialized_type(
            materialized,
            MaterializedValidationContext {
                fqn,
                block: Some(block),
                span,
                surface: "GC intrinsic token type",
            },
            token_ty,
        )?;
    }
    validate_materialized_value_transport(
        materialized,
        fqn,
        block,
        span,
        "GC intrinsic subject transport",
        &gc.subject,
    )
}

pub(super) fn validate_materialized_array_transport(
    materialized: &MaterializedMir,
    fqn: &str,
    block: BasicBlockId,
    span: Span,
    array: &ArrayElementTransportMetadata,
) -> MaterializeResult<()> {
    validate_materialized_type(
        materialized,
        MaterializedValidationContext {
            fqn,
            block: Some(block),
            span,
            surface: "array transport array type",
        },
        array.array_ty,
    )?;
    validate_materialized_type(
        materialized,
        MaterializedValidationContext {
            fqn,
            block: Some(block),
            span,
            surface: "array transport element type",
        },
        array.element_ty,
    )?;
    validate_materialized_value_transport(
        materialized,
        fqn,
        block,
        span,
        "array element transport",
        &array.element,
    )
}

pub(super) fn validate_materialized_top_level_ref(
    materialized: &MaterializedMir,
    fqn: &str,
    block: BasicBlockId,
    span: Span,
    top: &TopLevelRef,
    root_sets: MaterializedRootSets<'_>,
) -> MaterializeResult<()> {
    validate_materialized_effect_row(
        materialized,
        MaterializedValidationContext {
            fqn,
            block: Some(block),
            span,
            surface: "top-level root hidden effects",
        },
        &top.hidden_effects,
    )?;
    validate_materialized_call_target(
        fqn,
        Some(block),
        span,
        &top.fqn,
        root_sets.known_roots,
        root_sets.generic_templates,
    )
}

pub(super) fn validate_materialized_call_args(
    materialized: &MaterializedMir,
    fqn: &str,
    block: BasicBlockId,
    span: Span,
    locals: &[LocalDecl],
    args: &[CallArg],
) -> MaterializeResult<()> {
    for arg in args {
        validate_materialized_operand(
            materialized,
            fqn,
            block,
            arg.span,
            "call arg",
            locals,
            &arg.value,
        )?;
    }
    let _ = span;
    Ok(())
}

pub(super) fn validate_materialized_call_kind(
    materialized: &MaterializedMir,
    fqn: &str,
    block: BasicBlockId,
    span: Span,
    locals: &[LocalDecl],
    kind: &CallKind,
    root_sets: MaterializedRootSets<'_>,
) -> MaterializeResult<()> {
    match kind {
        CallKind::Direct { callee_fqn } => validate_materialized_call_target(
            fqn,
            Some(block),
            span,
            callee_fqn,
            root_sets.known_roots,
            root_sets.generic_templates,
        ),
        CallKind::Closure { callee, fn_ptr } => {
            validate_materialized_operand(
                materialized,
                fqn,
                block,
                span,
                "closure callee",
                locals,
                callee,
            )?;
            validate_materialized_call_target(
                fqn,
                Some(block),
                span,
                fn_ptr,
                root_sets.known_roots,
                root_sets.generic_templates,
            )
        }
        CallKind::FunValue { callee } | CallKind::FunPtr { callee } => {
            validate_materialized_operand(
                materialized,
                fqn,
                block,
                span,
                "callable-value callee",
                locals,
                callee,
            )
        }
        CallKind::Virtual { receiver, dispatch } | CallKind::Interface { receiver, dispatch } => {
            validate_materialized_operand(
                materialized,
                fqn,
                block,
                span,
                "dispatch receiver",
                locals,
                receiver,
            )?;
            validate_materialized_type(
                materialized,
                MaterializedValidationContext {
                    fqn,
                    block: Some(block),
                    span,
                    surface: "dispatch receiver type",
                },
                dispatch.receiver_ty,
            )
        }
        CallKind::Resume {
            continuation,
            resume,
        } => {
            validate_materialized_operand(
                materialized,
                fqn,
                block,
                span,
                "resume continuation",
                locals,
                continuation,
            )?;
            validate_materialized_resume_metadata(materialized, fqn, block, span, resume)
        }
    }
}

pub(super) fn validate_materialized_resume_metadata(
    materialized: &MaterializedMir,
    fqn: &str,
    block: BasicBlockId,
    span: Span,
    resume: &super::super::ResumeMetadata,
) -> MaterializeResult<()> {
    for (surface, ty) in [
        ("resume continuation type", resume.continuation_ty),
        ("resume payload type", resume.resume_ty),
        ("resume answer type", resume.answer_ty),
        ("resume return type", resume.return_ty),
    ] {
        validate_materialized_type(
            materialized,
            MaterializedValidationContext {
                fqn,
                block: Some(block),
                span,
                surface,
            },
            ty,
        )?;
    }
    validate_materialized_effect_row(
        materialized,
        MaterializedValidationContext {
            fqn,
            block: Some(block),
            span,
            surface: "resume out effects",
        },
        &resume.out_effects,
    )?;
    if let Some(runtime_error) = resume.runtime_error_effect_ty {
        validate_materialized_type(
            materialized,
            MaterializedValidationContext {
                fqn,
                block: Some(block),
                span,
                surface: "resume runtime-error effect type",
            },
            runtime_error,
        )?;
    }
    Ok(())
}

pub(super) fn validate_materialized_struct_lit_field(
    materialized: &MaterializedMir,
    fqn: &str,
    block: BasicBlockId,
    span: Span,
    locals: &[LocalDecl],
    field: &StructLitField,
) -> MaterializeResult<()> {
    validate_materialized_operand(
        materialized,
        fqn,
        block,
        field.span,
        "struct aggregate field",
        locals,
        &field.value,
    )?;
    let _ = span;
    Ok(())
}

pub(super) fn validate_materialized_terminator(
    materialized: &MaterializedMir,
    fun: &FunDecl,
    block: BasicBlockId,
    locals: &[LocalDecl],
    terminator: &Terminator,
) -> MaterializeResult<()> {
    match &terminator.kind {
        TerminatorKind::Return { value: Some(value) } => {
            materialized_operand_ty(
                materialized,
                &fun.fqn,
                block,
                terminator.span,
                "return value",
                locals,
                value,
            )?;
            Ok(())
        }
        TerminatorKind::Return { value: None } => {
            let builtins = materialized
                .types
                .builtins()
                .expect("materialized MIR should always intern builtin types before validation");
            if fun.return_ty != builtins.unit {
                return Err(materialize_err(
                    MirMaterializeError::MaterializedMirValidation {
                        fqn: fun.fqn.clone(),
                        error: super::super::MirValidationError::ProductionMissingReturnValue {
                            fqn: fun.fqn.clone(),
                            block,
                            span: terminator.span,
                            return_ty: fun.return_ty,
                        },
                    },
                ));
            }
            Ok(())
        }
        TerminatorKind::Perform { metadata, args, .. } => {
            validate_materialized_perform_metadata(
                materialized,
                &fun.fqn,
                block,
                terminator.span,
                metadata,
            )?;
            for arg in args {
                validate_materialized_perform_arg(
                    materialized,
                    &fun.fqn,
                    block,
                    terminator.span,
                    locals,
                    arg,
                )?;
            }
            Ok(())
        }
        TerminatorKind::Handle { metadata, arms, .. } => {
            validate_materialized_handle_metadata(
                materialized,
                &fun.fqn,
                block,
                terminator.span,
                metadata,
            )?;
            for arm in arms {
                validate_materialized_handler_arm(
                    materialized,
                    &fun.fqn,
                    block,
                    terminator.span,
                    arm,
                )?;
            }
            Ok(())
        }
        TerminatorKind::CondBr { cond, .. } => validate_materialized_bool_operand(
            materialized,
            &fun.fqn,
            block,
            terminator.span,
            "branch condition",
            locals,
            cond,
        ),
        TerminatorKind::Todo(reason) => {
            Err(materialize_err(MirMaterializeError::MaterializedTodo {
                fqn: fun.fqn.clone(),
                block: Some(block),
                span: terminator.span,
                category: MirPlaceholderCategory::Terminator,
                reason,
            }))
        }
        TerminatorKind::ResumeUnwind
        | TerminatorKind::Goto { .. }
        | TerminatorKind::Unreachable => Ok(()),
    }
}

pub(super) fn validate_materialized_perform_arg(
    materialized: &MaterializedMir,
    fqn: &str,
    block: BasicBlockId,
    span: Span,
    locals: &[LocalDecl],
    arg: &PerformArg,
) -> MaterializeResult<()> {
    validate_materialized_operand(
        materialized,
        fqn,
        block,
        arg.span,
        "perform payload arg",
        locals,
        &arg.value,
    )?;
    let _ = span;
    Ok(())
}

pub(super) fn validate_materialized_perform_metadata(
    materialized: &MaterializedMir,
    fqn: &str,
    block: BasicBlockId,
    span: Span,
    metadata: &PerformMetadata,
) -> MaterializeResult<()> {
    validate_materialized_type(
        materialized,
        MaterializedValidationContext {
            fqn,
            block: Some(block),
            span,
            surface: "perform effect type",
        },
        metadata.effect_ty,
    )?;
    validate_materialized_type(
        materialized,
        MaterializedValidationContext {
            fqn,
            block: Some(block),
            span,
            surface: "perform result type",
        },
        metadata.result_ty,
    )?;
    if let Some(payload_tuple_ty) = metadata.payload_tuple_ty {
        validate_materialized_type(
            materialized,
            MaterializedValidationContext {
                fqn,
                block: Some(block),
                span,
                surface: "perform payload tuple type",
            },
            payload_tuple_ty,
        )?;
    }
    for &payload_ty in &metadata.payload_component_tys {
        validate_materialized_type(
            materialized,
            MaterializedValidationContext {
                fqn,
                block: Some(block),
                span,
                surface: "perform payload component type",
            },
            payload_ty,
        )?;
    }
    for payload in &metadata.payload_transport {
        validate_materialized_value_transport(
            materialized,
            fqn,
            block,
            span,
            "perform payload transport",
            payload,
        )?;
    }
    Ok(())
}

pub(super) fn validate_materialized_handle_metadata(
    materialized: &MaterializedMir,
    fqn: &str,
    block: BasicBlockId,
    span: Span,
    metadata: &HandleMetadata,
) -> MaterializeResult<()> {
    for (surface, ty) in [
        ("handle result type", Some(metadata.result_ty)),
        ("handle body result type", Some(metadata.body_result_ty)),
        ("handle finally result type", metadata.finally_result_ty),
    ] {
        if let Some(ty) = ty {
            validate_materialized_type(
                materialized,
                MaterializedValidationContext {
                    fqn,
                    block: Some(block),
                    span,
                    surface,
                },
                ty,
            )?;
        }
    }
    Ok(())
}

pub(super) fn validate_materialized_handler_arm(
    materialized: &MaterializedMir,
    fqn: &str,
    block: BasicBlockId,
    span: Span,
    arm: &HandlerArm,
) -> MaterializeResult<()> {
    for (surface, ty) in [
        ("handler arm effect type", Some(arm.handled_effect_ty)),
        ("handler arm payload tuple type", arm.payload_tuple_ty),
        ("handler arm body type", Some(arm.body_ty)),
    ] {
        if let Some(ty) = ty {
            validate_materialized_type(
                materialized,
                MaterializedValidationContext {
                    fqn,
                    block: Some(block),
                    span,
                    surface,
                },
                ty,
            )?;
        }
    }
    for &payload_ty in &arm.payload_component_tys {
        validate_materialized_type(
            materialized,
            MaterializedValidationContext {
                fqn,
                block: Some(block),
                span,
                surface: "handler arm payload component type",
            },
            payload_ty,
        )?;
    }
    Ok(())
}

pub(super) fn validate_materialized_unwind_action(
    span: Span,
    fqn: &str,
    block: BasicBlockId,
    unwind: &UnwindAction,
) -> MaterializeResult<()> {
    match unwind {
        UnwindAction::Todo(reason) => Err(materialize_err(MirMaterializeError::MaterializedTodo {
            fqn: fqn.to_string(),
            block: Some(block),
            span,
            category: MirPlaceholderCategory::UnwindAction,
            reason,
        })),
        UnwindAction::NoUnwind | UnwindAction::Propagate | UnwindAction::Cleanup { .. } => Ok(()),
    }
}

pub(super) fn validate_materialized_member_metadata(
    materialized: &MaterializedMir,
    fqn: &str,
    block: BasicBlockId,
    span: Span,
    member: &MemberAccessMetadata,
) -> MaterializeResult<()> {
    validate_materialized_type(
        materialized,
        MaterializedValidationContext {
            fqn,
            block: Some(block),
            span,
            surface: "member receiver type",
        },
        member.receiver_ty,
    )?;
    validate_materialized_effect_row(
        materialized,
        MaterializedValidationContext {
            fqn,
            block: Some(block),
            span,
            surface: "member hidden effects",
        },
        &member.hidden_effects,
    )
}

pub(super) fn validate_materialized_type_test_metadata(
    materialized: &MaterializedMir,
    ctx: MaterializedValidationContext<'_>,
    metadata: &RuntimeTypeTestMetadata,
) -> MaterializeResult<()> {
    validate_materialized_type(
        materialized,
        ctx.with_surface("type-test source type"),
        metadata.source_ty,
    )?;
    validate_materialized_type(
        materialized,
        ctx.with_surface("type-test target type"),
        metadata.target_ty,
    )?;
    validate_materialized_descriptor_key(
        materialized,
        ctx.with_surface("type-test descriptor"),
        &metadata.descriptor,
    )?;
    validate_materialized_parameterized_match(
        materialized,
        ctx.with_surface("type-test parameterized match"),
        &metadata.parameterized,
    )
}

pub(super) fn validate_materialized_cast_metadata(
    materialized: &MaterializedMir,
    ctx: MaterializedValidationContext<'_>,
    metadata: &RuntimeCastMetadata,
) -> MaterializeResult<()> {
    validate_materialized_type_test_metadata(
        materialized,
        ctx.with_surface("cast type test"),
        &metadata.test,
    )?;
    match &metadata.failure {
        RuntimeCastFailure::Raise { effect_ty, .. } => {
            if let Some(effect_ty) = effect_ty {
                validate_materialized_type(
                    materialized,
                    ctx.with_surface("cast failure effect"),
                    *effect_ty,
                )?;
            }
        }
        RuntimeCastFailure::ReturnNone => {}
    }
    match &metadata.result {
        RuntimeCastResult::Target { ty } => {
            validate_materialized_type(materialized, ctx.with_surface("cast result type"), *ty)
        }
        RuntimeCastResult::Option { option_ty, some_ty } => {
            validate_materialized_type(
                materialized,
                ctx.with_surface("cast optional result type"),
                *option_ty,
            )?;
            validate_materialized_type(materialized, ctx.with_surface("cast some type"), *some_ty)
        }
    }
}

pub(super) fn validate_materialized_pattern_type_test_metadata(
    materialized: &MaterializedMir,
    ctx: MaterializedValidationContext<'_>,
    metadata: &RuntimePatternTypeTestMetadata,
) -> MaterializeResult<()> {
    validate_materialized_type(
        materialized,
        ctx.with_surface("pattern subject type"),
        metadata.subject_ty,
    )?;
    validate_materialized_type(
        materialized,
        ctx.with_surface("pattern target type"),
        metadata.target_ty,
    )?;
    validate_materialized_descriptor_key(
        materialized,
        ctx.with_surface("pattern descriptor"),
        &metadata.descriptor,
    )?;
    validate_materialized_parameterized_match(
        materialized,
        ctx.with_surface("pattern parameterized match"),
        &metadata.parameterized,
    )
}

pub(super) fn validate_materialized_descriptor_key(
    materialized: &MaterializedMir,
    ctx: MaterializedValidationContext<'_>,
    descriptor: &RuntimeTypeDescriptorKey,
) -> MaterializeResult<()> {
    validate_materialized_type(materialized, ctx, descriptor.ty)
}

pub(super) fn validate_materialized_parameterized_match(
    materialized: &MaterializedMir,
    ctx: MaterializedValidationContext<'_>,
    parameterized: &RuntimeTypeParameterizedMatch,
) -> MaterializeResult<()> {
    match parameterized {
        RuntimeTypeParameterizedMatch::None => Ok(()),
        RuntimeTypeParameterizedMatch::Nominal {
            type_args,
            effect_arg,
        } => {
            for ty in type_args {
                validate_materialized_type(
                    materialized,
                    ctx.with_surface("nominal type arg"),
                    *ty,
                )?;
            }
            if let Some(effect_arg) = effect_arg {
                validate_materialized_effect_row(
                    materialized,
                    ctx.with_surface("nominal effect arg"),
                    effect_arg,
                )?;
            }
            Ok(())
        }
        RuntimeTypeParameterizedMatch::Function {
            receiver,
            params,
            return_ty,
            effects,
            ..
        } => {
            if let Some(receiver) = receiver {
                validate_materialized_type(
                    materialized,
                    ctx.with_surface("function receiver type"),
                    *receiver,
                )?;
            }
            for param in params {
                validate_materialized_type(
                    materialized,
                    ctx.with_surface("function param type"),
                    *param,
                )?;
            }
            validate_materialized_type(
                materialized,
                ctx.with_surface("function return type"),
                *return_ty,
            )?;
            validate_materialized_effect_row(
                materialized,
                ctx.with_surface("function effects"),
                effects,
            )
        }
        RuntimeTypeParameterizedMatch::Option { payload_ty } => validate_materialized_type(
            materialized,
            ctx.with_surface("option payload type"),
            *payload_ty,
        ),
        RuntimeTypeParameterizedMatch::Tuple { element_tys } => {
            for element_ty in element_tys {
                validate_materialized_type(
                    materialized,
                    ctx.with_surface("tuple element type"),
                    *element_ty,
                )?;
            }
            Ok(())
        }
        RuntimeTypeParameterizedMatch::Union { variants } => {
            for variant in variants {
                validate_materialized_type(
                    materialized,
                    ctx.with_surface("union variant"),
                    *variant,
                )?;
            }
            Ok(())
        }
        RuntimeTypeParameterizedMatch::StarProjection { read_ty } => validate_materialized_type(
            materialized,
            ctx.with_surface("star projection read type"),
            *read_ty,
        ),
    }
}

pub(super) fn validate_materialized_pattern(
    materialized: &MaterializedMir,
    fqn: &str,
    block: BasicBlockId,
    span: Span,
    pattern: &Pattern,
) -> MaterializeResult<()> {
    match pattern {
        Pattern::Is { ty, metadata } => {
            validate_materialized_type(
                materialized,
                MaterializedValidationContext {
                    fqn,
                    block: Some(block),
                    span,
                    surface: "pattern type",
                },
                *ty,
            )?;
            validate_materialized_pattern_type_test_metadata(
                materialized,
                MaterializedValidationContext {
                    fqn,
                    block: Some(block),
                    span,
                    surface: "pattern type-test metadata",
                },
                metadata,
            )
        }
        Pattern::Bind { ty, .. } => validate_materialized_type(
            materialized,
            MaterializedValidationContext {
                fqn,
                block: Some(block),
                span,
                surface: "pattern type",
            },
            *ty,
        ),
        Pattern::Or { pats } => {
            for pat in pats {
                validate_materialized_pattern(materialized, fqn, block, span, pat)?;
            }
            Ok(())
        }
        Pattern::Tuple { elements } | Pattern::Variant { args: elements, .. } => {
            for pat in elements {
                validate_materialized_pattern(materialized, fqn, block, span, pat)?;
            }
            Ok(())
        }
        Pattern::Else
        | Pattern::Wildcard
        | Pattern::Rest
        | Pattern::IntLit { .. }
        | Pattern::CharLit { .. }
        | Pattern::StringLit { .. }
        | Pattern::BoolLit { .. } => Ok(()),
    }
}

pub(super) fn validate_materialized_operands(
    materialized: &MaterializedMir,
    fqn: &str,
    block: BasicBlockId,
    span: Span,
    surface: &'static str,
    locals: &[LocalDecl],
    operands: &[Operand],
) -> MaterializeResult<()> {
    for operand in operands {
        validate_materialized_operand(materialized, fqn, block, span, surface, locals, operand)?;
    }
    Ok(())
}

pub(super) fn validate_materialized_local<'a>(
    fqn: &str,
    block: Option<BasicBlockId>,
    span: Span,
    surface: &'static str,
    locals: &'a [LocalDecl],
    local: LocalId,
) -> MaterializeResult<&'a LocalDecl> {
    locals.get(local.as_u32() as usize).ok_or_else(|| {
        materialized_type_contract_err(
            fqn,
            block,
            span,
            surface,
            "local reference is outside the body local table",
        )
    })
}

pub(super) fn validate_materialized_operand(
    materialized: &MaterializedMir,
    fqn: &str,
    block: BasicBlockId,
    span: Span,
    surface: &'static str,
    locals: &[LocalDecl],
    operand: &Operand,
) -> MaterializeResult<()> {
    if let Operand::Local(local) = operand {
        let local_decl =
            validate_materialized_local(fqn, Some(block), span, surface, locals, *local)?;
        validate_materialized_type(
            materialized,
            MaterializedValidationContext {
                fqn,
                block: Some(block),
                span,
                surface,
            },
            local_decl.ty,
        )?;
    }
    Ok(())
}

pub(super) fn validate_materialized_bool_operand(
    materialized: &MaterializedMir,
    fqn: &str,
    block: BasicBlockId,
    span: Span,
    surface: &'static str,
    locals: &[LocalDecl],
    operand: &Operand,
) -> MaterializeResult<()> {
    let expected = materialized
        .types
        .builtins()
        .expect("materialized MIR should always intern builtin types before validation")
        .bool_;
    let actual = match operand {
        Operand::Local(local) => {
            let local_decl =
                validate_materialized_local(fqn, Some(block), span, surface, locals, *local)?;
            validate_materialized_type(
                materialized,
                MaterializedValidationContext {
                    fqn,
                    block: Some(block),
                    span,
                    surface,
                },
                local_decl.ty,
            )?;
            local_decl.ty
        }
        Operand::Const(ConstValue::Bool(_)) => return Ok(()),
        Operand::Const(_) => {
            return Err(materialized_type_contract_err(
                fqn,
                Some(block),
                span,
                surface,
                "branch condition operand must have Bool type",
            ));
        }
    };
    if actual != expected {
        return Err(materialized_type_contract_err(
            fqn,
            Some(block),
            span,
            surface,
            "branch condition operand must have Bool type",
        ));
    }
    Ok(())
}

pub(super) fn materialized_operand_ty(
    materialized: &MaterializedMir,
    fqn: &str,
    block: BasicBlockId,
    span: Span,
    surface: &'static str,
    locals: &[LocalDecl],
    operand: &Operand,
) -> MaterializeResult<Option<TypeId>> {
    match operand {
        Operand::Local(local) => {
            let local_decl =
                validate_materialized_local(fqn, Some(block), span, surface, locals, *local)?;
            validate_materialized_type(
                materialized,
                MaterializedValidationContext {
                    fqn,
                    block: Some(block),
                    span,
                    surface,
                },
                local_decl.ty,
            )?;
            Ok(Some(local_decl.ty))
        }
        Operand::Const(_) => Ok(None),
    }
}

pub(super) fn validate_materialized_type(
    materialized: &MaterializedMir,
    ctx: MaterializedValidationContext<'_>,
    ty: TypeId,
) -> MaterializeResult<()> {
    if type_contains_param(&materialized.types, ty) {
        return Err(materialize_err(
            MirMaterializeError::MaterializedUnresolvedGenericParam {
                fqn: ctx.fqn.to_string(),
                block: ctx.block,
                span: ctx.span,
                surface: ctx.surface,
                ty: materialized.types.display(ty).to_string(),
            },
        ));
    }
    Ok(())
}

pub(super) fn validate_materialized_effect_row(
    materialized: &MaterializedMir,
    ctx: MaterializedValidationContext<'_>,
    row: &EffectRow,
) -> MaterializeResult<()> {
    if effect_row_contains_param(&materialized.types, row) {
        let ty = format!("eff {:?}", EffectRowRepr(row));
        return Err(materialize_err(
            MirMaterializeError::MaterializedUnresolvedGenericParam {
                fqn: ctx.fqn.to_string(),
                block: ctx.block,
                span: ctx.span,
                surface: ctx.surface,
                ty,
            },
        ));
    }
    Ok(())
}

pub(super) fn validate_materialized_call_target(
    fqn: &str,
    block: Option<BasicBlockId>,
    span: Span,
    callee_fqn: &str,
    known_roots: &HashSet<String>,
    generic_templates: &HashSet<String>,
) -> MaterializeResult<()> {
    if is_canonical_array_member_intrinsic_fqn(callee_fqn) {
        return Ok(());
    }
    let unresolved_generic_target = callee_fqn.is_empty()
        || generic_templates.contains(callee_fqn)
        || (callee_fqn.contains("::<") && !known_roots.contains(callee_fqn));
    if unresolved_generic_target {
        return Err(materialize_err(
            MirMaterializeError::MaterializedMissingCallTarget {
                fqn: fqn.to_string(),
                block,
                span,
                callee_fqn: callee_fqn.to_string(),
            },
        ));
    }
    Ok(())
}
