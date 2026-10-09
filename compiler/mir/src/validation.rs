use super::*;

mod constants;
pub use constants::MirConstantImageError;
use constants::validate_constant_images;
mod callbacks;
use callbacks::{validate_foreign_callback_metadata, validate_static_callback_metadata};
mod function_adapters;
use function_adapters::validate_function_adapter_metadata;
mod function_bridges;
use function_bridges::validate_function_bridge_metadata;
mod closure_environments;
use closure_environments::validate_closure_environment_metadata;
mod boxed_values;
use boxed_values::validate_boxed_value_metadata;
mod interface_adjusts;
use interface_adjusts::validate_interface_adjust_metadata;
mod c_abi;
mod metadata;
use c_abi::validate_c_abi_projections;
use metadata::{
    validate_enum_metadata, validate_local_value_metadata,
    validate_source_callable_materializations,
};
mod generated_exact_types;
use generated_exact_types::validate_generated_exact_type_metadata;
mod runtime_types;
pub use runtime_types::MirRuntimeTypeLocation;
use runtime_types::validate_runtime_type_metadata;
mod generated_callables;
use generated_callables::validate_generated_callable_metadata;
mod immortal_objects;
use immortal_objects::validate_immortal_objects;
mod imported_calls;
use imported_calls::validate_imported_call;

mod callable_functions;
use callable_functions::validate_callable_functions;
mod callable_signatures;
use callable_signatures::validate_callable_signature_metadata;
mod coroutines;
use coroutines::validate_coroutine_metadata;

mod errors;
pub use errors::*;
mod release;

impl Module {
    pub fn validate(&self) -> Result<(), MirValidationError> {
        validate_module(self)
    }
}

/// Validate body-local control-flow metadata, representation-independent enum
/// primitives, and compiler-only raw aggregate construction in every body.
///
/// A matching dominance proof is deliberately tied to an immutable local id,
/// not to printed or structural expression equality. Producers must materialize
/// a tested enum value into such a local before testing and projecting it.
pub fn validate_module(module: &Module) -> Result<(), MirValidationError> {
    release::validate_policies(module)?;
    validate_c_abi_projections(module)?;
    validate_source_callable_materializations(module)?;
    validate_local_value_metadata(module)?;
    validate_enum_metadata(module)?;
    validate_coroutine_metadata(module)?;
    validate_foreign_callback_metadata(module)?;
    validate_static_callback_metadata(module)?;
    validate_closure_environment_metadata(module)?;
    validate_function_adapter_metadata(module)?;
    validate_function_bridge_metadata(module)?;
    validate_boxed_value_metadata(module)?;
    validate_interface_adjust_metadata(module)?;
    validate_generated_exact_type_metadata(module)?;
    validate_runtime_type_metadata(module)?;
    validate_generated_callable_metadata(module)?;
    validate_immortal_objects(module)?;
    validate_callable_functions(module)?;
    validate_callable_signature_metadata(module)?;
    validate_constant_images(module)?;
    for (function_id, function) in module.functions.iter() {
        validate_body(
            module,
            LocalValueOwner::Function(function_id),
            &function.body,
        )?;
    }
    for (hook, definition) in module.release_hooks.iter() {
        release::validate_signature(module, hook, definition)?;
        validate_body(
            module,
            LocalValueOwner::ReleaseHook(hook),
            &definition.code.body,
        )?;
    }
    Ok(())
}

fn validate_body(
    module: &Module,
    owner: LocalValueOwner,
    body: &Body,
) -> Result<(), MirValidationError> {
    let mut loop_header_polls = vec![false; body.blocks.len()];
    for target in &body.loop_header_polls {
        let block = target.header();
        let index = block.into_raw().into_u32() as usize;
        let Some(seen) = loop_header_polls.get_mut(index) else {
            return Err(MirValidationError {
                location: release::block_location(owner, block),
                kind: MirValidationErrorKind::InvalidLoopHeaderPollTarget,
            });
        };
        if std::mem::replace(seen, true) {
            return Err(MirValidationError {
                location: release::block_location(owner, block),
                kind: MirValidationErrorKind::DuplicateLoopHeaderPollTarget,
            });
        }
    }

    for (block, definition) in body.blocks.iter() {
        for (index, statement) in definition.statements.iter().enumerate() {
            release::validate_statement(module, owner, definition, index).map_err(|kind| {
                MirValidationError {
                    location: release::block_location(owner, block),
                    kind,
                }
            })?;
            let call = match &statement.kind {
                StatementKind::Call(CallEffect::Unit(call))
                | StatementKind::Call(CallEffect::Value { call, .. }) => Some(call),
                _ => None,
            };
            if call.is_some_and(|call| !call.pending.is_root()) {
                return Err(MirValidationError {
                    location: release::block_location(owner, block),
                    kind: MirValidationErrorKind::NonRootCoroutinePendingContext,
                });
            }
            if let Some(call) = call
                && let Err(kind) = validate_imported_call(module, call)
            {
                return Err(MirValidationError {
                    location: release::block_location(owner, block),
                    kind,
                });
            }
        }
        try_visit_block_exprs(definition, &mut |expr| {
            validate_expression_shape(module, expr)
                .and_then(|()| release::validate_expression(module, owner, expr))
                .map_err(|kind| MirValidationError {
                    location: release::block_location(owner, block),
                    kind,
                })
        })?;
        release::validate_control_flow(owner, definition).map_err(|kind| MirValidationError {
            location: release::block_location(owner, block),
            kind,
        })?;
    }

    let stable_locals = stable_locals(body);
    let proofs = matching_test_edges(module, body, &stable_locals);
    for (block, definition) in body.blocks.iter() {
        try_visit_block_exprs(definition, &mut |expr| {
            let ExprKind::VariantPayloadProject { operand, field } = &expr.kind else {
                return Ok(());
            };
            let Some(value) = stable_variant_value(body, &stable_locals, operand) else {
                return Err(MirValidationError {
                    location: release::block_location(owner, block),
                    kind: MirValidationErrorKind::VariantPayloadNotDominated { field: *field },
                });
            };
            let dominated = proofs.iter().any(|proof| {
                proof.value == value
                    && proof.variant == field.variant()
                    && edge_dominates_block(body, proof.edge, block)
            });
            if dominated {
                Ok(())
            } else {
                Err(MirValidationError {
                    location: release::block_location(owner, block),
                    kind: MirValidationErrorKind::VariantPayloadNotDominated { field: *field },
                })
            }
        })?;
    }
    Ok(())
}

fn validate_expression_shape(module: &Module, expr: &Expr) -> Result<(), MirValidationErrorKind> {
    match &expr.kind {
        ExprKind::StructConstruct { struct_id, fields } => {
            let index = struct_id.into_raw().into_u32() as usize;
            if index >= module.structs.len() {
                return Err(MirValidationErrorKind::InvalidStructReference {
                    struct_id: *struct_id,
                });
            }
            if expr.ty != Type::Struct(*struct_id) {
                return Err(MirValidationErrorKind::RawStructConstructResultType {
                    struct_id: *struct_id,
                    actual: expr.ty.clone(),
                });
            }
            let StructRepresentation::Declared {
                fields: expected_fields,
                ..
            } = &module.structs[*struct_id].representation
            else {
                return Err(MirValidationErrorKind::RawStructConstructRequiresDeclared {
                    struct_id: *struct_id,
                });
            };
            if fields.len() != expected_fields.len() {
                return Err(MirValidationErrorKind::RawStructConstructArity {
                    struct_id: *struct_id,
                    expected: expected_fields.len(),
                    actual: fields.len(),
                });
            }
            for (index, (field, expected)) in fields.iter().zip(expected_fields.iter()).enumerate()
            {
                if field.ty != expected.ty {
                    return Err(MirValidationErrorKind::RawStructConstructFieldType {
                        struct_id: *struct_id,
                        field: index as u32,
                        expected: expected.ty.clone(),
                        actual: field.ty.clone(),
                    });
                }
            }
        }
        ExprKind::ClosureAlloc { class, captures } => {
            if class.into_raw().into_u32() as usize >= module.closure_classes.len() {
                return Err(MirValidationErrorKind::InvalidClosureExpression {
                    reason: "allocation references an invalid closure class",
                });
            }
            let definition = &module.closure_classes[*class];
            if expr.ty != Type::Function(definition.function_type) {
                return Err(MirValidationErrorKind::InvalidClosureExpression {
                    reason: "allocation result does not match the closure function type",
                });
            }
            if captures.len() != definition.captures.len() {
                return Err(MirValidationErrorKind::InvalidClosureExpression {
                    reason: "allocation does not initialize every physical capture field",
                });
            }
            let mut initialized = vec![false; definition.captures.len()];
            for capture in captures {
                let field = capture.field() as usize;
                let Some(expected) = definition.captures.get(field) else {
                    return Err(MirValidationErrorKind::InvalidClosureExpression {
                        reason: "allocation references an invalid physical capture field",
                    });
                };
                if std::mem::replace(&mut initialized[field], true) {
                    return Err(MirValidationErrorKind::InvalidClosureExpression {
                        reason: "allocation initializes a physical capture field more than once",
                    });
                }
                if capture.value().ty != expected.ty {
                    return Err(MirValidationErrorKind::InvalidClosureExpression {
                        reason: "allocation value does not match its physical capture field",
                    });
                }
            }
        }
        ExprKind::ClosureCapture {
            closure,
            class,
            index,
        } => {
            if class.into_raw().into_u32() as usize >= module.closure_classes.len() {
                return Err(MirValidationErrorKind::InvalidClosureExpression {
                    reason: "capture read references an invalid closure class",
                });
            }
            let definition = &module.closure_classes[*class];
            if closure.ty != Type::Function(definition.function_type) {
                return Err(MirValidationErrorKind::InvalidClosureExpression {
                    reason: "capture read operand does not match the closure function type",
                });
            }
            let Some(field) = definition.captures.get(*index as usize) else {
                return Err(MirValidationErrorKind::InvalidClosureExpression {
                    reason: "capture read references an invalid physical field",
                });
            };
            if expr.ty != field.ty {
                return Err(MirValidationErrorKind::InvalidClosureExpression {
                    reason: "capture read result does not match its physical field",
                });
            }
        }
        ExprKind::VariantConstruct { variant, fields } => {
            let definition = variant.definition(&module.enums).map_err(|error| {
                MirValidationErrorKind::InvalidVariantReference {
                    operation: MirVariantOperation::Construct,
                    error,
                }
            })?;
            if expr.ty
                != variant
                    .enum_type(&module.enums)
                    .expect("the variant was validated")
            {
                return Err(MirValidationErrorKind::VariantConstructResultType {
                    variant: *variant,
                    actual: expr.ty.clone(),
                });
            }
            if fields.len() != definition.fields.len() {
                return Err(MirValidationErrorKind::VariantConstructArity {
                    variant: *variant,
                    expected: definition.fields.len(),
                    actual: fields.len(),
                });
            }
            for (index, (field, expected)) in fields.iter().zip(&definition.fields).enumerate() {
                if field.ty != expected.ty {
                    let field_ref = MirVariantFieldRef::new(&module.enums, *variant, index as u32)
                        .expect("an enumerated variant field is checked by its definition");
                    return Err(MirValidationErrorKind::VariantConstructFieldType {
                        field: field_ref,
                        expected: expected.ty.clone(),
                        actual: field.ty.clone(),
                    });
                }
            }
        }
        ExprKind::VariantTest { operand, variant } => {
            variant.definition(&module.enums).map_err(|error| {
                MirValidationErrorKind::InvalidVariantReference {
                    operation: MirVariantOperation::Test,
                    error,
                }
            })?;
            validate_operand_type(module, operand, *variant, MirVariantOperation::Test)?;
            if expr.ty != Type::Boolean {
                return Err(MirValidationErrorKind::VariantTestResultType {
                    actual: expr.ty.clone(),
                });
            }
        }
        ExprKind::VariantPayloadProject { operand, field } => {
            let definition = field
                .definition(&module.enums)
                .map_err(|error| MirValidationErrorKind::InvalidVariantFieldReference { error })?;
            validate_operand_type(
                module,
                operand,
                field.variant(),
                MirVariantOperation::PayloadProject,
            )?;
            if expr.ty != definition.ty {
                return Err(MirValidationErrorKind::VariantPayloadResultType {
                    field: *field,
                    expected: definition.ty.clone(),
                    actual: expr.ty.clone(),
                });
            }
        }
        ExprKind::FunctionAddress { callback } => {
            if callback.into_raw().into_u32() as usize >= module.callback_bridges.len() {
                return Err(MirValidationErrorKind::InvalidCallbackBridge {
                    reason: "function address references an invalid callback bridge",
                });
            }
            let callback = &module.callback_bridges[*callback];
            if expr.ty != Type::FunPtr(callback.signature) {
                return Err(MirValidationErrorKind::InvalidCallbackBridge {
                    reason: "function address result does not match the callback signature",
                });
            }
        }
        ExprKind::ForeignCallbackRegister { bridge, closure } => {
            if bridge.into_raw().into_u32() as usize >= module.foreign_callback_bridges.len() {
                return Err(MirValidationErrorKind::InvalidForeignCallbackExpression {
                    reason: "registration references an invalid bridge",
                });
            }
            let bridge = &module.foreign_callback_bridges[*bridge];
            let family = module.foreign_callback_families[bridge.family];
            let adapter = &module.foreign_callback_adapters[bridge.adapter];
            if closure.ty != Type::Function(adapter.managed_signature) {
                return Err(MirValidationErrorKind::InvalidForeignCallbackExpression {
                    reason: "registration closure does not match the adapter signature",
                });
            }
            if expr.ty != Type::Struct(family.callback) {
                return Err(MirValidationErrorKind::InvalidForeignCallbackExpression {
                    reason: "registration result does not match the family callback struct",
                });
            }
        }
        ExprKind::ForeignCallbackOperation {
            operation,
            callback,
        } => {
            let family_id = operation.family();
            if family_id.into_raw().into_u32() as usize >= module.foreign_callback_families.len() {
                return Err(MirValidationErrorKind::InvalidForeignCallbackExpression {
                    reason: "operation references an invalid callback family",
                });
            }
            let family = module.foreign_callback_families[family_id];
            if callback.ty != Type::Struct(family.callback) {
                return Err(MirValidationErrorKind::InvalidForeignCallbackExpression {
                    reason: "operation operand does not match the family callback struct",
                });
            }
            let expected = match operation {
                ForeignCallbackOperation::Retain(_) => Type::Struct(family.callback),
                ForeignCallbackOperation::Release(_) => Type::Unit,
                ForeignCallbackOperation::State(_) => family
                    .states
                    .registered()
                    .enum_type(&module.enums)
                    .expect("family metadata was validated before function bodies"),
                ForeignCallbackOperation::Failure(_) => family
                    .failure_result
                    .none()
                    .enum_type(&module.enums)
                    .expect("family metadata was validated before function bodies"),
            };
            if expr.ty != expected {
                return Err(MirValidationErrorKind::InvalidForeignCallbackExpression {
                    reason: "operation result does not match its typed family relation",
                });
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate_operand_type(
    module: &Module,
    operand: &Expr,
    variant: MirVariantRef,
    operation: MirVariantOperation,
) -> Result<(), MirValidationErrorKind> {
    let Type::Enum(actual, actual_arguments) = &operand.ty else {
        return Err(MirValidationErrorKind::VariantOperandIsNotEnum {
            operation,
            actual: operand.ty.clone(),
        });
    };
    if *actual != variant.enum_id() {
        return Err(MirValidationErrorKind::VariantOperandEnumMismatch {
            operation,
            expected: variant.enum_id(),
            actual: *actual,
        });
    }
    let expected_arguments = &module.enums[variant.enum_id()].type_arguments;
    if actual_arguments != expected_arguments {
        return Err(
            MirValidationErrorKind::VariantOperandTypeArgumentsMismatch {
                operation,
                enum_id: variant.enum_id(),
                expected: expected_arguments.clone(),
                actual: actual_arguments.clone(),
            },
        );
    }
    Ok(())
}

fn stable_locals(body: &Body) -> Vec<bool> {
    let mut stable = body
        .locals
        .iter()
        .map(|(_, local)| !local.mutable)
        .collect::<Vec<_>>();
    for (_, block) in body.blocks.iter() {
        for statement in &block.statements {
            if let StatementKind::Assign { local, .. } = statement.kind {
                let index = local.into_raw().into_u32() as usize;
                if let Some(stable) = stable.get_mut(index) {
                    *stable = false;
                }
            }
        }
    }
    stable
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StableVariantValue(LocalId);

fn stable_variant_value(
    body: &Body,
    stable_locals: &[bool],
    operand: &Expr,
) -> Option<StableVariantValue> {
    let ExprKind::Local(local) = operand.kind else {
        return None;
    };
    let index = local.into_raw().into_u32() as usize;
    if !stable_locals.get(index).copied().unwrap_or(false) {
        return None;
    }
    (body.locals[local].ty == operand.ty).then_some(StableVariantValue(local))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CfgEdgeKind {
    Goto,
    BranchTrue,
    BranchFalse,
    BlockUnwind,
    ThrowUnwind,
    RethrowUnwind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CfgEdge {
    from: BlockId,
    to: BlockId,
    kind: CfgEdgeKind,
}

#[derive(Debug, Clone, Copy)]
struct VariantProof {
    value: StableVariantValue,
    variant: MirVariantRef,
    edge: CfgEdge,
}

fn matching_test_edges(module: &Module, body: &Body, stable_locals: &[bool]) -> Vec<VariantProof> {
    let mut proofs = Vec::new();
    for (block, definition) in body.blocks.iter() {
        let Terminator::Branch {
            cond, then_block, ..
        } = &definition.terminator
        else {
            continue;
        };
        let ExprKind::VariantTest { operand, variant } = &cond.kind else {
            continue;
        };
        if cond.ty != Type::Boolean
            || variant.definition(&module.enums).is_err()
            || validate_operand_type(module, operand, *variant, MirVariantOperation::Test).is_err()
        {
            continue;
        }
        let Some(value) = stable_variant_value(body, stable_locals, operand) else {
            continue;
        };
        proofs.push(VariantProof {
            value,
            variant: *variant,
            edge: CfgEdge {
                from: block,
                to: *then_block,
                kind: CfgEdgeKind::BranchTrue,
            },
        });
    }
    proofs
}

fn edge_dominates_block(body: &Body, edge: CfgEdge, target: BlockId) -> bool {
    is_reachable(body, target, None) && !is_reachable(body, target, Some(edge))
}

fn is_reachable(body: &Body, target: BlockId, skipped: Option<CfgEdge>) -> bool {
    if !valid_block(body, body.entry) || !valid_block(body, target) {
        return false;
    }
    let mut visited = vec![false; body.blocks.len()];
    let mut work = vec![body.entry];
    while let Some(block) = work.pop() {
        let index = block.into_raw().into_u32() as usize;
        if visited[index] {
            continue;
        }
        visited[index] = true;
        if block == target {
            return true;
        }
        for edge in block_edges(body, block) {
            if Some(edge) != skipped && valid_block(body, edge.to) {
                work.push(edge.to);
            }
        }
    }
    false
}

fn valid_block(body: &Body, block: BlockId) -> bool {
    (block.into_raw().into_u32() as usize) < body.blocks.len()
}

fn block_edges(body: &Body, from: BlockId) -> Vec<CfgEdge> {
    let block = &body.blocks[from];
    let mut edges = Vec::new();
    if let Some(to) = block.unwind {
        edges.push(CfgEdge {
            from,
            to,
            kind: CfgEdgeKind::BlockUnwind,
        });
    }
    match block.terminator {
        Terminator::Goto(to) => edges.push(CfgEdge {
            from,
            to,
            kind: CfgEdgeKind::Goto,
        }),
        Terminator::Branch {
            then_block,
            else_block,
            ..
        } => {
            edges.push(CfgEdge {
                from,
                to: then_block,
                kind: CfgEdgeKind::BranchTrue,
            });
            edges.push(CfgEdge {
                from,
                to: else_block,
                kind: CfgEdgeKind::BranchFalse,
            });
        }
        Terminator::Throw {
            unwind: Some(to), ..
        } => edges.push(CfgEdge {
            from,
            to,
            kind: CfgEdgeKind::ThrowUnwind,
        }),
        Terminator::Rethrow { unwind: Some(to) } => edges.push(CfgEdge {
            from,
            to,
            kind: CfgEdgeKind::RethrowUnwind,
        }),
        Terminator::Return { .. }
        | Terminator::Throw { unwind: None, .. }
        | Terminator::Rethrow { unwind: None }
        | Terminator::Resume
        | Terminator::Trap { .. }
        | Terminator::Unreachable => {}
    }
    edges
}

fn try_visit_block_exprs(
    block: &BasicBlock,
    visitor: &mut impl FnMut(&Expr) -> Result<(), MirValidationError>,
) -> Result<(), MirValidationError> {
    let mut result = Ok(());
    visit_block_exprs(block, &mut |expr| {
        if result.is_ok() {
            result = visitor(expr);
        }
    });
    result
}

#[cfg(test)]
pub(crate) mod tests;
