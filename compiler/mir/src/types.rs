use super::*;
use std::num::NonZeroU32;

/// Compiler-owned scalar domains that never denote a Scoop source integer.
///
/// Keeping the semantic role in the type prevents enum discriminants,
/// runtime protocol outcomes, and coroutine synchronization words from
/// acquiring source `Int`/`UInt` operations, boxing, or FFI behavior merely
/// because their current physical representation is an integer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MachineScalarKind {
    EnumTag,
    InitializationOutcome,
    CoroutineFrameState,
    CoroutineAdapterState,
    ForeignCallbackStatus,
    PointerElementOffset,
}

impl MachineScalarKind {
    pub const fn name(self) -> &'static str {
        match self {
            Self::EnumTag => "enum-tag",
            Self::InitializationOutcome => "initialization-outcome",
            Self::CoroutineFrameState => "coroutine-frame-state",
            Self::CoroutineAdapterState => "coroutine-adapter-state",
            Self::ForeignCallbackStatus => "foreign-callback-status",
            Self::PointerElementOffset => "pointer-element-offset",
        }
    }

    pub const fn is_atomic_state(self) -> bool {
        matches!(
            self,
            Self::CoroutineFrameState | Self::CoroutineAdapterState
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InitializationOutcome {
    RunInitializer,
    Ready,
    Failed,
    Cycle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CoroutineSuspendStateId(NonZeroU32);

impl CoroutineSuspendStateId {
    pub fn new(raw: u32) -> Option<Self> {
        NonZeroU32::new(raw).map(Self)
    }

    pub const fn get(self) -> u32 {
        self.0.get()
    }
}

impl std::fmt::Display for CoroutineSuspendStateId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.get().fmt(formatter)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CoroutineFrameState {
    Initial,
    Running,
    Completed,
    /// A successful resume entry, keyed by the one-based suspension site.
    Suspended(CoroutineSuspendStateId),
    /// An exceptional resume entry for the same one-based suspension site.
    ResumeFailure(CoroutineSuspendStateId),
}

impl std::fmt::Display for CoroutineFrameState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Initial => formatter.write_str("initial"),
            Self::Running => formatter.write_str("running"),
            Self::Completed => formatter.write_str("completed"),
            Self::Suspended(site) => write!(formatter, "suspended.{site}"),
            Self::ResumeFailure(site) => write!(formatter, "resume_failure.{site}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CoroutineAdapterState {
    Registering,
    Waiting,
    CompletingSuccess,
    CompletingFailure,
    LatchedSuccess,
    LatchedFailure,
    Consumed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ForeignCallbackStatus {
    Returned,
    Threw,
}

/// A semantic constant in one compiler-owned scalar domain.
///
/// The variant determines the kind, so a value cannot carry a contradictory
/// parallel kind field. `raw_bits` is the frozen runtime encoding consumed by
/// lower stages; it does not turn the value into a source unsigned integer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MachineScalarValue {
    EnumTag(u32),
    InitializationOutcome(InitializationOutcome),
    CoroutineFrameState(CoroutineFrameState),
    CoroutineAdapterState(CoroutineAdapterState),
    ForeignCallbackStatus(ForeignCallbackStatus),
    PointerElementOffset(u64),
}

impl MachineScalarValue {
    pub const fn kind(self) -> MachineScalarKind {
        match self {
            Self::EnumTag(_) => MachineScalarKind::EnumTag,
            Self::InitializationOutcome(_) => MachineScalarKind::InitializationOutcome,
            Self::CoroutineFrameState(_) => MachineScalarKind::CoroutineFrameState,
            Self::CoroutineAdapterState(_) => MachineScalarKind::CoroutineAdapterState,
            Self::ForeignCallbackStatus(_) => MachineScalarKind::ForeignCallbackStatus,
            Self::PointerElementOffset(_) => MachineScalarKind::PointerElementOffset,
        }
    }

    pub const fn raw_bits(self) -> u64 {
        match self {
            Self::EnumTag(tag) => tag as u64,
            Self::InitializationOutcome(outcome) => match outcome {
                InitializationOutcome::RunInitializer => 0,
                InitializationOutcome::Ready => 1,
                InitializationOutcome::Failed => 2,
                InitializationOutcome::Cycle => 3,
            },
            Self::CoroutineFrameState(state) => match state {
                CoroutineFrameState::Initial => 0,
                CoroutineFrameState::Running => u64::MAX,
                CoroutineFrameState::Completed => u64::MAX - 1,
                CoroutineFrameState::Suspended(site) => site.get() as u64,
                CoroutineFrameState::ResumeFailure(site) => u64::MAX - site.get() as u64 - 1,
            },
            Self::CoroutineAdapterState(state) => match state {
                CoroutineAdapterState::Registering => 0,
                CoroutineAdapterState::Waiting => 1,
                CoroutineAdapterState::CompletingSuccess => 2,
                CoroutineAdapterState::CompletingFailure => 3,
                CoroutineAdapterState::LatchedSuccess => 4,
                CoroutineAdapterState::LatchedFailure => 5,
                CoroutineAdapterState::Consumed => 6,
            },
            Self::ForeignCallbackStatus(status) => match status {
                ForeignCallbackStatus::Returned => 0,
                ForeignCallbackStatus::Threw => 1,
            },
            Self::PointerElementOffset(offset) => offset,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Unit,
    /// One exact Scoop source integer. Its nominal owner is already canonical
    /// (transparent aliases have disappeared before MIR).
    Integer(IntegerKind),
    /// An internal typed scalar, disjoint from every source integer type.
    MachineScalar(MachineScalarKind),
    Boolean,
    String,
    Struct(StructId),
    /// A reference type declared with `class`.
    Class(ClassId),
    /// An interface type (dispatch through itables, impl spec 2.9).
    Interface(InterfaceId),
    /// The root of all types; boxed value types live behind it.
    Any,
    Tuple(Vec<Type>),
    /// Concrete managed function signature. Function values have reference
    /// representation; closure classes are materialized by M11 conversion.
    Function(FunctionTypeId),
    /// Typed raw data pointer; representation is one native pointer word.
    Ptr(Box<Type>),
    /// Typed C function pointer; identity includes its exact signature.
    FunPtr(FunctionTypeId),
    /// An instantiated enum type (including `Option<T>` since M4).
    Enum(EnumId, Vec<Type>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionType {
    pub is_suspend: bool,
    pub parameter_types: Vec<Type>,
    pub return_type: Type,
}

#[derive(Debug)]
pub struct ClosureClass {
    /// Compiler-generated native-emission identity, separate from `name`.
    pub link_stem: NominalLinkStem,
    pub name: String,
    pub function_type: FunctionTypeId,
    pub invoke: ClosureInvokeFunctionId,
    pub captures: Vec<Field>,
    /// Function-type views supported by this exact closure class. Each slot
    /// is a typed forwarding entry whose ABI is `target`.
    pub bridges: Vec<FunctionBridge>,
}

#[derive(Debug)]
pub struct FunctionBridge {
    pub target: FunctionTypeId,
    pub function: FunctionId,
}

#[derive(Debug)]
pub struct ClosureInvokeFunction {
    pub function: FunctionId,
}

/// Opaque request-local identity of one nominal declaration or one
/// compiler-generated nominal role. It is an emission input, not a
/// source-facing name and not the persistent identity frozen by M23-2.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NominalLinkStem(String);

impl NominalLinkStem {
    pub fn from_session_local_encoding(encoding: String) -> Self {
        assert!(!encoding.is_empty(), "a nominal link stem cannot be empty");
        Self(encoding)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug)]
pub struct StructDef {
    pub link_stem: NominalLinkStem,
    pub name: String,
    /// Canonical arguments of this fully specialized application.
    pub type_arguments: Vec<Type>,
    /// Fixed after all type parameters have been resolved and this MIR
    /// type entity has a complete concrete field list.
    pub gc_free: bool,
    pub representation: StructRepresentation,
}

#[derive(Debug)]
pub enum StructRepresentation {
    Declared {
        c_layout: Option<MirCLayoutContract>,
        interior_mutable: bool,
        fields: Vec<Field>,
    },
    Intrinsic(IntrinsicTypeRepresentation),
}

impl StructDef {
    /// MIR type whose physical layout is defined by this declaration.
    pub fn physical_type(&self, id: StructId) -> Type {
        match &self.representation {
            StructRepresentation::Declared { .. } => Type::Struct(id),
            StructRepresentation::Intrinsic(representation) => match representation {
                IntrinsicTypeRepresentation::Integer(kind) => Type::Integer(*kind),
                IntrinsicTypeRepresentation::Boolean => Type::Boolean,
                IntrinsicTypeRepresentation::Ptr { pointee } => {
                    Type::Ptr(Box::new(pointee.clone()))
                }
                IntrinsicTypeRepresentation::FunPtr { signature } => Type::FunPtr(*signature),
                IntrinsicTypeRepresentation::String
                | IntrinsicTypeRepresentation::Array { .. }
                | IntrinsicTypeRepresentation::MutableArray { .. } => {
                    unreachable!("the MIR intrinsic registry fixes physical declaration kinds")
                }
            },
        }
    }

    pub fn declared_fields(&self) -> &[Field] {
        match &self.representation {
            StructRepresentation::Declared { fields, .. } => fields,
            StructRepresentation::Intrinsic(_) => {
                panic!("an intrinsic struct has no declared field representation")
            }
        }
    }

    pub fn declared_fields_mut(&mut self) -> &mut Vec<Field> {
        match &mut self.representation {
            StructRepresentation::Declared { fields, .. } => fields,
            StructRepresentation::Intrinsic(_) => {
                panic!("an intrinsic struct has no declared field representation")
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MirCLayoutValue {
    Natural,
    A1,
    A2,
    A4,
    A8,
    A16,
}

impl MirCLayoutValue {
    pub const fn bytes(self) -> Option<u8> {
        match self {
            Self::Natural => None,
            Self::A1 => Some(1),
            Self::A2 => Some(2),
            Self::A4 => Some(4),
            Self::A8 => Some(8),
            Self::A16 => Some(16),
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Natural => "natural",
            Self::A1 => "1",
            Self::A2 => "2",
            Self::A4 => "4",
            Self::A8 => "8",
            Self::A16 => "16",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MirCLayoutContract {
    pub aligned: MirCLayoutValue,
    pub packed: MirCLayoutValue,
}

#[derive(Debug)]
pub struct Field {
    pub name: String,
    pub ty: Type,
}

/// An instantiated enum definition (M4): variants with concrete field
/// types. `name` is the mangled instance name (e.g. `Option$I`).
#[derive(Debug)]
pub struct EnumDef {
    pub link_stem: NominalLinkStem,
    pub name: String,
    /// Canonical concrete arguments of this monomorphized enum instance.
    /// Together with the arena id these recover its exact MIR `Type`.
    pub type_arguments: Vec<Type>,
    /// True exactly when every fully specialized variant is GC-free.
    pub gc_free: bool,
    pub variants: Vec<VariantDef>,
}

#[derive(Debug)]
pub struct VariantDef {
    pub name: String,
    /// GC-free classification of this fully specialized variant.
    pub gc_free: bool,
    /// Fields in declaration order (named and positional forms both
    /// normalized; positional fields carry `_1`-style names).
    pub fields: Vec<Field>,
}

/// A variant identity checked against one concrete MIR enum definition.
///
/// The fields are private deliberately: a raw enum id and variant index cannot
/// be paired at an expression site without first consulting the enum arena.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MirVariantRef {
    enum_id: EnumId,
    variant: u32,
}

impl MirVariantRef {
    pub fn new(
        enums: &Arena<EnumDef>,
        enum_id: EnumId,
        variant: u32,
    ) -> Result<Self, MirVariantRefError> {
        let enum_index = enum_id.into_raw().into_u32() as usize;
        if enum_index >= enums.len() {
            return Err(MirVariantRefError::UnknownEnum { enum_id });
        }
        let variant_count = enums[enum_id].variants.len();
        if variant as usize >= variant_count {
            return Err(MirVariantRefError::VariantOutOfBounds {
                enum_id,
                variant,
                variant_count,
            });
        }
        Ok(Self { enum_id, variant })
    }

    pub const fn enum_id(self) -> EnumId {
        self.enum_id
    }

    pub const fn variant_index(self) -> u32 {
        self.variant
    }

    pub fn definition(self, enums: &Arena<EnumDef>) -> Result<&VariantDef, MirVariantRefError> {
        Self::new(enums, self.enum_id, self.variant)?;
        Ok(&enums[self.enum_id].variants[self.variant as usize])
    }

    pub fn enum_type(self, enums: &Arena<EnumDef>) -> Result<Type, MirVariantRefError> {
        self.definition(enums)?;
        Ok(Type::Enum(
            self.enum_id,
            enums[self.enum_id].type_arguments.clone(),
        ))
    }
}

/// A payload-field identity inseparably bound to its checked enum variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MirVariantFieldRef {
    variant: MirVariantRef,
    field: u32,
}

impl MirVariantFieldRef {
    pub fn new(
        enums: &Arena<EnumDef>,
        variant: MirVariantRef,
        field: u32,
    ) -> Result<Self, MirVariantFieldRefError> {
        let definition = variant
            .definition(enums)
            .map_err(MirVariantFieldRefError::InvalidVariant)?;
        let field_count = definition.fields.len();
        if field as usize >= field_count {
            return Err(MirVariantFieldRefError::FieldOutOfBounds {
                variant,
                field,
                field_count,
            });
        }
        Ok(Self { variant, field })
    }

    pub const fn variant(self) -> MirVariantRef {
        self.variant
    }

    pub const fn field_index(self) -> u32 {
        self.field
    }

    pub fn definition(self, enums: &Arena<EnumDef>) -> Result<&Field, MirVariantFieldRefError> {
        let variant = self
            .variant
            .definition(enums)
            .map_err(MirVariantFieldRefError::InvalidVariant)?;
        variant
            .fields
            .get(self.field as usize)
            .ok_or(MirVariantFieldRefError::FieldOutOfBounds {
                variant: self.variant,
                field: self.field,
                field_count: variant.fields.len(),
            })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MirVariantRefError {
    UnknownEnum {
        enum_id: EnumId,
    },
    VariantOutOfBounds {
        enum_id: EnumId,
        variant: u32,
        variant_count: usize,
    },
}

impl std::fmt::Display for MirVariantRefError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownEnum { enum_id } => write!(
                formatter,
                "unknown MIR enum {}",
                enum_id.into_raw().into_u32()
            ),
            Self::VariantOutOfBounds {
                enum_id,
                variant,
                variant_count,
            } => write!(
                formatter,
                "variant {variant} is out of bounds for MIR enum {} with {variant_count} variants",
                enum_id.into_raw().into_u32()
            ),
        }
    }
}

impl std::error::Error for MirVariantRefError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MirVariantFieldRefError {
    InvalidVariant(MirVariantRefError),
    FieldOutOfBounds {
        variant: MirVariantRef,
        field: u32,
        field_count: usize,
    },
}

impl std::fmt::Display for MirVariantFieldRefError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidVariant(error) => error.fmt(formatter),
            Self::FieldOutOfBounds {
                variant,
                field,
                field_count,
            } => write!(
                formatter,
                "field {field} is out of bounds for MIR enum {} variant {} with {field_count} fields",
                variant.enum_id().into_raw().into_u32(),
                variant.variant_index()
            ),
        }
    }
}

impl std::error::Error for MirVariantFieldRefError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassModifier {
    Final,
    Open,
    Abstract,
}

/// A class definition with its dispatch layout fixed by mir-lower
/// (impl spec 2.9).
#[derive(Debug)]
pub struct ClassDef {
    pub modifier: ClassModifier,
    pub link_stem: NominalLinkStem,
    pub name: String,
    /// Canonical arguments of this fully specialized application.
    pub type_arguments: Vec<Type>,
    pub representation: ClassRepresentation,
    pub interfaces: Vec<InterfaceId>,
    /// Ordinary virtual methods in vtable order (overrides share the base
    /// slot). Empty vtables are valid and have no implicit prefix.
    pub vtable: Vec<TableSlot>,
    /// itable entries, one per implemented interface (pointer-keyed
    /// lookup at runtime).
    pub itables: Vec<ItableRecord>,
}

#[derive(Debug)]
pub enum ClassRepresentation {
    Declared {
        /// Constructor properties in flattened base-first order.
        fields: Vec<Field>,
        base_class: Option<ClassId>,
    },
    Intrinsic(IntrinsicTypeRepresentation),
}

impl ClassDef {
    pub fn declared_fields(&self) -> &[Field] {
        match &self.representation {
            ClassRepresentation::Declared { fields, .. } => fields,
            ClassRepresentation::Intrinsic(_) => {
                panic!("an intrinsic class has no declared field representation")
            }
        }
    }

    pub fn declared_fields_mut(&mut self) -> &mut Vec<Field> {
        match &mut self.representation {
            ClassRepresentation::Declared { fields, .. } => fields,
            ClassRepresentation::Intrinsic(_) => {
                panic!("an intrinsic class has no declared field representation")
            }
        }
    }

    pub fn base_class(&self) -> Option<ClassId> {
        match &self.representation {
            ClassRepresentation::Declared { base_class, .. } => *base_class,
            ClassRepresentation::Intrinsic(_) => None,
        }
    }
}

/// The exact compiler representation selected upstream for this fully
/// specialized nominal type. Family variants carry their MIR element type.
#[derive(Debug, Clone, PartialEq)]
pub enum IntrinsicTypeRepresentation {
    Integer(IntegerKind),
    Boolean,
    String,
    Array {
        element: Type,
    },
    MutableArray {
        element: Type,
    },
    /// Intrinsic `Ptr<T>` declaration provenance. Canonical values still use
    /// [`Type::Ptr`], never this declaration as an ordinary struct payload.
    Ptr {
        pointee: Type,
    },
    /// Intrinsic `FunPtr<F>` declaration provenance. Canonical values still
    /// use [`Type::FunPtr`].
    FunPtr {
        signature: FunctionTypeId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrayKind {
    Immutable,
    Mutable,
}

pub fn array_type<'a>(module: &'a Module, ty: &Type) -> Option<(ArrayKind, &'a Type)> {
    let Type::Class(class) = ty else {
        return None;
    };
    match &module.classes[*class].representation {
        ClassRepresentation::Intrinsic(IntrinsicTypeRepresentation::Array { element }) => {
            Some((ArrayKind::Immutable, element))
        }
        ClassRepresentation::Intrinsic(IntrinsicTypeRepresentation::MutableArray { element }) => {
            Some((ArrayKind::Mutable, element))
        }
        ClassRepresentation::Declared { .. }
        | ClassRepresentation::Intrinsic(IntrinsicTypeRepresentation::String) => None,
        ClassRepresentation::Intrinsic(_) => {
            unreachable!("the intrinsic registry fixes declaration targets")
        }
    }
}

#[derive(Debug)]
pub enum TableSlot {
    Function(FunctionId),
    Runtime(RuntimeFn),
}

#[derive(Debug)]
pub struct ItableRecord {
    pub interface: InterfaceId,
    pub slots: Vec<TableSlot>,
}

#[derive(Debug)]
pub struct InterfaceDef {
    pub link_stem: NominalLinkStem,
    pub name: String,
    /// Canonical arguments of this fully specialized application.
    pub type_arguments: Vec<Type>,
    /// Signature-only method declarations in itable-slot order.
    pub methods: Vec<FunctionId>,
}

#[derive(Debug)]
pub struct Local {
    pub name: String,
    pub ty: Type,
    pub mutable: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_source_integer_kinds_have_exact_names_widths_and_compact_v2_codes() {
        let expected = [
            (
                IntegerKind::SIGNED_8,
                IntegerSignedness::Signed,
                IntegerWidth::W8,
                "Int8",
                "I8",
            ),
            (
                IntegerKind::SIGNED_16,
                IntegerSignedness::Signed,
                IntegerWidth::W16,
                "Int16",
                "I16",
            ),
            (
                IntegerKind::SIGNED_32,
                IntegerSignedness::Signed,
                IntegerWidth::W32,
                "Int",
                "I32",
            ),
            (
                IntegerKind::SIGNED_64,
                IntegerSignedness::Signed,
                IntegerWidth::W64,
                "Long",
                "I64",
            ),
            (
                IntegerKind::UNSIGNED_8,
                IntegerSignedness::Unsigned,
                IntegerWidth::W8,
                "UInt8",
                "V8",
            ),
            (
                IntegerKind::UNSIGNED_16,
                IntegerSignedness::Unsigned,
                IntegerWidth::W16,
                "UInt16",
                "V16",
            ),
            (
                IntegerKind::UNSIGNED_32,
                IntegerSignedness::Unsigned,
                IntegerWidth::W32,
                "UInt",
                "V32",
            ),
            (
                IntegerKind::UNSIGNED_64,
                IntegerSignedness::Unsigned,
                IntegerWidth::W64,
                "ULong",
                "V64",
            ),
        ];

        assert_eq!(IntegerKind::ALL.len(), expected.len());
        for (index, (kind, signedness, width, name, code)) in expected.into_iter().enumerate() {
            assert_eq!(IntegerKind::ALL[index], kind);
            assert_eq!(kind.signedness(), signedness);
            assert_eq!(kind.width(), width);
            assert_eq!(kind.canonical_name(), name);
            assert_eq!(kind.compact_v2_code(), code);
            assert_eq!(width.bytes() * 8, width.bits());
        }
    }

    #[test]
    fn integer_constants_derive_every_property_from_the_exact_variant() {
        let constants = [
            (
                MirIntegerConstant::Signed8(0x80),
                IntegerKind::SIGNED_8,
                -128,
            ),
            (
                MirIntegerConstant::Signed16(0x8000),
                IntegerKind::SIGNED_16,
                -32_768,
            ),
            (
                MirIntegerConstant::Signed32(0x8000_0000),
                IntegerKind::SIGNED_32,
                -2_147_483_648,
            ),
            (
                MirIntegerConstant::Signed64(0x8000_0000_0000_0000),
                IntegerKind::SIGNED_64,
                -9_223_372_036_854_775_808,
            ),
            (
                MirIntegerConstant::Unsigned8(u8::MAX),
                IntegerKind::UNSIGNED_8,
                u8::MAX as i128,
            ),
            (
                MirIntegerConstant::Unsigned16(u16::MAX),
                IntegerKind::UNSIGNED_16,
                u16::MAX as i128,
            ),
            (
                MirIntegerConstant::Unsigned32(u32::MAX),
                IntegerKind::UNSIGNED_32,
                u32::MAX as i128,
            ),
            (
                MirIntegerConstant::Unsigned64(u64::MAX),
                IntegerKind::UNSIGNED_64,
                u64::MAX as i128,
            ),
        ];

        for (constant, kind, mathematical_value) in constants {
            assert_eq!(constant.kind(), kind);
            assert_eq!(constant.signedness(), kind.signedness());
            assert_eq!(constant.width(), kind.width());
            assert_eq!(constant.mathematical_value(), mathematical_value);
            assert_eq!(
                MirIntegerConstant::from_raw_bits(kind, constant.raw_bits()),
                Some(constant)
            );
            assert_eq!(Expr::integer(constant).ty, Type::Integer(kind));
        }

        assert_eq!(
            MirIntegerConstant::from_raw_bits(IntegerKind::SIGNED_8, 0x100),
            None
        );
        assert_eq!(
            MirIntegerConstant::from_raw_bits(IntegerKind::UNSIGNED_32, 1_u64 << 32),
            None
        );
    }

    #[test]
    fn static_and_annotation_integer_zero_remain_exactly_typed() {
        let zero = MirIntegerConstant::Unsigned16(0);
        let encoded = MirStaticInitialState::EncodedStaticValue {
            payload: MirConstantImage::Integer(zero),
        };
        assert_ne!(encoded, MirStaticInitialState::ZeroedForRuntimeUnit);
        assert_eq!(
            MirAnnotationValue::Integer(zero),
            MirAnnotationValue::Integer(MirIntegerConstant::Unsigned16(0))
        );
        assert_ne!(
            MirAnnotationValue::Integer(zero),
            MirAnnotationValue::Integer(MirIntegerConstant::Signed16(0))
        );
    }

    #[test]
    fn c_layout_annotation_contract_has_only_qualified_alignments() {
        let values = [
            MirCLayoutValue::Natural,
            MirCLayoutValue::A1,
            MirCLayoutValue::A2,
            MirCLayoutValue::A4,
            MirCLayoutValue::A8,
            MirCLayoutValue::A16,
        ];
        assert_eq!(
            values.map(MirCLayoutValue::bytes),
            [None, Some(1), Some(2), Some(4), Some(8), Some(16)]
        );
    }

    #[test]
    fn suspend_state_ids_are_nonzero_by_construction() {
        assert_eq!(CoroutineSuspendStateId::new(0), None);
        assert_eq!(
            CoroutineSuspendStateId::new(1).map(CoroutineSuspendStateId::get),
            Some(1)
        );
        assert_eq!(
            CoroutineSuspendStateId::new(u32::MAX).map(CoroutineSuspendStateId::get),
            Some(u32::MAX)
        );
    }

    #[test]
    fn coroutine_frame_state_encodings_do_not_overlap() {
        let first = CoroutineSuspendStateId::new(1).expect("one is nonzero");
        let last = CoroutineSuspendStateId::new(u32::MAX).expect("u32::MAX is nonzero");
        let initial =
            MachineScalarValue::CoroutineFrameState(CoroutineFrameState::Initial).raw_bits();
        let running =
            MachineScalarValue::CoroutineFrameState(CoroutineFrameState::Running).raw_bits();
        let completed =
            MachineScalarValue::CoroutineFrameState(CoroutineFrameState::Completed).raw_bits();
        let first_suspended =
            MachineScalarValue::CoroutineFrameState(CoroutineFrameState::Suspended(first))
                .raw_bits();
        let last_suspended =
            MachineScalarValue::CoroutineFrameState(CoroutineFrameState::Suspended(last))
                .raw_bits();
        let first_failure =
            MachineScalarValue::CoroutineFrameState(CoroutineFrameState::ResumeFailure(first))
                .raw_bits();
        let last_failure =
            MachineScalarValue::CoroutineFrameState(CoroutineFrameState::ResumeFailure(last))
                .raw_bits();

        assert_eq!(initial, 0);
        assert_eq!(first_suspended, 1);
        assert_eq!(last_suspended, u64::from(u32::MAX));
        assert!(last_suspended < last_failure);
        assert!(last_failure <= first_failure);
        assert!(first_failure < completed);
        assert!(completed < running);
    }

    #[test]
    fn machine_scalar_values_have_one_total_semantic_kind() {
        let site = CoroutineSuspendStateId::new(1).expect("one is nonzero");
        let cases = [
            (MachineScalarValue::EnumTag(7), MachineScalarKind::EnumTag),
            (
                MachineScalarValue::InitializationOutcome(InitializationOutcome::Ready),
                MachineScalarKind::InitializationOutcome,
            ),
            (
                MachineScalarValue::CoroutineFrameState(CoroutineFrameState::Suspended(site)),
                MachineScalarKind::CoroutineFrameState,
            ),
            (
                MachineScalarValue::CoroutineAdapterState(CoroutineAdapterState::Waiting),
                MachineScalarKind::CoroutineAdapterState,
            ),
            (
                MachineScalarValue::ForeignCallbackStatus(ForeignCallbackStatus::Returned),
                MachineScalarKind::ForeignCallbackStatus,
            ),
            (
                MachineScalarValue::PointerElementOffset(9),
                MachineScalarKind::PointerElementOffset,
            ),
        ];

        for (value, expected) in cases {
            assert_eq!(value.kind(), expected);
            assert_eq!(
                Expr::machine_scalar(value).ty,
                Type::MachineScalar(expected)
            );
        }
        assert_eq!(
            Expr::enum_tag(Expr::unit()).ty,
            Type::MachineScalar(MachineScalarKind::EnumTag)
        );
    }

    #[test]
    #[should_panic(expected = "machine scalar equality operands have the same semantic kind")]
    fn machine_scalar_equality_rejects_mismatched_kinds() {
        let _ = Expr::machine_eq(
            Expr::machine_scalar(MachineScalarValue::EnumTag(0)),
            MachineScalarValue::ForeignCallbackStatus(ForeignCallbackStatus::Returned),
        );
    }
}
