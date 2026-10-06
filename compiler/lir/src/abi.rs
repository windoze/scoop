use std::num::NonZeroU64;

use super::{
    CallingConvention, EnumDefId, EnumDefs, EnumRepr, LirTargetProfile, LirType, LocalId, RefScan,
    ScoopAbiPassing, ScoopAbiValueShape, Value,
};

static EMPTY_REF_SCAN: RefScan = RefScan::None;

/// Failure to construct one of the refined Scoop ABI storage layouts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbiLayoutError {
    ZeroSize,
    ZeroAlignment,
    AlignmentNotPowerOfTwo(u64),
}

/// Failure to bind an exact LIR storage type to an ABI value or address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbiValueError {
    VoidStorageType,
    StorageTypeMismatch { expected: LirType, actual: LirType },
}

/// Failure to derive the physical Scoop ABI shape of a stored LIR value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScoopAbiClassificationError {
    VoidStorageType,
    InvalidEnumDefinition(EnumDefId),
}

/// Classifies one non-zero logical LIR value with the module's target profile.
///
/// Layout lowering handles zero-sized values before calling this function.
/// Keeping the exact type-to-shape mapping here gives MIR -> LIR lowering and
/// codegen boundary validation one authority while the target profile owns the
/// final shape-to-passing decision.
pub fn classify_non_zero_scoop_abi_value(
    profile: LirTargetProfile,
    enums: &EnumDefs,
    ty: &LirType,
) -> Result<ScoopAbiPassing, ScoopAbiClassificationError> {
    Ok(profile.classify_scoop_abi_value(scoop_abi_value_shape(enums, ty)?))
}

/// Return the target-independent scalar/aggregate shape consumed by the
/// closed Scoop ABI classifier. Zero-sized values use the same shape even
/// though their physical parameter is elided.
pub fn scoop_abi_value_shape(
    enums: &EnumDefs,
    ty: &LirType,
) -> Result<ScoopAbiValueShape, ScoopAbiClassificationError> {
    let shape = match ty {
        LirType::I1
        | LirType::I8
        | LirType::I16
        | LirType::I32
        | LirType::F32
        | LirType::F64
        | LirType::I64
        | LirType::MachineScalar(_)
        | LirType::Ptr(_) => ScoopAbiValueShape::Scalar,
        LirType::Enum(id) => {
            let definition = enums
                .get(*id)
                .ok_or(ScoopAbiClassificationError::InvalidEnumDefinition(*id))?;
            if matches!(definition.repr, EnumRepr::Niche { .. }) {
                ScoopAbiValueShape::Scalar
            } else {
                ScoopAbiValueShape::Aggregate
            }
        }
        LirType::Aggregate(_) | LirType::Struct(_) | LirType::ExceptionRecord => {
            ScoopAbiValueShape::Aggregate
        }
        LirType::Void => return Err(ScoopAbiClassificationError::VoidStorageType),
    };
    Ok(shape)
}

fn checked_alignment(alignment: u64) -> Result<NonZeroU64, AbiLayoutError> {
    let alignment = NonZeroU64::new(alignment).ok_or(AbiLayoutError::ZeroAlignment)?;
    if alignment.get().is_power_of_two() {
        Ok(alignment)
    } else {
        Err(AbiLayoutError::AlignmentNotPowerOfTwo(alignment.get()))
    }
}

/// Checked layout of an exact zero-sized Scoop value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbiZeroSizedLayout {
    alignment: NonZeroU64,
}

impl AbiZeroSizedLayout {
    pub fn new(alignment: u64) -> Result<Self, AbiLayoutError> {
        Ok(Self {
            alignment: checked_alignment(alignment)?,
        })
    }

    pub const fn size(self) -> u64 {
        0
    }

    pub const fn alignment(self) -> NonZeroU64 {
        self.alignment
    }
}

/// Checked layout of an exact non-zero-sized Scoop value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbiNonZeroLayout {
    size: NonZeroU64,
    alignment: NonZeroU64,
}

impl AbiNonZeroLayout {
    pub fn new(size: u64, alignment: u64) -> Result<Self, AbiLayoutError> {
        Ok(Self {
            size: NonZeroU64::new(size).ok_or(AbiLayoutError::ZeroSize)?,
            alignment: checked_alignment(alignment)?,
        })
    }

    pub const fn size(self) -> NonZeroU64 {
        self.size
    }

    pub const fn alignment(self) -> NonZeroU64 {
        self.alignment
    }
}

/// A logical Scoop value that occupies no physical ABI storage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbiZst {
    storage_type: LirType,
    layout: AbiZeroSizedLayout,
}

impl AbiZst {
    pub fn new(storage_type: LirType, layout: AbiZeroSizedLayout) -> Result<Self, AbiValueError> {
        if storage_type == LirType::Void {
            return Err(AbiValueError::VoidStorageType);
        }
        Ok(Self {
            storage_type,
            layout,
        })
    }

    pub const fn storage_type(&self) -> &LirType {
        &self.storage_type
    }

    pub const fn layout(&self) -> AbiZeroSizedLayout {
        self.layout
    }

    pub fn scan(&self) -> &'static RefScan {
        &EMPTY_REF_SCAN
    }
}

/// An exact, non-zero-sized Scoop value together with its complete root scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbiValue {
    storage_type: LirType,
    layout: AbiNonZeroLayout,
    scan: RefScan,
}

impl AbiValue {
    pub fn new(
        storage_type: LirType,
        layout: AbiNonZeroLayout,
        scan: RefScan,
    ) -> Result<Self, AbiValueError> {
        if storage_type == LirType::Void {
            return Err(AbiValueError::VoidStorageType);
        }
        Ok(Self {
            storage_type,
            layout,
            scan,
        })
    }

    pub const fn storage_type(&self) -> &LirType {
        &self.storage_type
    }

    pub const fn layout(&self) -> AbiNonZeroLayout {
        self.layout
    }

    pub const fn scan(&self) -> &RefScan {
        &self.scan
    }
}

/// Exact typed local storage used to pass one indirect Scoop ABI argument.
///
/// Construction checks the local's declared storage type against the ABI
/// value whose address the call signature requires. The private local id
/// prevents later stages from pairing an unrelated slot with that signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AbiArgumentStorage(LocalId);

impl AbiArgumentStorage {
    pub fn new(
        local: LocalId,
        local_storage_type: &LirType,
        expected: &AbiValue,
    ) -> Result<Self, AbiValueError> {
        if local_storage_type == expected.storage_type() {
            Ok(Self(local))
        } else {
            Err(AbiValueError::StorageTypeMismatch {
                expected: expected.storage_type().clone(),
                actual: local_storage_type.clone(),
            })
        }
    }

    pub const fn local(self) -> LocalId {
        self.0
    }
}

/// One logical call argument together with its selected physical convention.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AbiCallArgument {
    ElidedZst(Value),
    Direct(Value),
    Indirect(AbiArgumentStorage),
}

impl AbiCallArgument {
    pub const fn logical_value(self) -> Value {
        match self {
            Self::ElidedZst(value) | Self::Direct(value) => value,
            Self::Indirect(storage) => Value::Local(storage.local()),
        }
    }
}

/// Physical passing convention selected for one logical Scoop argument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbiArgument {
    ElidedZst(AbiZst),
    Direct(AbiValue),
    Indirect(AbiValue),
}

impl AbiArgument {
    pub const fn logical_storage_type(&self) -> &LirType {
        match self {
            Self::ElidedZst(value) => value.storage_type(),
            Self::Direct(value) | Self::Indirect(value) => value.storage_type(),
        }
    }

    pub fn scan(&self) -> &RefScan {
        match self {
            Self::ElidedZst(value) => value.scan(),
            Self::Direct(value) | Self::Indirect(value) => value.scan(),
        }
    }

    pub const fn physical_value(&self) -> Option<&AbiValue> {
        match self {
            Self::ElidedZst(_) => None,
            Self::Direct(value) | Self::Indirect(value) => Some(value),
        }
    }

    pub const fn is_elided(&self) -> bool {
        matches!(self, Self::ElidedZst(_))
    }

    pub const fn is_indirect(&self) -> bool {
        matches!(self, Self::Indirect(_))
    }
}

/// Physical return convention selected for one logical Scoop result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbiReturn {
    UnitVoid,
    ElidedZst(AbiZst),
    Direct(AbiValue),
    Indirect(AbiValue),
}

impl AbiReturn {
    pub const fn logical_storage_type(&self) -> Option<&LirType> {
        match self {
            Self::UnitVoid => None,
            Self::ElidedZst(value) => Some(value.storage_type()),
            Self::Direct(value) | Self::Indirect(value) => Some(value.storage_type()),
        }
    }

    pub fn scan(&self) -> Option<&RefScan> {
        match self {
            Self::UnitVoid => None,
            Self::ElidedZst(value) => Some(value.scan()),
            Self::Direct(value) | Self::Indirect(value) => Some(value.scan()),
        }
    }

    pub const fn physical_value(&self) -> Option<&AbiValue> {
        match self {
            Self::Direct(value) | Self::Indirect(value) => Some(value),
            Self::UnitVoid | Self::ElidedZst(_) => None,
        }
    }

    pub const fn is_indirect(&self) -> bool {
        matches!(self, Self::Indirect(_))
    }
}

/// Origin and passing convention of one physical Scoop ABI parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbiPhysicalParameterOrigin {
    IndirectReturn,
    DirectArgument { logical_index: usize },
    IndirectArgument { logical_index: usize },
}

impl AbiPhysicalParameterOrigin {
    pub const fn logical_argument_index(self) -> Option<usize> {
        match self {
            Self::IndirectReturn => None,
            Self::DirectArgument { logical_index } | Self::IndirectArgument { logical_index } => {
                Some(logical_index)
            }
        }
    }
}

/// One computed entry in the physical parameter sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbiPhysicalParameter<'a> {
    index: usize,
    origin: AbiPhysicalParameterOrigin,
    value: &'a AbiValue,
}

impl<'a> AbiPhysicalParameter<'a> {
    pub const fn index(self) -> usize {
        self.index
    }

    pub const fn origin(self) -> AbiPhysicalParameterOrigin {
        self.origin
    }

    pub const fn value(self) -> &'a AbiValue {
        self.value
    }
}

/// Computed physical location of one logical argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbiArgumentLocation {
    Elided,
    Parameter(usize),
}

/// One Scoop calling signature with arguments retained in logical order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScoopAbiSignature {
    arguments: Vec<AbiArgument>,
    result: AbiReturn,
    calling_convention: CallingConvention,
}

impl ScoopAbiSignature {
    pub const fn new(
        arguments: Vec<AbiArgument>,
        result: AbiReturn,
        calling_convention: CallingConvention,
    ) -> Self {
        Self {
            arguments,
            result,
            calling_convention,
        }
    }

    pub fn arguments(&self) -> &[AbiArgument] {
        &self.arguments
    }

    pub const fn result(&self) -> &AbiReturn {
        &self.result
    }

    pub const fn calling_convention(&self) -> CallingConvention {
        self.calling_convention
    }

    pub fn logical_argument_count(&self) -> usize {
        self.arguments.len()
    }

    pub fn physical_parameter_count(&self) -> usize {
        usize::from(self.result.is_indirect())
            + self
                .arguments
                .iter()
                .filter(|argument| !argument.is_elided())
                .count()
    }

    pub fn argument_location(&self, logical_index: usize) -> Option<AbiArgumentLocation> {
        let argument = self.arguments.get(logical_index)?;
        if argument.is_elided() {
            return Some(AbiArgumentLocation::Elided);
        }

        let return_offset = usize::from(self.result.is_indirect());
        let preceding_parameters = self.arguments[..logical_index]
            .iter()
            .filter(|argument| !argument.is_elided())
            .count();
        Some(AbiArgumentLocation::Parameter(
            return_offset + preceding_parameters,
        ))
    }

    pub fn physical_parameters(&self) -> impl Iterator<Item = AbiPhysicalParameter<'_>> + '_ {
        let return_parameter = match &self.result {
            AbiReturn::Indirect(value) => Some((AbiPhysicalParameterOrigin::IndirectReturn, value)),
            AbiReturn::UnitVoid | AbiReturn::ElidedZst(_) | AbiReturn::Direct(_) => None,
        };
        let argument_parameters = self.arguments.iter().enumerate().filter_map(
            |(logical_index, argument)| match argument {
                AbiArgument::ElidedZst(_) => None,
                AbiArgument::Direct(value) => Some((
                    AbiPhysicalParameterOrigin::DirectArgument { logical_index },
                    value,
                )),
                AbiArgument::Indirect(value) => Some((
                    AbiPhysicalParameterOrigin::IndirectArgument { logical_index },
                    value,
                )),
            },
        );

        return_parameter
            .into_iter()
            .chain(argument_parameters)
            .enumerate()
            .map(|(index, (origin, value))| AbiPhysicalParameter {
                index,
                origin,
                value,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn non_zero_value(ty: LirType, size: u64, scan: RefScan) -> AbiValue {
        AbiValue::new(
            ty,
            AbiNonZeroLayout::new(size, 8).expect("test layout should be valid"),
            scan,
        )
        .expect("test ABI value should be valid")
    }

    fn zst(ty: LirType) -> AbiZst {
        AbiZst::new(
            ty,
            AbiZeroSizedLayout::new(1).expect("test layout should be valid"),
        )
        .expect("test ABI ZST should be valid")
    }

    #[test]
    fn layouts_reject_invalid_size_and_alignment() {
        assert_eq!(
            AbiZeroSizedLayout::new(0),
            Err(AbiLayoutError::ZeroAlignment)
        );
        assert_eq!(
            AbiZeroSizedLayout::new(3),
            Err(AbiLayoutError::AlignmentNotPowerOfTwo(3))
        );
        assert_eq!(AbiNonZeroLayout::new(0, 8), Err(AbiLayoutError::ZeroSize));
        assert_eq!(
            AbiNonZeroLayout::new(16, 6),
            Err(AbiLayoutError::AlignmentNotPowerOfTwo(6))
        );
    }

    #[test]
    fn layouts_expose_only_refined_sizes() {
        let zero = AbiZeroSizedLayout::new(16).expect("zero-sized layout should be valid");
        assert_eq!(zero.size(), 0);
        assert_eq!(zero.alignment().get(), 16);

        let non_zero = AbiNonZeroLayout::new(24, 8).expect("non-zero-sized layout should be valid");
        assert_eq!(non_zero.size().get(), 24);
        assert_eq!(non_zero.alignment().get(), 8);
    }

    #[test]
    fn abi_values_reject_void_storage() {
        let zero_layout = AbiZeroSizedLayout::new(1).expect("test layout should be valid");
        assert_eq!(
            AbiZst::new(LirType::Void, zero_layout),
            Err(AbiValueError::VoidStorageType)
        );

        let non_zero_layout = AbiNonZeroLayout::new(8, 8).expect("test layout should be valid");
        assert_eq!(
            AbiValue::new(LirType::Void, non_zero_layout, RefScan::None),
            Err(AbiValueError::VoidStorageType)
        );
    }

    #[test]
    fn indirect_call_argument_requires_exact_local_storage_type() {
        let mut locals = la_arena::Arena::new();
        let expected = non_zero_value(
            LirType::Aggregate(vec![LirType::I64, LirType::I64]),
            16,
            RefScan::None,
        );
        let local = locals.alloc(super::super::Local::new(
            "argument",
            super::super::LocalStorage::NonZero(expected.clone()),
        ));

        let storage = AbiArgumentStorage::new(local, locals[local].ty(), &expected)
            .expect("exact local storage should be accepted");
        assert_eq!(storage.local(), local);
        assert_eq!(
            AbiCallArgument::Indirect(storage).logical_value(),
            Value::Local(local)
        );

        assert_eq!(
            AbiArgumentStorage::new(local, &LirType::I64, &expected),
            Err(AbiValueError::StorageTypeMismatch {
                expected: expected.storage_type().clone(),
                actual: LirType::I64,
            })
        );
    }

    #[test]
    fn direct_and_elided_call_arguments_preserve_their_logical_value() {
        let direct = Value::IntegerConst(super::super::LirIntegerConstant::Signed64(42));
        let elided = Value::BoolConst(false);

        assert_eq!(AbiCallArgument::Direct(direct).logical_value(), direct);
        assert_eq!(AbiCallArgument::ElidedZst(elided).logical_value(), elided);
    }

    #[test]
    fn signature_computes_physical_parameter_order_without_storing_it() {
        let signature = ScoopAbiSignature::new(
            vec![
                AbiArgument::ElidedZst(zst(LirType::Aggregate(Vec::new()))),
                AbiArgument::Direct(non_zero_value(LirType::I64, 8, RefScan::None)),
                AbiArgument::Indirect(non_zero_value(
                    LirType::Aggregate(vec![LirType::Ptr(super::super::PointerKind::Managed)]),
                    8,
                    RefScan::References(vec![0]),
                )),
            ],
            AbiReturn::Indirect(non_zero_value(
                LirType::Aggregate(vec![LirType::I64, LirType::I64]),
                16,
                RefScan::None,
            )),
            CallingConvention::Cdecl,
        );

        assert_eq!(signature.logical_argument_count(), 3);
        assert_eq!(signature.physical_parameter_count(), 3);
        assert_eq!(
            signature.argument_location(0),
            Some(AbiArgumentLocation::Elided)
        );
        assert_eq!(
            signature.argument_location(1),
            Some(AbiArgumentLocation::Parameter(1))
        );
        assert_eq!(
            signature.argument_location(2),
            Some(AbiArgumentLocation::Parameter(2))
        );
        assert_eq!(signature.argument_location(3), None);

        let parameters = signature.physical_parameters().collect::<Vec<_>>();
        assert_eq!(parameters.len(), 3);
        assert_eq!(parameters[0].index(), 0);
        assert_eq!(
            parameters[0].origin(),
            AbiPhysicalParameterOrigin::IndirectReturn
        );
        assert_eq!(
            parameters[1].origin(),
            AbiPhysicalParameterOrigin::DirectArgument { logical_index: 1 }
        );
        assert_eq!(
            parameters[2].origin(),
            AbiPhysicalParameterOrigin::IndirectArgument { logical_index: 2 }
        );
    }

    #[test]
    fn unit_and_zst_results_have_distinct_logical_storage() {
        let unit = AbiReturn::UnitVoid;
        assert_eq!(unit.logical_storage_type(), None);
        assert_eq!(unit.scan(), None);

        let zst = AbiReturn::ElidedZst(zst(LirType::Aggregate(Vec::new())));
        assert_eq!(
            zst.logical_storage_type(),
            Some(&LirType::Aggregate(Vec::new()))
        );
        assert_eq!(zst.scan(), Some(&RefScan::None));
    }
}
