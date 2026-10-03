use super::*;

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
    InvalidRelease {
        reason: &'static str,
    },
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
    InvalidExternalDispatch {
        reason: &'static str,
    },
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
    ReleasePolicy {
        class: ClassId,
    },
    ReleaseHook {
        hook: ReleaseHookId,
    },
    ReleaseHookBlock {
        hook: ReleaseHookId,
        block: BlockId,
    },
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
        owner: LocalValueOwner,
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
            MirValidationLocation::ReleasePolicy { class } => {
                write!(formatter, "invalid MIR release policy for {class:?}: ")?
            }
            MirValidationLocation::ReleaseHook { hook } => {
                write!(formatter, "invalid MIR release hook {hook:?}: ")?
            }
            MirValidationLocation::ReleaseHookBlock { hook, block } => write!(
                formatter,
                "invalid MIR release hook {hook:?}, block {block:?}: "
            )?,
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
            MirValidationLocation::LocalValue { owner, local } => write!(
                formatter,
                "invalid MIR local value at {owner:?}, local {}: ",
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
            MirValidationErrorKind::InvalidRelease { reason } => formatter.write_str(reason),
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
            MirValidationErrorKind::InvalidExternalDispatch { reason } => {
                formatter.write_str(reason)
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
