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
use boxed_values::{validate_boxed_value_metadata, validate_boxing_adjust_metadata};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MirVariantOperation {
    Construct,
    Test,
    PayloadProject,
}

impl MirVariantOperation {
    const fn name(self) -> &'static str {
        match self {
            Self::Construct => "VariantConstruct",
            Self::Test => "VariantTest",
            Self::PayloadProject => "VariantPayloadProject",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum MirValidationErrorKind {
    InvalidStructCAbiProjection,
    InvalidLoopHeaderPollTarget,
    DuplicateLoopHeaderPollTarget,
    InvalidOptionCore,
    InvalidCoroutineStep,
    InvalidCoroutineSlot,
    InvalidSourceCallableMaterialization {
        reason: &'static str,
    },
    InvalidLocalValue {
        reason: &'static str,
    },
    InvalidGeneratedExactType {
        reason: &'static str,
    },
    InvalidRuntimeTypeIdentity {
        reason: &'static str,
    },
    InvalidGeneratedCallable {
        reason: &'static str,
    },
    DuplicateImmortalObjectIdentity {
        previous: StringConstId,
    },
    InvalidImmortalObjectOwner {
        reason: &'static str,
    },
    InvalidCallableFunction {
        reason: &'static str,
    },
    InvalidCallableSignature {
        reason: &'static str,
    },
    InvalidCoroutineMetadata {
        reason: &'static str,
    },
    NonRootCoroutinePendingContext,
    InvalidExternalCallableReference {
        callable: ExternalCallableUseId,
    },
    ExternalCallableRequiresDirect,
    InvalidForeignCallbackFamily {
        reason: &'static str,
    },
    InvalidForeignCallbackBridge {
        reason: &'static str,
    },
    InvalidCallbackBridge {
        reason: &'static str,
    },
    InvalidForeignCallbackExpression {
        reason: &'static str,
    },
    InvalidFunctionAdapter {
        reason: &'static str,
    },
    InvalidFunctionBridge {
        reason: &'static str,
    },
    InvalidClosureEnvironment {
        reason: &'static str,
    },
    InvalidClosureExpression {
        reason: &'static str,
    },
    InvalidBoxedValue {
        reason: &'static str,
    },
    InvalidBoxingAdjust {
        reason: &'static str,
    },
    InvalidConstantImage {
        path: Vec<u32>,
        expected: Type,
        error: MirConstantImageError,
    },
    InvalidStructReference {
        struct_id: StructId,
    },
    RawStructConstructRequiresDeclared {
        struct_id: StructId,
    },
    RawStructConstructResultType {
        struct_id: StructId,
        actual: Type,
    },
    RawStructConstructArity {
        struct_id: StructId,
        expected: usize,
        actual: usize,
    },
    RawStructConstructFieldType {
        struct_id: StructId,
        field: u32,
        expected: Type,
        actual: Type,
    },
    InvalidVariantReference {
        operation: MirVariantOperation,
        error: MirVariantRefError,
    },
    InvalidVariantFieldReference {
        error: MirVariantFieldRefError,
    },
    VariantConstructResultType {
        variant: MirVariantRef,
        actual: Type,
    },
    VariantConstructArity {
        variant: MirVariantRef,
        expected: usize,
        actual: usize,
    },
    VariantConstructFieldType {
        field: MirVariantFieldRef,
        expected: Type,
        actual: Type,
    },
    VariantOperandIsNotEnum {
        operation: MirVariantOperation,
        actual: Type,
    },
    VariantOperandEnumMismatch {
        operation: MirVariantOperation,
        expected: EnumId,
        actual: EnumId,
    },
    VariantOperandTypeArgumentsMismatch {
        operation: MirVariantOperation,
        enum_id: EnumId,
        expected: Vec<Type>,
        actual: Vec<Type>,
    },
    VariantTestResultType {
        actual: Type,
    },
    VariantPayloadResultType {
        field: MirVariantFieldRef,
        expected: Type,
        actual: Type,
    },
    VariantPayloadNotDominated {
        field: MirVariantFieldRef,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MirValidationLocation {
    OptionCore {
        enumeration: EnumId,
    },
    CoroutineStep {
        step: CoroutineStepId,
    },
    CoroutineSlot {
        slot: CoroutineSlotId,
    },
    SourceCallableMaterialization {
        function: FunctionId,
    },
    LocalValue {
        function: FunctionId,
        local: LocalId,
    },
    GeneratedExactType {
        entry: u32,
    },
    RuntimeType {
        location: MirRuntimeTypeLocation,
    },
    GeneratedCallable {
        entry: u32,
    },
    StringConstant {
        string: StringConstId,
    },
    CallableFunction {
        function: FunctionId,
    },
    CallableSignature {
        entry: u32,
    },
    ContinuationShell {
        shell: u32,
    },
    CoroutineStart {
        start: u32,
    },
    CoroutineSavedValue {
        value: CoroutineSavedValueId,
    },
    CoroutineFailureValue {
        value: CoroutineFailureValueId,
    },
    CoroutineFrame {
        frame: CoroutineFrameId,
    },
    CoroutineResumePoint {
        point: CoroutineResumePointId,
    },
    CoroutineFunction {
        coroutine: CoroutineFunctionId,
    },
    ForeignCallbackFamily {
        family: ForeignCallbackFamilyId,
    },
    ForeignCallbackBridge {
        bridge: ForeignCallbackBridgeId,
    },
    CallbackBridge {
        bridge: CallbackBridgeId,
    },
    FunctionAdapter {
        adapter: ClosureAdapterId,
    },
    DynamicFunctionAdapter {
        adapter: DynamicClosureAdapterId,
    },
    FunctionBridge {
        bridge: u32,
    },
    ClosureEnvironment {
        environment: u32,
    },
    BoxedValue {
        boxed: u32,
    },
    BoxingAdjust {
        adjust: u32,
    },
    FunctionBlock {
        function: FunctionId,
        block: BlockId,
    },
    Global {
        global: GlobalId,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct MirValidationError {
    pub location: MirValidationLocation,
    pub kind: MirValidationErrorKind,
}

impl std::fmt::Display for MirValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.location {
            MirValidationLocation::OptionCore { enumeration } => write!(
                formatter,
                "invalid MIR Option metadata for enum {}: ",
                enumeration.into_raw().into_u32()
            )?,
            MirValidationLocation::CoroutineStep { step } => write!(
                formatter,
                "invalid MIR CoroutineStep metadata {}: ",
                step.into_raw().into_u32()
            )?,
            MirValidationLocation::CoroutineSlot { slot } => write!(
                formatter,
                "invalid MIR CoroutineSlot metadata {}: ",
                slot.into_raw().into_u32()
            )?,
            MirValidationLocation::SourceCallableMaterialization { function } => write!(
                formatter,
                "invalid MIR source callable materialization at function {}: ",
                function.into_raw().into_u32()
            )?,
            MirValidationLocation::LocalValue { function, local } => write!(
                formatter,
                "invalid MIR local value at function {}, local {}: ",
                function.into_raw().into_u32(),
                local.into_raw().into_u32()
            )?,
            MirValidationLocation::GeneratedExactType { entry } => write!(
                formatter,
                "invalid MIR generated exact-type metadata {entry}: "
            )?,
            MirValidationLocation::RuntimeType { location } => write!(
                formatter,
                "invalid MIR runtime type identity for {location}: "
            )?,
            MirValidationLocation::GeneratedCallable { entry } => write!(
                formatter,
                "invalid MIR generated callable metadata {entry}: "
            )?,
            MirValidationLocation::StringConstant { string } => write!(
                formatter,
                "invalid MIR string constant {}: ",
                string.into_raw().into_u32()
            )?,
            MirValidationLocation::CallableFunction { function } => write!(
                formatter,
                "invalid MIR emitted callable function {}: ",
                function.into_raw().into_u32()
            )?,
            MirValidationLocation::CallableSignature { entry } => write!(
                formatter,
                "invalid MIR callable signature metadata {entry}: "
            )?,
            MirValidationLocation::ContinuationShell { shell } => write!(
                formatter,
                "invalid MIR continuation-shell metadata {shell}: "
            )?,
            MirValidationLocation::CoroutineStart { start } => {
                write!(formatter, "invalid MIR coroutine-start metadata {start}: ")?
            }
            MirValidationLocation::CoroutineSavedValue { value } => write!(
                formatter,
                "invalid MIR coroutine saved-value metadata {}: ",
                value.into_raw().into_u32()
            )?,
            MirValidationLocation::CoroutineFailureValue { value } => write!(
                formatter,
                "invalid MIR coroutine failure-value metadata {}: ",
                value.into_raw().into_u32()
            )?,
            MirValidationLocation::CoroutineFrame { frame } => write!(
                formatter,
                "invalid MIR coroutine frame metadata {}: ",
                frame.into_raw().into_u32()
            )?,
            MirValidationLocation::CoroutineResumePoint { point } => write!(
                formatter,
                "invalid MIR coroutine resume-point metadata {}: ",
                point.into_raw().into_u32()
            )?,
            MirValidationLocation::CoroutineFunction { coroutine } => write!(
                formatter,
                "invalid MIR coroutine function metadata {}: ",
                coroutine.into_raw().into_u32()
            )?,
            MirValidationLocation::ForeignCallbackFamily { family } => write!(
                formatter,
                "invalid MIR foreign callback family {}: ",
                family.into_raw().into_u32()
            )?,
            MirValidationLocation::ForeignCallbackBridge { bridge } => write!(
                formatter,
                "invalid MIR foreign callback bridge {}: ",
                bridge.into_raw().into_u32()
            )?,
            MirValidationLocation::CallbackBridge { bridge } => write!(
                formatter,
                "invalid MIR callback bridge {}: ",
                bridge.into_raw().into_u32()
            )?,
            MirValidationLocation::FunctionAdapter { adapter } => write!(
                formatter,
                "invalid MIR function adapter {}: ",
                adapter.into_raw().into_u32()
            )?,
            MirValidationLocation::DynamicFunctionAdapter { adapter } => write!(
                formatter,
                "invalid MIR dynamic function adapter {}: ",
                adapter.into_raw().into_u32()
            )?,
            MirValidationLocation::FunctionBridge { bridge } => {
                write!(formatter, "invalid MIR function bridge {bridge}: ")?
            }
            MirValidationLocation::ClosureEnvironment { environment } => {
                write!(formatter, "invalid MIR closure environment {environment}: ")?
            }
            MirValidationLocation::BoxedValue { boxed } => {
                write!(formatter, "invalid MIR boxed value {boxed}: ")?
            }
            MirValidationLocation::BoxingAdjust { adjust } => {
                write!(formatter, "invalid MIR boxing adjust {adjust}: ")?
            }
            MirValidationLocation::FunctionBlock { function, block } => write!(
                formatter,
                "invalid MIR in function {}, block {}: ",
                function.into_raw().into_u32(),
                block.into_raw().into_u32()
            )?,
            MirValidationLocation::Global { global } => write!(
                formatter,
                "invalid MIR global {} constant image: ",
                global.into_raw().into_u32()
            )?,
        }
        match &self.kind {
            MirValidationErrorKind::InvalidStructCAbiProjection => {
                formatter.write_str("C UInt64 projection requires the exact sole UInt64 field of a GC-free ordinary struct")
            }
            MirValidationErrorKind::InvalidLoopHeaderPollTarget => {
                formatter.write_str("loop-header poll target is outside the function body")
            }
            MirValidationErrorKind::DuplicateLoopHeaderPollTarget => {
                formatter.write_str("loop-header poll target is registered more than once")
            }
            MirValidationErrorKind::InvalidOptionCore => {
                formatter.write_str("stored Some/None identities no longer match the enum")
            }
            MirValidationErrorKind::InvalidCoroutineStep => formatter.write_str(
                "stored Completed/Suspended identities no longer match the coroutine-step enum",
            ),
            MirValidationErrorKind::InvalidCoroutineSlot => formatter
                .write_str("stored Value/Empty identities no longer match the coroutine-slot enum"),
            MirValidationErrorKind::InvalidSourceCallableMaterialization { reason } => {
                formatter.write_str(reason)
            }
            MirValidationErrorKind::InvalidLocalValue { reason } => formatter.write_str(reason),
            MirValidationErrorKind::InvalidGeneratedExactType { reason } => {
                formatter.write_str(reason)
            }
            MirValidationErrorKind::InvalidRuntimeTypeIdentity { reason } => {
                formatter.write_str(reason)
            }
            MirValidationErrorKind::InvalidGeneratedCallable { reason } => {
                formatter.write_str(reason)
            }
            MirValidationErrorKind::DuplicateImmortalObjectIdentity { previous } => write!(
                formatter,
                "duplicates the immortal-object identity of string constant {}",
                previous.into_raw().into_u32()
            ),
            MirValidationErrorKind::InvalidImmortalObjectOwner { reason } => {
                formatter.write_str(reason)
            }
            MirValidationErrorKind::InvalidCallableFunction { reason } => {
                formatter.write_str(reason)
            }
            MirValidationErrorKind::InvalidCallableSignature { reason } => {
                formatter.write_str(reason)
            }
            MirValidationErrorKind::InvalidCoroutineMetadata { reason } => {
                formatter.write_str(reason)
            }
            MirValidationErrorKind::NonRootCoroutinePendingContext => formatter.write_str(
                "transient coroutine pending context remains after state-machine conversion",
            ),
            MirValidationErrorKind::InvalidExternalCallableReference { callable } => write!(
                formatter,
                "external callable {} is missing from this MIR module",
                callable.into_raw()
            ),
            MirValidationErrorKind::ExternalCallableRequiresDirect => {
                formatter.write_str("external callable requires direct dispatch")
            }
            MirValidationErrorKind::InvalidForeignCallbackFamily { reason }
            | MirValidationErrorKind::InvalidForeignCallbackBridge { reason }
            | MirValidationErrorKind::InvalidCallbackBridge { reason }
            | MirValidationErrorKind::InvalidForeignCallbackExpression { reason }
            | MirValidationErrorKind::InvalidFunctionAdapter { reason }
            | MirValidationErrorKind::InvalidFunctionBridge { reason }
            | MirValidationErrorKind::InvalidClosureEnvironment { reason }
            | MirValidationErrorKind::InvalidClosureExpression { reason }
            | MirValidationErrorKind::InvalidBoxedValue { reason }
            | MirValidationErrorKind::InvalidBoxingAdjust { reason } => formatter.write_str(reason),
            MirValidationErrorKind::InvalidConstantImage {
                path,
                expected,
                error,
            } => {
                if !path.is_empty() {
                    write!(formatter, "at struct field path {path:?}, ")?;
                }
                match error {
                    MirConstantImageError::TypeMismatch { image } => {
                        write!(
                            formatter,
                            "{image} does not match expected type {expected:?}"
                        )
                    }
                    MirConstantImageError::InvalidStringReference { string } => write!(
                        formatter,
                        "references unknown MIR string {}",
                        string.into_raw().into_u32()
                    ),
                    MirConstantImageError::InvalidStructReference { struct_id } => write!(
                        formatter,
                        "references unknown MIR struct {}",
                        struct_id.into_raw().into_u32()
                    ),
                    MirConstantImageError::StructRequiresDeclared { struct_id } => write!(
                        formatter,
                        "MIR struct {} does not have a declared representation",
                        struct_id.into_raw().into_u32()
                    ),
                    MirConstantImageError::StructArity {
                        struct_id,
                        expected,
                        actual,
                    } => write!(
                        formatter,
                        "MIR struct {} requires {expected} fields, got {actual}",
                        struct_id.into_raw().into_u32()
                    ),
                    MirConstantImageError::InvalidVariantReference { error } => {
                        write!(formatter, "EnumUnit carries {error}")
                    }
                    MirConstantImageError::EnumUnitHasPayload { variant } => write!(
                        formatter,
                        "EnumUnit for MIR enum {} variant {} requires a payloadless variant",
                        variant.enum_id().into_raw().into_u32(),
                        variant.variant_index()
                    ),
                }
            }
            MirValidationErrorKind::InvalidStructReference { struct_id } => write!(
                formatter,
                "StructConstruct references unknown MIR struct {}",
                struct_id.into_raw().into_u32()
            ),
            MirValidationErrorKind::RawStructConstructRequiresDeclared { struct_id } => write!(
                formatter,
                "StructConstruct for MIR struct {} requires a declared representation",
                struct_id.into_raw().into_u32()
            ),
            MirValidationErrorKind::RawStructConstructResultType { struct_id, actual } => write!(
                formatter,
                "StructConstruct for MIR struct {} must produce that exact struct type, got {actual:?}",
                struct_id.into_raw().into_u32()
            ),
            MirValidationErrorKind::RawStructConstructArity {
                struct_id,
                expected,
                actual,
            } => write!(
                formatter,
                "StructConstruct for MIR struct {} requires {expected} fields, got {actual}",
                struct_id.into_raw().into_u32()
            ),
            MirValidationErrorKind::RawStructConstructFieldType {
                struct_id,
                field,
                expected,
                actual,
            } => write!(
                formatter,
                "StructConstruct for MIR struct {} field {field} must have type {expected:?}, got {actual:?}",
                struct_id.into_raw().into_u32()
            ),
            MirValidationErrorKind::InvalidVariantReference { operation, error } => {
                write!(formatter, "{} carries {error}", operation.name())
            }
            MirValidationErrorKind::InvalidVariantFieldReference { error } => {
                write!(formatter, "VariantPayloadProject carries {error}")
            }
            MirValidationErrorKind::VariantConstructResultType { variant, actual } => write!(
                formatter,
                "VariantConstruct for MIR enum {} variant {} must produce that exact enum type, got {actual:?}",
                variant.enum_id().into_raw().into_u32(),
                variant.variant_index()
            ),
            MirValidationErrorKind::VariantConstructArity {
                variant,
                expected,
                actual,
            } => write!(
                formatter,
                "VariantConstruct for MIR enum {} variant {} requires {expected} fields, got {actual}",
                variant.enum_id().into_raw().into_u32(),
                variant.variant_index()
            ),
            MirValidationErrorKind::VariantConstructFieldType {
                field,
                expected,
                actual,
            } => write!(
                formatter,
                "VariantConstruct for MIR enum {} variant {} field {} must have type {expected:?}, got {actual:?}",
                field.variant().enum_id().into_raw().into_u32(),
                field.variant().variant_index(),
                field.field_index()
            ),
            MirValidationErrorKind::VariantOperandIsNotEnum { operation, actual } => write!(
                formatter,
                "{} requires an enum operand, got {actual:?}",
                operation.name()
            ),
            MirValidationErrorKind::VariantOperandEnumMismatch {
                operation,
                expected,
                actual,
            } => write!(
                formatter,
                "{} expects MIR enum {}, got MIR enum {}",
                operation.name(),
                expected.into_raw().into_u32(),
                actual.into_raw().into_u32()
            ),
            MirValidationErrorKind::VariantOperandTypeArgumentsMismatch {
                operation,
                enum_id,
                expected,
                actual,
            } => write!(
                formatter,
                "{} expects MIR enum {} arguments {expected:?}, got {actual:?}",
                operation.name(),
                enum_id.into_raw().into_u32()
            ),
            MirValidationErrorKind::VariantTestResultType { actual } => write!(
                formatter,
                "VariantTest result must be Boolean, got {actual:?}"
            ),
            MirValidationErrorKind::VariantPayloadResultType {
                field,
                expected,
                actual,
            } => write!(
                formatter,
                "VariantPayloadProject for MIR enum {} variant {} field {} must produce {expected:?}, got {actual:?}",
                field.variant().enum_id().into_raw().into_u32(),
                field.variant().variant_index(),
                field.field_index()
            ),
            MirValidationErrorKind::VariantPayloadNotDominated { field } => write!(
                formatter,
                "VariantPayloadProject for MIR enum {} variant {} field {} is not dominated by the true edge of a matching VariantTest on the same stable value",
                field.variant().enum_id().into_raw().into_u32(),
                field.variant().variant_index(),
                field.field_index()
            ),
        }
    }
}

impl std::error::Error for MirValidationError {}

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
    validate_boxing_adjust_metadata(module)?;
    validate_generated_exact_type_metadata(module)?;
    validate_runtime_type_metadata(module)?;
    validate_generated_callable_metadata(module)?;
    validate_immortal_objects(module)?;
    validate_callable_functions(module)?;
    validate_callable_signature_metadata(module)?;
    validate_constant_images(module)?;
    for (function_id, function) in module.functions.iter() {
        validate_body(module, function_id, &function.body)?;
    }
    Ok(())
}

fn validate_body(
    module: &Module,
    function: FunctionId,
    body: &Body,
) -> Result<(), MirValidationError> {
    let mut loop_header_polls = vec![false; body.blocks.len()];
    for target in &body.loop_header_polls {
        let block = target.header();
        let index = block.into_raw().into_u32() as usize;
        let Some(seen) = loop_header_polls.get_mut(index) else {
            return Err(MirValidationError {
                location: MirValidationLocation::FunctionBlock { function, block },
                kind: MirValidationErrorKind::InvalidLoopHeaderPollTarget,
            });
        };
        if std::mem::replace(seen, true) {
            return Err(MirValidationError {
                location: MirValidationLocation::FunctionBlock { function, block },
                kind: MirValidationErrorKind::DuplicateLoopHeaderPollTarget,
            });
        }
    }

    for (block, definition) in body.blocks.iter() {
        for statement in &definition.statements {
            let call = match &statement.kind {
                StatementKind::Call(CallEffect::Unit(call))
                | StatementKind::Call(CallEffect::Value { call, .. }) => Some(call),
                _ => None,
            };
            if call.is_some_and(|call| !call.pending.is_root()) {
                return Err(MirValidationError {
                    location: MirValidationLocation::FunctionBlock { function, block },
                    kind: MirValidationErrorKind::NonRootCoroutinePendingContext,
                });
            }
            if let Some(call) = call
                && let Err(kind) = validate_imported_call(module, call)
            {
                return Err(MirValidationError {
                    location: MirValidationLocation::FunctionBlock { function, block },
                    kind,
                });
            }
        }
        try_visit_block_exprs(definition, &mut |expr| {
            validate_expression_shape(module, expr).map_err(|kind| MirValidationError {
                location: MirValidationLocation::FunctionBlock { function, block },
                kind,
            })
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
                    location: MirValidationLocation::FunctionBlock { function, block },
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
                    location: MirValidationLocation::FunctionBlock { function, block },
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
    for statement in &block.statements {
        match &statement.kind {
            StatementKind::Expr(expr) => try_visit_expr(expr, visitor)?,
            StatementKind::Call(effect) => {
                let call = match effect {
                    CallEffect::Unit(call) | CallEffect::Value { call, .. } => call,
                };
                for argument in &call.args {
                    try_visit_expr(argument, visitor)?;
                }
            }
            StatementKind::ValDecl { init, .. } => try_visit_expr(init, visitor)?,
            StatementKind::Assign { value, .. } | StatementKind::GlobalAssign { value, .. } => {
                try_visit_expr(value, visitor)?
            }
            StatementKind::ArraySet {
                array,
                index,
                value,
                ..
            } => {
                try_visit_expr(array, visitor)?;
                try_visit_expr(index, visitor)?;
                try_visit_expr(value, visitor)?;
            }
            StatementKind::FieldSet { object, value, .. }
            | StatementKind::AtomicFieldStore { object, value, .. } => {
                try_visit_expr(object, visitor)?;
                try_visit_expr(value, visitor)?;
            }
            StatementKind::Eh(_) => {}
        }
    }
    match &block.terminator {
        Terminator::Branch { cond, .. } => try_visit_expr(cond, visitor)?,
        Terminator::Return { value: Some(value) } => try_visit_expr(value, visitor)?,
        Terminator::Throw { exception, .. } => try_visit_expr(exception, visitor)?,
        Terminator::Goto(_)
        | Terminator::Return { value: None }
        | Terminator::Rethrow { .. }
        | Terminator::Resume
        | Terminator::Trap { .. }
        | Terminator::Unreachable => {}
    }
    Ok(())
}

fn try_visit_expr(
    expr: &Expr,
    visitor: &mut impl FnMut(&Expr) -> Result<(), MirValidationError>,
) -> Result<(), MirValidationError> {
    let mut error = None;
    visit_expr(expr, &mut |expr| {
        if error.is_none()
            && let Err(found) = visitor(expr)
        {
            error = Some(found);
        }
    });
    match error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

#[cfg(test)]
pub(crate) mod tests;
