//! Validation pass that walks a materialized MIR and verifies every artifact (instance keys, items, payload contracts, transports, terminators, patterns, type metadata) against the published contract before any consumer is allowed to read it.

use super::super::{RuntimeTypeDescriptorKind, RuntimeTypeStaticFold};
use super::*;
use crate::ast;
use crate::mir::AggregateTransportKind;
use crate::mir::{ClassCtorCallMetadata, DispatchMetadata, InitializerRootKind};
use crate::ty::{RefTypeKind, TypeKind, ValueTypeKind};

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

fn materialized_transport_contract_err(
    fqn: &str,
    block: BasicBlockId,
    span: Span,
    transport: &'static str,
    detail: &'static str,
) -> Box<MirMaterializeError> {
    materialize_err(MirMaterializeError::MaterializedMirValidation {
        fqn: fqn.to_string(),
        error: super::super::MirValidationError::ProductionTransportMetadata {
            fqn: fqn.to_string(),
            block,
            span,
            transport,
            detail,
        },
    })
}

fn materialized_runtime_contract_err(
    fqn: &str,
    block: BasicBlockId,
    span: Span,
    primitive: &'static str,
    detail: &'static str,
) -> Box<MirMaterializeError> {
    materialize_err(MirMaterializeError::MaterializedMirValidation {
        fqn: fqn.to_string(),
        error: super::super::MirValidationError::ProductionRuntimeValueMetadata {
            fqn: fqn.to_string(),
            block,
            span,
            primitive,
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
    if !root.initializer_absent {
        return Err(materialized_type_contract_err(
            &root.fqn,
            None,
            root.span,
            "extern global initializer",
            "extern global roots must not publish an initializer",
        ));
    }
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
            let target_decl = validate_materialized_local(
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
                Some(target_decl.ty),
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
            fqn: target_fqn,
            value,
            value_ty,
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
            let operand_ty = materialized_operand_ty(
                materialized,
                fqn,
                block,
                stmt.span,
                "top-level store value",
                locals,
                value,
            )?;
            if operand_ty.is_some_and(|ty| ty != *value_ty) {
                return Err(materialized_type_contract_err(
                    fqn,
                    Some(block),
                    stmt.span,
                    "top-level store value",
                    "value operand type and published top-level store type disagree",
                ));
            }
            validate_materialized_top_level_store_target(
                materialized,
                fqn,
                block,
                stmt.span,
                target_fqn,
                *value_ty,
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

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_materialized_rvalue(
    materialized: &MaterializedMir,
    fqn: &str,
    block: BasicBlockId,
    span: Span,
    locals: &[LocalDecl],
    value: &Rvalue,
    result_ty: Option<TypeId>,
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
            let source_ty = materialized_operand_ty(
                materialized,
                fqn,
                block,
                span,
                "typecheck value",
                locals,
                value,
            )?;
            validate_materialized_type_test_contract(
                materialized,
                fqn,
                block,
                span,
                source_ty,
                *test_ty,
                result_ty,
                metadata,
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
            op,
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
            let source_ty = materialized_operand_ty(
                materialized,
                fqn,
                block,
                span,
                "cast value",
                locals,
                value,
            )?;
            validate_materialized_cast_contract(
                materialized,
                fqn,
                block,
                span,
                *op,
                source_ty,
                *target_ty,
                result_ty,
                metadata,
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
            variant_name,
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
            let expected_fields = args
                .iter()
                .map(|arg| {
                    materialized_operand_ty(
                        materialized,
                        fqn,
                        block,
                        arg.span,
                        "enum payload argument",
                        locals,
                        &arg.value,
                    )
                    .map(|ty| (arg.name.as_deref(), ty))
                })
                .collect::<MaterializeResult<Vec<_>>>()?;
            validate_materialized_aggregate_schema(
                materialized,
                fqn,
                block,
                span,
                "enum payload transport",
                Some(*enum_ty),
                Some(AggregateTransportKind::EnumPayload),
                Some(variant_name.as_str()),
                &expected_fields,
                payload,
            )
        }
        Rvalue::ClassCtor {
            class_fqn,
            ctor,
            args,
            hidden_effects,
            ..
        } => {
            validate_materialized_class_ctor_contract(
                materialized,
                fqn,
                block,
                span,
                locals,
                class_fqn,
                ctor,
                args,
                result_ty,
            )?;
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
            validate_materialized_call_transport(materialized, fqn, block, span, transport)?;
            validate_materialized_call_abi(
                materialized,
                fqn,
                block,
                span,
                locals,
                kind,
                args,
                transport,
                result_ty,
            )
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
            let expected_fields = elements
                .iter()
                .map(|operand| {
                    materialized_operand_ty(
                        materialized,
                        fqn,
                        block,
                        span,
                        "tuple aggregate element",
                        locals,
                        operand,
                    )
                    .map(|ty| (None, ty))
                })
                .collect::<MaterializeResult<Vec<_>>>()?;
            validate_materialized_aggregate_schema(
                materialized,
                fqn,
                block,
                span,
                "tuple aggregate transport",
                result_ty,
                None,
                None,
                &expected_fields,
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
            let expected_fields = fields
                .iter()
                .map(|field| {
                    materialized_operand_ty(
                        materialized,
                        fqn,
                        block,
                        field.span,
                        "struct aggregate field",
                        locals,
                        &field.value,
                    )
                    .map(|ty| (Some(field.name.as_str()), ty))
                })
                .collect::<MaterializeResult<Vec<_>>>()?;
            validate_materialized_aggregate_schema(
                materialized,
                fqn,
                block,
                span,
                "struct aggregate transport",
                result_ty,
                Some(AggregateTransportKind::Struct),
                None,
                &expected_fields,
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
            let subject_ty = materialized_operand_ty(
                materialized,
                fqn,
                block,
                span,
                "pattern subject",
                locals,
                subject,
            )?;
            validate_materialized_pattern(materialized, fqn, block, span, subject_ty, pattern)
        }
        Rvalue::PatternExtract { subject, path } => {
            validate_materialized_operand(
                materialized,
                fqn,
                block,
                span,
                "pattern extract subject",
                locals,
                subject,
            )?;
            let subject_ty = materialized_operand_ty(
                materialized,
                fqn,
                block,
                span,
                "pattern extract subject",
                locals,
                subject,
            )?;
            validate_materialized_pattern_extract_schema(
                materialized,
                fqn,
                block,
                span,
                subject_ty,
                path,
                result_ty,
            )
        }
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

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_materialized_aggregate_schema(
    materialized: &MaterializedMir,
    fqn: &str,
    block: BasicBlockId,
    span: Span,
    surface: &'static str,
    expected_result_ty: Option<TypeId>,
    expected_kind: Option<AggregateTransportKind>,
    variant_name: Option<&str>,
    expected_fields: &[(Option<&str>, Option<TypeId>)],
    metadata: &AggregateTransportMetadata,
) -> MaterializeResult<()> {
    validate_materialized_aggregate_transport(materialized, fqn, block, span, surface, metadata)?;

    let detail = if expected_result_ty.is_some_and(|ty| ty != metadata.aggregate_ty) {
        Some("aggregate transport type and result/source type disagree")
    } else if expected_kind.is_some_and(|kind| kind != metadata.kind) {
        Some("aggregate transport kind is wrong for this MIR node")
    } else if metadata.fields.len() != expected_fields.len() {
        Some("aggregate transport field count does not match lowered values")
    } else if metadata
        .fields
        .iter()
        .enumerate()
        .any(|(index, field)| field.index != index || field.ty != field.transport.source_ty)
    {
        Some("aggregate transport field metadata is inconsistent")
    } else if metadata.fields.iter().zip(expected_fields.iter()).any(
        |(field, (expected_name, _))| {
            expected_name.is_some_and(|name| field.name.as_deref() != Some(name))
        },
    ) {
        Some("aggregate transport field name does not match lowered value")
    } else if metadata
        .fields
        .iter()
        .zip(expected_fields.iter())
        .any(|(field, (_, expected_ty))| expected_ty.is_some_and(|ty| field.ty != ty))
    {
        Some("aggregate transport field type does not match lowered value")
    } else {
        materialized_aggregate_schema_detail(materialized, metadata, variant_name)
    };

    if let Some(detail) = detail {
        return Err(materialized_transport_contract_err(
            fqn, block, span, surface, detail,
        ));
    }
    Ok(())
}

fn materialized_aggregate_schema_detail(
    materialized: &MaterializedMir,
    metadata: &AggregateTransportMetadata,
    variant_name: Option<&str>,
) -> Option<&'static str> {
    match metadata.kind {
        AggregateTransportKind::Tuple | AggregateTransportKind::ClosureEnv => {
            let TypeKind::Value(ValueTypeKind::Tuple(elements)) =
                materialized.types.kind(metadata.aggregate_ty)
            else {
                return Some("tuple aggregate type must be a tuple");
            };
            if metadata.fields.len() != elements.len() {
                return Some("tuple aggregate field count does not match tuple type");
            }
            if metadata.kind == AggregateTransportKind::Tuple
                && metadata.fields.iter().any(|field| field.name.is_some())
            {
                return Some("tuple aggregate fields must not publish names");
            }
            if metadata
                .fields
                .iter()
                .zip(elements.iter())
                .any(|(field, element_ty)| field.ty != *element_ty)
            {
                return Some("tuple aggregate field type does not match tuple type");
            }
            None
        }
        AggregateTransportKind::Struct => {
            let TypeKind::Value(ValueTypeKind::Nominal(nominal_ty)) =
                materialized.types.kind(metadata.aggregate_ty)
            else {
                return Some("struct aggregate type must be a struct nominal type");
            };
            let nominal = materialized_nominal_metadata_by_fqn(materialized, &nominal_ty.fqn)
                .or_else(|| {
                    materialized_nominal_metadata_by_fqn(
                        materialized,
                        strip_materialized_type_args(&nominal_ty.fqn),
                    )
                })?;
            if nominal.kind != ast::TypeKind::Struct {
                return Some("struct aggregate type must be a struct nominal type");
            }
            let declared_fields = nominal
                .members
                .iter()
                .filter_map(|member| match member {
                    DeclMemberMetadata::Field(field) => Some((field.name.as_str(), field.ty)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            if metadata.fields.len() != declared_fields.len() {
                return Some("struct aggregate field count does not match struct declaration");
            }
            for field in &metadata.fields {
                let Some(name) = field.name.as_deref() else {
                    return Some("struct aggregate field must publish a name");
                };
                let matches = declared_fields
                    .iter()
                    .filter(|(decl_name, _)| *decl_name == name)
                    .count();
                if matches == 0 {
                    return Some("struct aggregate field name is not declared");
                }
                if matches > 1 {
                    return Some("struct declaration contains duplicate aggregate field names");
                }
                let declared_ty = declared_fields
                    .iter()
                    .find_map(|(decl_name, ty)| (*decl_name == name).then_some(*ty))
                    .expect("declared field was just found");
                if field.ty != declared_ty {
                    return Some("struct aggregate field type does not match struct declaration");
                }
                if metadata
                    .fields
                    .iter()
                    .filter(|candidate| candidate.name.as_deref() == Some(name))
                    .count()
                    > 1
                {
                    return Some("struct aggregate field names must be unique");
                }
            }
            None
        }
        AggregateTransportKind::EnumPayload => {
            let Some(variant_name) = variant_name else {
                return Some("enum payload aggregate must publish a variant name");
            };
            let Some(variant_fields) =
                materialized_enum_variant_fields(materialized, metadata.aggregate_ty, variant_name)
            else {
                return Some("enum payload aggregate variant is not declared");
            };
            if metadata.fields.len() != variant_fields.len() {
                return Some("enum payload field count does not match variant declaration");
            }
            if metadata
                .fields
                .iter()
                .zip(variant_fields.iter())
                .any(|(field, (_, ty))| field.ty != *ty)
            {
                return Some("enum payload field type does not match variant declaration");
            }
            None
        }
    }
}

fn materialized_nominal_metadata_for_value_type(
    materialized: &MaterializedMir,
    ty: TypeId,
    kind: ast::TypeKind,
) -> Option<&NominalMetadata> {
    let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = materialized.types.kind(ty) else {
        return None;
    };
    materialized_nominal_metadata_by_fqn(materialized, &nominal.fqn)
        .or_else(|| {
            materialized_nominal_metadata_by_fqn(
                materialized,
                strip_materialized_type_args(&nominal.fqn),
            )
        })
        .filter(|metadata| metadata.kind == kind)
}

fn materialized_nominal_metadata_by_fqn<'a>(
    materialized: &'a MaterializedMir,
    fqn: &str,
) -> Option<&'a NominalMetadata> {
    materialized.file.items.iter().find_map(|item| match item {
        Item::Metadata(MetadataRoot::Nominal(metadata)) if metadata.fqn == fqn => Some(metadata),
        _ => None,
    })
}

fn materialized_enum_variant_fields(
    materialized: &MaterializedMir,
    enum_ty: TypeId,
    variant_name: &str,
) -> Option<Vec<(Option<String>, TypeId)>> {
    if let TypeKind::Value(ValueTypeKind::Option(payload_ty)) = materialized.types.kind(enum_ty) {
        return match variant_name {
            "Some" => Some(vec![(None, *payload_ty)]),
            "None" => Some(Vec::new()),
            _ => None,
        };
    }
    let metadata =
        materialized_nominal_metadata_for_value_type(materialized, enum_ty, ast::TypeKind::Enum)?;
    metadata.members.iter().find_map(|member| match member {
        DeclMemberMetadata::EnumVariant(variant) if variant.name == variant_name => Some(
            variant
                .fields
                .iter()
                .map(|field| (Some(field.name.clone()), field.ty))
                .collect(),
        ),
        _ => None,
    })
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

fn validate_materialized_top_level_store_target(
    materialized: &MaterializedMir,
    fqn: &str,
    block: BasicBlockId,
    span: Span,
    target_fqn: &str,
    value_ty: TypeId,
) -> MaterializeResult<()> {
    let Some(target) = materialized_top_level_store_target(materialized, target_fqn) else {
        return Err(materialized_type_contract_err(
            fqn,
            Some(block),
            span,
            "top-level store target",
            "top-level store target is not published in materialized metadata",
        ));
    };
    if !target.mutable {
        return Err(materialized_type_contract_err(
            fqn,
            Some(block),
            span,
            "top-level store target",
            "top-level store target is immutable",
        ));
    }
    if target.ty != value_ty {
        return Err(materialized_type_contract_err(
            fqn,
            Some(block),
            span,
            "top-level store target",
            "top-level store value type does not match target type",
        ));
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct MaterializedTopLevelStoreTarget {
    ty: TypeId,
    mutable: bool,
}

fn materialized_top_level_store_target(
    materialized: &MaterializedMir,
    target_fqn: &str,
) -> Option<MaterializedTopLevelStoreTarget> {
    materialized.file.items.iter().find_map(|item| match item {
        Item::InitializerRoot(root) if root.fqn == target_fqn => {
            root.ty.map(|ty| MaterializedTopLevelStoreTarget {
                ty,
                mutable: matches!(root.kind, InitializerRootKind::RuntimeMutableVar { .. }),
            })
        }
        Item::ExternGlobal(root) if root.fqn == target_fqn => {
            Some(MaterializedTopLevelStoreTarget {
                ty: root.ty,
                mutable: root.mutable,
            })
        }
        _ => None,
    })
}

#[allow(clippy::too_many_arguments)]
fn validate_materialized_class_ctor_contract(
    materialized: &MaterializedMir,
    fqn: &str,
    block: BasicBlockId,
    span: Span,
    locals: &[LocalDecl],
    class_fqn: &str,
    ctor: &ClassCtorCallMetadata,
    args: &[CallArg],
    result_ty: Option<TypeId>,
) -> MaterializeResult<()> {
    let class = materialized_class_metadata_by_fqn(materialized, class_fqn);
    if class.is_some_and(|class| class.kind != ast::TypeKind::Class) {
        return Err(materialized_type_contract_err(
            fqn,
            Some(block),
            span,
            "class constructor target",
            "class constructor target must be class metadata",
        ));
    }
    if let Some(result_ty) = result_ty {
        let TypeKind::Ref(RefTypeKind::Nominal(nominal)) = materialized.types.kind(result_ty)
        else {
            return Err(materialized_type_contract_err(
                fqn,
                Some(block),
                span,
                "class constructor result",
                "class constructor result target must have class reference type",
            ));
        };
        if strip_materialized_type_args(&nominal.fqn) != strip_materialized_type_args(class_fqn) {
            return Err(materialized_type_contract_err(
                fqn,
                Some(block),
                span,
                "class constructor result",
                "class constructor result target and class metadata disagree",
            ));
        }
    }

    if args.iter().any(|arg| arg.name.is_some()) || args.len() != ctor.ordered_param_count {
        return Err(materialized_type_contract_err(
            fqn,
            Some(block),
            span,
            "class constructor arguments",
            "class constructor arguments must be ordered and complete",
        ));
    }

    let Some(class) = class else {
        for arg in args {
            materialized_operand_ty(
                materialized,
                fqn,
                block,
                arg.span,
                "class constructor argument",
                locals,
                &arg.value,
            )?;
        }
        return Ok(());
    };

    let selected_ctor = match ctor.selected_ctor_span {
        Some(selected_span) => {
            let mut matches = class
                .constructors
                .iter()
                .filter(|candidate| candidate.span == selected_span);
            let selected = matches.next().ok_or_else(|| {
                materialized_type_contract_err(
                    fqn,
                    Some(block),
                    span,
                    "class constructor target",
                    "selected class constructor is not declared",
                )
            })?;
            if matches.next().is_some() {
                return Err(materialized_type_contract_err(
                    fqn,
                    Some(block),
                    span,
                    "class constructor target",
                    "selected class constructor span is ambiguous",
                ));
            }
            Some(selected)
        }
        None if class.constructors.is_empty() => None,
        None => {
            return Err(materialized_type_contract_err(
                fqn,
                Some(block),
                span,
                "class constructor target",
                "class constructor target must publish a selected constructor",
            ));
        }
    };
    let params = selected_ctor.map_or([].as_slice(), |selected| selected.params.as_slice());
    if params.len() != args.len() {
        return Err(materialized_type_contract_err(
            fqn,
            Some(block),
            span,
            "class constructor arguments",
            "selected constructor parameter count does not match lowered arguments",
        ));
    }
    for (arg, param) in args.iter().zip(params) {
        let arg_ty = materialized_operand_ty(
            materialized,
            fqn,
            block,
            arg.span,
            "class constructor argument",
            locals,
            &arg.value,
        )?;
        if arg_ty.is_some_and(|ty| ty != param.ty) {
            return Err(materialized_type_contract_err(
                fqn,
                Some(block),
                arg.span,
                "class constructor argument",
                "class constructor argument type does not match selected parameter",
            ));
        }
    }
    Ok(())
}

fn materialized_class_metadata_by_fqn<'a>(
    materialized: &'a MaterializedMir,
    class_fqn: &str,
) -> Option<&'a NominalMetadata> {
    materialized_nominal_metadata_by_fqn(materialized, class_fqn).or_else(|| {
        materialized_nominal_metadata_by_fqn(materialized, strip_materialized_type_args(class_fqn))
    })
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
            let receiver_ty = materialized_operand_ty(
                materialized,
                fqn,
                block,
                span,
                "dispatch receiver",
                locals,
                receiver,
            )?;
            if receiver_ty.is_some_and(|ty| ty != dispatch.receiver_ty) {
                return Err(materialized_type_contract_err(
                    fqn,
                    Some(block),
                    span,
                    "dispatch receiver",
                    "dispatch receiver operand type and metadata disagree",
                ));
            }
            validate_materialized_type(
                materialized,
                MaterializedValidationContext {
                    fqn,
                    block: Some(block),
                    span,
                    surface: "dispatch receiver type",
                },
                dispatch.receiver_ty,
            )?;
            validate_materialized_dispatch_metadata(materialized, fqn, block, span, dispatch)
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

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_materialized_call_abi(
    materialized: &MaterializedMir,
    fqn: &str,
    block: BasicBlockId,
    span: Span,
    locals: &[LocalDecl],
    kind: &CallKind,
    args: &[CallArg],
    transport: &CallTransportMetadata,
    result_ty: Option<TypeId>,
) -> MaterializeResult<()> {
    match kind {
        CallKind::Direct { callee_fqn } => {
            let Some(callee) = materialized_callable_by_fqn(materialized, callee_fqn) else {
                if result_ty.is_some_and(|ty| {
                    !materialized_abi_type_equivalent(materialized, ty, transport.result.source_ty)
                }) {
                    return Err(materialized_transport_contract_err(
                        fqn,
                        block,
                        span,
                        "call result transport",
                        "call result transport type and assignment target disagree",
                    ));
                }
                return Ok(());
            };
            validate_materialized_direct_call_signature(
                materialized,
                fqn,
                block,
                span,
                locals,
                callee,
                args,
            )?;
            validate_materialized_call_return_contract(
                materialized,
                fqn,
                block,
                span,
                transport,
                result_ty,
                callee.return_ty,
            )
        }
        CallKind::Closure { callee, .. } | CallKind::FunValue { callee } => {
            let fun_ty = materialized_operand_function_type(
                materialized,
                fqn,
                block,
                span,
                locals,
                "callable-value callee",
                callee,
            )?;
            validate_materialized_callable_value_call_signature(
                materialized,
                fqn,
                block,
                span,
                locals,
                fun_ty,
                args,
            )?;
            validate_materialized_call_return_contract(
                materialized,
                fqn,
                block,
                span,
                transport,
                result_ty,
                fun_ty.return_ty,
            )
        }
        CallKind::FunPtr { callee } => {
            let fun_ty = materialized_operand_funptr_function_type(
                materialized,
                fqn,
                block,
                span,
                locals,
                callee,
            )?;
            validate_materialized_callable_value_call_signature(
                materialized,
                fqn,
                block,
                span,
                locals,
                fun_ty,
                args,
            )?;
            validate_materialized_call_return_contract(
                materialized,
                fqn,
                block,
                span,
                transport,
                result_ty,
                fun_ty.return_ty,
            )
        }
        CallKind::Resume { resume, .. } => validate_materialized_call_return_contract(
            materialized,
            fqn,
            block,
            span,
            transport,
            result_ty,
            resume.answer_ty,
        ),
        CallKind::Virtual { dispatch, .. } | CallKind::Interface { dispatch, .. } => {
            let Some(member_fun) =
                materialized_member_fun_by_fqn(materialized, &dispatch.member_fqn)
            else {
                if result_ty.is_some_and(|ty| {
                    !materialized_abi_type_equivalent(materialized, ty, transport.result.source_ty)
                }) {
                    return Err(materialized_transport_contract_err(
                        fqn,
                        block,
                        span,
                        "call result transport",
                        "dispatch result transport type and assignment target disagree",
                    ));
                }
                return Ok(());
            };
            if member_fun.params.is_empty() {
                return Err(materialized_type_contract_err(
                    fqn,
                    Some(block),
                    span,
                    "dispatch target",
                    "dispatch target member function must publish receiver parameter",
                ));
            }
            validate_materialized_callable_value_call_signature(
                materialized,
                fqn,
                block,
                span,
                locals,
                &crate::ty::FunctionType {
                    receiver: None,
                    params: member_fun.params[1..]
                        .iter()
                        .map(|param| param.ty)
                        .collect(),
                    return_ty: member_fun.return_ty,
                    effects: EffectRow::pure(),
                    effects_closed: true,
                },
                args,
            )?;
            validate_materialized_call_return_contract(
                materialized,
                fqn,
                block,
                span,
                transport,
                result_ty,
                member_fun.return_ty,
            )
        }
    }
}

fn materialized_callable_by_fqn<'a>(
    materialized: &'a MaterializedMir,
    callee_fqn: &str,
) -> Option<&'a FunDecl> {
    let pass_view = materialized.pass_view();
    pass_view.callable(callee_fqn).or_else(|| {
        materialized.file.items.iter().find_map(|item| match item {
            Item::Fun(fun) if fun.fqn == callee_fqn => Some(fun),
            _ => None,
        })
    })
}

#[allow(clippy::too_many_arguments)]
fn validate_materialized_direct_call_signature(
    materialized: &MaterializedMir,
    fqn: &str,
    block: BasicBlockId,
    span: Span,
    locals: &[LocalDecl],
    callee: &FunDecl,
    args: &[CallArg],
) -> MaterializeResult<()> {
    let arg_to_param =
        map_materialized_call_args_to_params(&callee.params, args).ok_or_else(|| {
            materialized_type_contract_err(
                fqn,
                Some(block),
                span,
                "call arguments",
                "call arguments do not bind exactly to callee parameters",
            )
        })?;
    for (arg_index, arg) in args.iter().enumerate() {
        let param = &callee.params[arg_to_param[arg_index]];
        validate_materialized_call_arg_type(materialized, fqn, block, locals, arg, param.ty)?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_materialized_callable_value_call_signature(
    materialized: &MaterializedMir,
    fqn: &str,
    block: BasicBlockId,
    span: Span,
    locals: &[LocalDecl],
    fun_ty: &crate::ty::FunctionType,
    args: &[CallArg],
) -> MaterializeResult<()> {
    if args.iter().any(|arg| arg.name.is_some()) {
        return Err(materialized_type_contract_err(
            fqn,
            Some(block),
            span,
            "call arguments",
            "callable-value arguments must be positional",
        ));
    }
    let param_tys = callable_value_param_tys(fun_ty);
    if args.len() != param_tys.len() {
        return Err(materialized_type_contract_err(
            fqn,
            Some(block),
            span,
            "call arguments",
            "call argument count does not match callable type",
        ));
    }
    for (arg, param_ty) in args.iter().zip(param_tys) {
        validate_materialized_call_arg_type(materialized, fqn, block, locals, arg, param_ty)?;
    }
    Ok(())
}

fn callable_value_param_tys(fun_ty: &crate::ty::FunctionType) -> Vec<TypeId> {
    fun_ty
        .receiver
        .into_iter()
        .chain(fun_ty.params.iter().copied())
        .collect()
}

fn map_materialized_call_args_to_params(params: &[Param], args: &[CallArg]) -> Option<Vec<usize>> {
    let mut used = vec![false; params.len()];
    let mut next_pos = 0usize;
    let mut out = Vec::with_capacity(args.len());

    for arg in args {
        let param_idx = match arg.name.as_deref() {
            Some(name) => params
                .iter()
                .enumerate()
                .find_map(|(idx, param)| (!used[idx] && param.name == name).then_some(idx))?,
            None => {
                while used.get(next_pos).copied().unwrap_or(false) {
                    next_pos += 1;
                }
                let idx = next_pos;
                if idx >= params.len() {
                    return None;
                }
                next_pos += 1;
                idx
            }
        };
        used[param_idx] = true;
        out.push(param_idx);
    }

    (out.len() == params.len()).then_some(out)
}

fn validate_materialized_call_arg_type(
    materialized: &MaterializedMir,
    fqn: &str,
    block: BasicBlockId,
    locals: &[LocalDecl],
    arg: &CallArg,
    expected_ty: TypeId,
) -> MaterializeResult<()> {
    materialized_operand_ty(
        materialized,
        fqn,
        block,
        arg.span,
        "call argument",
        locals,
        &arg.value,
    )?;
    validate_materialized_type(
        materialized,
        MaterializedValidationContext {
            fqn,
            block: Some(block),
            span: arg.span,
            surface: "call parameter type",
        },
        expected_ty,
    )
}

fn materialized_abi_type_equivalent(
    materialized: &MaterializedMir,
    lhs: TypeId,
    rhs: TypeId,
) -> bool {
    lhs == rhs
        || materialized.types.kind(lhs) == materialized.types.kind(rhs)
        || materialized.types.display(lhs).to_string()
            == materialized.types.display(rhs).to_string()
}

fn validate_materialized_call_return_contract(
    materialized: &MaterializedMir,
    fqn: &str,
    block: BasicBlockId,
    span: Span,
    transport: &CallTransportMetadata,
    result_ty: Option<TypeId>,
    expected_return_ty: TypeId,
) -> MaterializeResult<()> {
    let detail = if !materialized_abi_type_equivalent(
        materialized,
        transport.result.source_ty,
        expected_return_ty,
    ) {
        Some("call result transport type does not match callee return type")
    } else if result_ty.is_some_and(|ty| {
        !materialized_abi_type_equivalent(materialized, ty, transport.result.source_ty)
    }) {
        Some("call result transport type and assignment target disagree")
    } else if transport
        .aggregate_return
        .as_ref()
        .is_some_and(|aggregate| {
            !materialized_abi_type_equivalent(materialized, aggregate.source_ty, expected_return_ty)
        })
    {
        Some("call aggregate return transport type does not match callee return type")
    } else {
        None
    };
    if let Some(detail) = detail {
        return Err(materialized_transport_contract_err(
            fqn,
            block,
            span,
            "call result transport",
            detail,
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn materialized_operand_function_type<'a>(
    materialized: &'a MaterializedMir,
    fqn: &str,
    block: BasicBlockId,
    span: Span,
    locals: &[LocalDecl],
    surface: &'static str,
    operand: &Operand,
) -> MaterializeResult<&'a crate::ty::FunctionType> {
    let Some(ty) =
        materialized_operand_ty(materialized, fqn, block, span, surface, locals, operand)?
    else {
        return Err(materialized_type_contract_err(
            fqn,
            Some(block),
            span,
            surface,
            "callable operand type must be known",
        ));
    };
    let TypeKind::Ref(RefTypeKind::Function(fun_ty)) = materialized.types.kind(ty) else {
        return Err(materialized_type_contract_err(
            fqn,
            Some(block),
            span,
            surface,
            "callable operand must have function type",
        ));
    };
    Ok(fun_ty)
}

fn materialized_operand_funptr_function_type<'a>(
    materialized: &'a MaterializedMir,
    fqn: &str,
    block: BasicBlockId,
    span: Span,
    locals: &[LocalDecl],
    operand: &Operand,
) -> MaterializeResult<&'a crate::ty::FunctionType> {
    let Some(ty) = materialized_operand_ty(
        materialized,
        fqn,
        block,
        span,
        "FunPtr callee",
        locals,
        operand,
    )?
    else {
        return Err(materialized_type_contract_err(
            fqn,
            Some(block),
            span,
            "FunPtr callee",
            "FunPtr operand type must be known",
        ));
    };
    let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = materialized.types.kind(ty) else {
        return Err(materialized_type_contract_err(
            fqn,
            Some(block),
            span,
            "FunPtr callee",
            "FunPtr operand must have nominal FunPtr type",
        ));
    };
    if nominal.fqn != "scoop.unsafe.FunPtr" || nominal.args.len() != 1 {
        return Err(materialized_type_contract_err(
            fqn,
            Some(block),
            span,
            "FunPtr callee",
            "FunPtr operand must publish exactly one function type argument",
        ));
    }
    let TypeKind::Ref(RefTypeKind::Function(fun_ty)) = materialized.types.kind(nominal.args[0])
    else {
        return Err(materialized_type_contract_err(
            fqn,
            Some(block),
            span,
            "FunPtr callee",
            "FunPtr type argument must be a function type",
        ));
    };
    Ok(fun_ty)
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
    )?;
    validate_materialized_member_target(materialized, fqn, block, span, member)
}

fn validate_materialized_dispatch_metadata(
    _materialized: &MaterializedMir,
    fqn: &str,
    block: BasicBlockId,
    span: Span,
    dispatch: &DispatchMetadata,
) -> MaterializeResult<()> {
    if dispatch.owner_fqn.is_empty()
        || dispatch.member_name.is_empty()
        || dispatch.member_fqn.is_empty()
    {
        return Err(materialized_type_contract_err(
            fqn,
            Some(block),
            span,
            "dispatch target",
            "dispatch metadata must publish owner and member identity",
        ));
    }
    if !dispatch.member_fqn.starts_with(&dispatch.owner_fqn) {
        return Err(materialized_type_contract_err(
            fqn,
            Some(block),
            span,
            "dispatch target",
            "dispatch target member does not belong to published owner",
        ));
    }
    Ok(())
}

fn validate_materialized_member_target(
    materialized: &MaterializedMir,
    fqn: &str,
    block: BasicBlockId,
    span: Span,
    member: &MemberAccessMetadata,
) -> MaterializeResult<()> {
    let target_fqn = match &member.resolved {
        Some(MemberTarget::Value { fqn: target_fqn }) => target_fqn,
        Some(_) => {
            return Err(materialized_type_contract_err(
                fqn,
                Some(block),
                span,
                "member target",
                "member access target must be a value member",
            ));
        }
        None => {
            return Err(materialized_type_contract_err(
                fqn,
                Some(block),
                span,
                "member target",
                "member access target must be resolved before MIR codegen",
            ));
        }
    };
    match materialized_declares_value_member(materialized, target_fqn) {
        Some(true) => return Ok(()),
        None => {
            if materialized_value_member_owner_matches_receiver(
                materialized,
                member.receiver_ty,
                target_fqn,
            ) {
                return Ok(());
            }
            return Err(materialized_type_contract_err(
                fqn,
                Some(block),
                span,
                "member target",
                "resolved value member owner is not declared in MIR metadata",
            ));
        }
        Some(false) => {}
    }
    Err(materialized_type_contract_err(
        fqn,
        Some(block),
        span,
        "member target",
        "resolved value member target is not declared in MIR metadata",
    ))
}

fn materialized_value_member_owner_matches_receiver(
    materialized: &MaterializedMir,
    receiver_ty: TypeId,
    target_fqn: &str,
) -> bool {
    let Some((owner_fqn, _)) = target_fqn.rsplit_once('.') else {
        return false;
    };
    let owner_fqn = strip_materialized_type_args(owner_fqn);
    match materialized.types.kind(receiver_ty) {
        TypeKind::Ref(RefTypeKind::Nominal(nominal))
        | TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
            strip_materialized_type_args(&nominal.fqn) == owner_fqn
        }
        TypeKind::Ref(RefTypeKind::String) => owner_fqn == "scoop.core.String",
        TypeKind::Value(ValueTypeKind::Bool) => owner_fqn == "scoop.core.Bool",
        TypeKind::Value(ValueTypeKind::Char) => owner_fqn == "scoop.core.Char",
        TypeKind::Value(ValueTypeKind::Float64) => owner_fqn == "scoop.core.Float64",
        TypeKind::Value(ValueTypeKind::Float32) => owner_fqn == "scoop.core.Float32",
        TypeKind::Value(ValueTypeKind::Int) => owner_fqn == "scoop.core.Int",
        TypeKind::Value(ValueTypeKind::UInt) => owner_fqn == "scoop.core.UInt",
        TypeKind::Value(ValueTypeKind::IntN(bits)) => owner_fqn == format!("scoop.core.Int{bits}"),
        TypeKind::Value(ValueTypeKind::UIntN(bits)) => {
            owner_fqn == format!("scoop.core.UInt{bits}")
        }
        TypeKind::Value(ValueTypeKind::Unit) => owner_fqn == "scoop.core.Unit",
        TypeKind::Ref(RefTypeKind::Any)
        | TypeKind::Ref(RefTypeKind::Function(_) | RefTypeKind::Union(_))
        | TypeKind::Value(
            ValueTypeKind::Nothing | ValueTypeKind::Option(_) | ValueTypeKind::Tuple(_),
        )
        | TypeKind::Param(_)
        | TypeKind::StarProjection(_) => false,
    }
}

fn materialized_member_fun_by_fqn<'a>(
    materialized: &'a MaterializedMir,
    target_fqn: &str,
) -> Option<&'a MemberFunMetadata> {
    let (owner_fqn, member_name) = target_fqn.rsplit_once('.')?;
    let owner_fqn = strip_materialized_type_args(owner_fqn);
    let normalized_target = format!("{owner_fqn}.{member_name}");
    materialized.file.items.iter().find_map(|item| match item {
        Item::Metadata(MetadataRoot::Nominal(metadata)) if metadata.fqn == owner_fqn => {
            metadata_member_fun_by_fqn(&metadata.members, &normalized_target)
        }
        Item::Metadata(MetadataRoot::Object(metadata)) if metadata.fqn == owner_fqn => {
            metadata_member_fun_by_fqn(&metadata.members, &normalized_target)
        }
        _ => None,
    })
}

fn metadata_member_fun_by_fqn<'a>(
    members: &'a [DeclMemberMetadata],
    target_fqn: &str,
) -> Option<&'a MemberFunMetadata> {
    members.iter().find_map(|member| match member {
        DeclMemberMetadata::Fun(fun) if fun.fqn == target_fqn => Some(fun),
        DeclMemberMetadata::Nested(root) => match root.as_ref() {
            MetadataRoot::Nominal(metadata) => {
                metadata_member_fun_by_fqn(&metadata.members, target_fqn)
            }
            MetadataRoot::Object(metadata) => {
                metadata_member_fun_by_fqn(&metadata.members, target_fqn)
            }
            MetadataRoot::TypeAlias(_) | MetadataRoot::ExtensionProperty(_) => None,
        },
        _ => None,
    })
}

fn materialized_declares_value_member(
    materialized: &MaterializedMir,
    target_fqn: &str,
) -> Option<bool> {
    let (owner_fqn, member_name) = target_fqn.rsplit_once('.')?;
    let owner_fqn = strip_materialized_type_args(owner_fqn);
    let normalized_target = format!("{owner_fqn}.{member_name}");
    materialized.file.items.iter().find_map(|item| match item {
        Item::Metadata(MetadataRoot::Nominal(metadata)) if metadata.fqn == owner_fqn => Some(
            metadata_declares_value_member(&metadata.members, &normalized_target),
        ),
        Item::Metadata(MetadataRoot::Object(metadata)) if metadata.fqn == owner_fqn => Some(
            metadata_declares_value_member(&metadata.members, &normalized_target),
        ),
        _ => None,
    })
}

fn strip_materialized_type_args(fqn: &str) -> &str {
    fqn.split_once("::<")
        .or_else(|| fqn.split_once('<'))
        .map_or(fqn, |(base, _)| base)
}

fn metadata_declares_value_member(members: &[DeclMemberMetadata], target_fqn: &str) -> bool {
    members.iter().any(|member| match member {
        DeclMemberMetadata::Field(field) => field.fqn == target_fqn,
        DeclMemberMetadata::Property(prop) => prop.fqn == target_fqn,
        DeclMemberMetadata::Nested(root) => match root.as_ref() {
            MetadataRoot::Nominal(metadata) => {
                metadata_declares_value_member(&metadata.members, target_fqn)
            }
            MetadataRoot::Object(metadata) => {
                metadata_declares_value_member(&metadata.members, target_fqn)
            }
            MetadataRoot::TypeAlias(_) | MetadataRoot::ExtensionProperty(_) => false,
        },
        DeclMemberMetadata::Fun(_)
        | DeclMemberMetadata::EnumVariant(_)
        | DeclMemberMetadata::InitBlock { .. } => false,
    })
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

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_materialized_type_test_contract(
    materialized: &MaterializedMir,
    fqn: &str,
    block: BasicBlockId,
    span: Span,
    expected_source_ty: Option<TypeId>,
    expected_target_ty: TypeId,
    expected_result_ty: Option<TypeId>,
    metadata: &RuntimeTypeTestMetadata,
) -> MaterializeResult<()> {
    validate_materialized_runtime_type_test_contract(
        materialized,
        fqn,
        block,
        span,
        "typecheck",
        expected_source_ty,
        expected_target_ty,
        metadata,
    )?;
    if let Some(result_ty) = expected_result_ty {
        let builtins = materialized
            .types
            .builtins()
            .expect("materialized MIR should always intern builtin types before validation");
        if result_ty != builtins.bool_ {
            return Err(materialized_type_contract_err(
                fqn,
                Some(block),
                span,
                "typecheck result",
                "typecheck result target must have Bool type",
            ));
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_materialized_cast_contract(
    materialized: &MaterializedMir,
    fqn: &str,
    block: BasicBlockId,
    span: Span,
    op: ast::CastOp,
    expected_source_ty: Option<TypeId>,
    expected_target_ty: TypeId,
    expected_result_ty: Option<TypeId>,
    metadata: &RuntimeCastMetadata,
) -> MaterializeResult<()> {
    validate_materialized_runtime_type_test_contract(
        materialized,
        fqn,
        block,
        span,
        "cast",
        expected_source_ty,
        expected_target_ty,
        &metadata.test,
    )?;
    if !materialized_runtime_ref_codegen_supported(materialized, expected_target_ty) {
        return Err(materialized_runtime_contract_err(
            fqn,
            block,
            span,
            "cast",
            "cast target must have runtime-ref codegen support",
        ));
    }

    match (op, &metadata.failure, &metadata.result) {
        (
            ast::CastOp::As,
            RuntimeCastFailure::Raise { error_fqn, .. },
            RuntimeCastResult::Target { ty },
        ) => {
            if error_fqn != "scoop.core.RuntimeError.ClassCastFailed" {
                return Err(materialized_runtime_contract_err(
                    fqn,
                    block,
                    span,
                    "cast",
                    "`as` cast must raise ClassCastFailed on failure",
                ));
            }
            if *ty != expected_target_ty {
                return Err(materialized_runtime_contract_err(
                    fqn,
                    block,
                    span,
                    "cast",
                    "`as` cast result type must match target type",
                ));
            }
            if expected_result_ty.is_some_and(|result_ty| result_ty != expected_target_ty) {
                return Err(materialized_runtime_contract_err(
                    fqn,
                    block,
                    span,
                    "cast",
                    "`as` cast assignment target must match target type",
                ));
            }
            Ok(())
        }
        (
            ast::CastOp::AsQ,
            RuntimeCastFailure::ReturnNone,
            RuntimeCastResult::Option { option_ty, some_ty },
        ) => {
            if *some_ty != expected_target_ty {
                return Err(materialized_runtime_contract_err(
                    fqn,
                    block,
                    span,
                    "cast",
                    "`as?` some type must match target type",
                ));
            }
            let TypeKind::Value(ValueTypeKind::Option(payload_ty)) =
                materialized.types.kind(*option_ty)
            else {
                return Err(materialized_runtime_contract_err(
                    fqn,
                    block,
                    span,
                    "cast",
                    "`as?` result type must be Option<T>",
                ));
            };
            if *payload_ty != *some_ty {
                return Err(materialized_runtime_contract_err(
                    fqn,
                    block,
                    span,
                    "cast",
                    "`as?` Option payload type must match some type",
                ));
            }
            if expected_result_ty.is_some_and(|result_ty| result_ty != *option_ty) {
                return Err(materialized_runtime_contract_err(
                    fqn,
                    block,
                    span,
                    "cast",
                    "`as?` assignment target must match Option result type",
                ));
            }
            Ok(())
        }
        _ => Err(materialized_runtime_contract_err(
            fqn,
            block,
            span,
            "cast",
            "failure/result contract does not match cast operator",
        )),
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_materialized_runtime_type_test_contract(
    materialized: &MaterializedMir,
    fqn: &str,
    block: BasicBlockId,
    span: Span,
    primitive: &'static str,
    expected_source_ty: Option<TypeId>,
    expected_target_ty: TypeId,
    metadata: &RuntimeTypeTestMetadata,
) -> MaterializeResult<()> {
    if expected_source_ty.is_some_and(|source_ty| metadata.source_ty != source_ty) {
        return Err(materialized_runtime_contract_err(
            fqn,
            block,
            span,
            primitive,
            "source type and operand type disagree",
        ));
    }
    if metadata.target_ty != expected_target_ty || metadata.descriptor.ty != expected_target_ty {
        return Err(materialized_runtime_contract_err(
            fqn,
            block,
            span,
            primitive,
            "target type and runtime descriptor disagree",
        ));
    }
    if !materialized_runtime_descriptor_shape_matches(materialized, &metadata.descriptor) {
        return Err(materialized_runtime_contract_err(
            fqn,
            block,
            span,
            primitive,
            "runtime descriptor kind does not match target type",
        ));
    }
    if metadata.static_fold == RuntimeTypeStaticFold::Dynamic
        && !materialized_runtime_ref_codegen_supported(materialized, metadata.target_ty)
    {
        return Err(materialized_runtime_contract_err(
            fqn,
            block,
            span,
            primitive,
            "dynamic runtime type-test target is not supported by codegen",
        ));
    }
    Ok(())
}

fn materialized_runtime_descriptor_shape_matches(
    materialized: &MaterializedMir,
    descriptor: &RuntimeTypeDescriptorKey,
) -> bool {
    match (&descriptor.kind, materialized.types.kind(descriptor.ty)) {
        (RuntimeTypeDescriptorKind::Any, TypeKind::Ref(RefTypeKind::Any)) => true,
        (RuntimeTypeDescriptorKind::String, TypeKind::Ref(RefTypeKind::String)) => true,
        (
            RuntimeTypeDescriptorKind::Nominal { fqn, kind },
            TypeKind::Ref(RefTypeKind::Nominal(nominal))
            | TypeKind::Value(ValueTypeKind::Nominal(nominal)),
        ) => {
            nominal.fqn == *fqn
                && kind.is_none_or(|expected| {
                    nominal_runtime_kind(materialized, &nominal.fqn) == Some(expected)
                })
        }
        (RuntimeTypeDescriptorKind::Function, TypeKind::Ref(RefTypeKind::Function(_))) => true,
        (RuntimeTypeDescriptorKind::Option, TypeKind::Value(ValueTypeKind::Option(_))) => true,
        (RuntimeTypeDescriptorKind::Tuple, TypeKind::Value(ValueTypeKind::Tuple(_))) => true,
        (RuntimeTypeDescriptorKind::Value, TypeKind::Value(_)) => true,
        (RuntimeTypeDescriptorKind::TypeParam, TypeKind::Param(_)) => true,
        (RuntimeTypeDescriptorKind::StarProjection, TypeKind::StarProjection(_)) => true,
        (RuntimeTypeDescriptorKind::Union, TypeKind::Ref(RefTypeKind::Union(_))) => true,
        _ => false,
    }
}

fn nominal_runtime_kind(materialized: &MaterializedMir, fqn: &str) -> Option<ast::TypeKind> {
    materialized_nominal_metadata_by_fqn(materialized, fqn).map(|metadata| metadata.kind)
}

fn materialized_runtime_ref_codegen_supported(materialized: &MaterializedMir, ty: TypeId) -> bool {
    matches!(
        materialized.types.kind(ty),
        TypeKind::Ref(RefTypeKind::Any | RefTypeKind::String | RefTypeKind::Nominal(_))
    )
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
    subject_ty: Option<TypeId>,
    pattern: &Pattern,
) -> MaterializeResult<()> {
    match pattern {
        Pattern::Is { ty, metadata } => {
            if subject_ty.is_some_and(|ty| metadata.subject_ty != ty) {
                return Err(materialized_runtime_contract_err(
                    fqn,
                    block,
                    span,
                    "pattern type test",
                    "subject type and operand type disagree",
                ));
            }
            if metadata.target_ty != *ty || metadata.descriptor.ty != *ty {
                return Err(materialized_runtime_contract_err(
                    fqn,
                    block,
                    span,
                    "pattern type test",
                    "target type and runtime descriptor disagree",
                ));
            }
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
                validate_materialized_pattern(materialized, fqn, block, span, subject_ty, pat)?;
            }
            Ok(())
        }
        Pattern::Tuple { elements } => {
            let Some(subject_ty) = subject_ty else {
                return Err(materialized_type_contract_err(
                    fqn,
                    Some(block),
                    span,
                    "pattern tuple subject",
                    "tuple pattern subject type must be known",
                ));
            };
            let TypeKind::Value(ValueTypeKind::Tuple(element_tys)) =
                materialized.types.kind(subject_ty)
            else {
                return Err(materialized_type_contract_err(
                    fqn,
                    Some(block),
                    span,
                    "pattern tuple subject",
                    "tuple pattern subject must have tuple type",
                ));
            };
            let (prefix, has_rest) = pattern_prefix_and_rest(elements);
            if (!has_rest && prefix.len() != element_tys.len())
                || (has_rest && prefix.len() > element_tys.len())
            {
                return Err(materialized_type_contract_err(
                    fqn,
                    Some(block),
                    span,
                    "pattern tuple arity",
                    "tuple pattern arity does not match subject type",
                ));
            }
            for (pat, element_ty) in prefix.iter().zip(element_tys.iter()) {
                validate_materialized_pattern(
                    materialized,
                    fqn,
                    block,
                    span,
                    Some(*element_ty),
                    pat,
                )?;
            }
            Ok(())
        }
        Pattern::Variant { name, args } => {
            let Some(subject_ty) = subject_ty else {
                return Err(materialized_type_contract_err(
                    fqn,
                    Some(block),
                    span,
                    "pattern variant subject",
                    "variant pattern subject type must be known",
                ));
            };
            let Some(variant_fields) =
                materialized_enum_variant_fields(materialized, subject_ty, name)
            else {
                return Err(materialized_type_contract_err(
                    fqn,
                    Some(block),
                    span,
                    "pattern variant",
                    "variant pattern name is not declared on subject enum",
                ));
            };
            let (prefix, has_rest) = pattern_prefix_and_rest(args);
            if (!has_rest && prefix.len() != variant_fields.len())
                || (has_rest && prefix.len() > variant_fields.len())
            {
                return Err(materialized_type_contract_err(
                    fqn,
                    Some(block),
                    span,
                    "pattern variant arity",
                    "variant pattern arity does not match subject enum",
                ));
            }
            for (pat, (_, field_ty)) in prefix.iter().zip(variant_fields.iter()) {
                validate_materialized_pattern(
                    materialized,
                    fqn,
                    block,
                    span,
                    Some(*field_ty),
                    pat,
                )?;
            }
            Ok(())
        }
        Pattern::IntLit { .. } => validate_materialized_pattern_scalar_subject(
            materialized,
            fqn,
            block,
            span,
            subject_ty,
            "pattern int subject",
            materialized_pattern_subject_is_int,
        ),
        Pattern::CharLit { .. } => validate_materialized_pattern_scalar_subject(
            materialized,
            fqn,
            block,
            span,
            subject_ty,
            "pattern char subject",
            materialized_pattern_subject_is_char,
        ),
        Pattern::StringLit { .. } => validate_materialized_pattern_scalar_subject(
            materialized,
            fqn,
            block,
            span,
            subject_ty,
            "pattern string subject",
            materialized_pattern_subject_is_string,
        ),
        Pattern::BoolLit { .. } => validate_materialized_pattern_scalar_subject(
            materialized,
            fqn,
            block,
            span,
            subject_ty,
            "pattern bool subject",
            materialized_pattern_subject_is_bool,
        ),
        Pattern::Else | Pattern::Wildcard => Ok(()),
        Pattern::Rest => Err(materialized_type_contract_err(
            fqn,
            Some(block),
            span,
            "pattern rest position",
            "rest pattern is only valid in tuple or variant tail position",
        )),
    }
}

fn pattern_prefix_and_rest(patterns: &[Pattern]) -> (&[Pattern], bool) {
    match patterns.last() {
        Some(Pattern::Rest) => (&patterns[..patterns.len().saturating_sub(1)], true),
        _ => (patterns, false),
    }
}

fn validate_materialized_pattern_scalar_subject(
    materialized: &MaterializedMir,
    fqn: &str,
    block: BasicBlockId,
    span: Span,
    subject_ty: Option<TypeId>,
    surface: &'static str,
    predicate: fn(&MaterializedMir, TypeId) -> bool,
) -> MaterializeResult<()> {
    let Some(subject_ty) = subject_ty else {
        return Err(materialized_type_contract_err(
            fqn,
            Some(block),
            span,
            surface,
            "literal pattern subject type must be known",
        ));
    };
    if !predicate(materialized, subject_ty) {
        return Err(materialized_type_contract_err(
            fqn,
            Some(block),
            span,
            surface,
            "literal pattern subject type is incompatible with pattern literal",
        ));
    }
    Ok(())
}

fn materialized_pattern_subject_is_int(materialized: &MaterializedMir, ty: TypeId) -> bool {
    matches!(
        materialized.types.kind(ty),
        TypeKind::Value(
            ValueTypeKind::Int
                | ValueTypeKind::UInt
                | ValueTypeKind::IntN(_)
                | ValueTypeKind::UIntN(_)
        )
    ) || materialized_nominal_value_fqn(materialized, ty).is_some_and(|fqn| {
        fqn == "scoop.core.Int"
            || fqn == "scoop.core.UInt"
            || fqn == "scoop.core.UIntPtr"
            || fqn
                .strip_prefix("scoop.core.Int")
                .is_some_and(|suffix| !suffix.is_empty() && suffix.parse::<u16>().is_ok())
            || fqn
                .strip_prefix("scoop.core.UInt")
                .is_some_and(|suffix| !suffix.is_empty() && suffix.parse::<u16>().is_ok())
    })
}

fn materialized_pattern_subject_is_char(materialized: &MaterializedMir, ty: TypeId) -> bool {
    matches!(
        materialized.types.kind(ty),
        TypeKind::Value(ValueTypeKind::Char)
    ) || materialized_nominal_value_fqn(materialized, ty) == Some("scoop.core.Char")
}

fn materialized_pattern_subject_is_bool(materialized: &MaterializedMir, ty: TypeId) -> bool {
    matches!(
        materialized.types.kind(ty),
        TypeKind::Value(ValueTypeKind::Bool)
    ) || materialized_nominal_value_fqn(materialized, ty) == Some("scoop.core.Bool")
}

fn materialized_pattern_subject_is_string(materialized: &MaterializedMir, ty: TypeId) -> bool {
    matches!(
        materialized.types.kind(ty),
        TypeKind::Ref(RefTypeKind::String)
    )
}

fn materialized_nominal_value_fqn(materialized: &MaterializedMir, ty: TypeId) -> Option<&str> {
    match materialized.types.kind(ty) {
        TypeKind::Value(ValueTypeKind::Nominal(nominal)) => Some(nominal.fqn.as_str()),
        _ => None,
    }
}

pub(super) fn validate_materialized_pattern_extract_schema(
    materialized: &MaterializedMir,
    fqn: &str,
    block: BasicBlockId,
    span: Span,
    subject_ty: Option<TypeId>,
    path: &[super::super::PatternBindingStep],
    expected_result_ty: Option<TypeId>,
) -> MaterializeResult<()> {
    let Some(mut current_ty) = subject_ty else {
        return Err(materialized_type_contract_err(
            fqn,
            Some(block),
            span,
            "pattern extract subject",
            "pattern extract subject type must be known",
        ));
    };
    for step in path {
        current_ty = match step {
            super::super::PatternBindingStep::TupleIndex(index) => {
                let TypeKind::Value(ValueTypeKind::Tuple(elements)) =
                    materialized.types.kind(current_ty)
                else {
                    return Err(materialized_type_contract_err(
                        fqn,
                        Some(block),
                        span,
                        "pattern extract tuple subject",
                        "tuple extraction step requires a tuple subject type",
                    ));
                };
                *elements.get(*index).ok_or_else(|| {
                    materialized_type_contract_err(
                        fqn,
                        Some(block),
                        span,
                        "pattern extract tuple index",
                        "tuple extraction index is outside the tuple type",
                    )
                })?
            }
            super::super::PatternBindingStep::VariantField {
                variant,
                field_index,
            } => {
                let Some(fields) =
                    materialized_enum_variant_fields(materialized, current_ty, variant)
                else {
                    return Err(materialized_type_contract_err(
                        fqn,
                        Some(block),
                        span,
                        "pattern extract variant",
                        "variant extraction step is not declared on subject enum",
                    ));
                };
                fields.get(*field_index).map(|(_, ty)| *ty).ok_or_else(|| {
                    materialized_type_contract_err(
                        fqn,
                        Some(block),
                        span,
                        "pattern extract variant field",
                        "variant extraction field index is outside the variant payload",
                    )
                })?
            }
        };
        validate_materialized_type(
            materialized,
            MaterializedValidationContext {
                fqn,
                block: Some(block),
                span,
                surface: "pattern extract field type",
            },
            current_ty,
        )?;
    }
    if expected_result_ty.is_some_and(|ty| ty != current_ty) {
        return Err(materialized_type_contract_err(
            fqn,
            Some(block),
            span,
            "pattern extract result",
            "pattern extract result type does not match assignment target",
        ));
    }
    Ok(())
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
