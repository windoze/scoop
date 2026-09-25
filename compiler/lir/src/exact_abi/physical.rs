use scoop_identity::ScoopAbiReturn as CanonicalReturn;

use super::*;
use crate::{
    AbiReturn, EnumDefs, GcEffect as PhysicalGcEffect, LirType, RefScan, ScoopAbiSignature,
};

impl ExactCallableAbiExportV1 {
    /// Joins the replayed record to an emitted function's sole physical
    /// signature. The containing producer separately binds its body identity.
    pub fn validate_physical_signature(
        &self,
        enums: &EnumDefs,
        physical: &ScoopAbiSignature,
        effect: PhysicalGcEffect,
    ) -> Result<(), ExactCallablePhysicalAbiError> {
        let effect = match effect {
            PhysicalGcEffect::Managed => GcEffect::Managed,
            PhysicalGcEffect::NoGc => GcEffect::NoGc,
        };
        if physical.calling_convention() != self.calling_convention()
            || effect != self.call_protocol().gc_effect()
        {
            return Err(ExactCallablePhysicalAbiError::Protocol);
        }
        crate::external_callable_abi::validate_canonical_scoop_abi(
            self.canonical_signature(),
            physical,
        )
        .map_err(|error| match error {
            crate::external_callable_abi::CanonicalScoopAbiMismatch::ArgumentCount { .. } => {
                ExactCallablePhysicalAbiError::ArgumentCount
            }
            crate::external_callable_abi::CanonicalScoopAbiMismatch::Argument { index } => {
                ExactCallablePhysicalAbiError::Argument(index)
            }
            crate::external_callable_abi::CanonicalScoopAbiMismatch::Result => {
                ExactCallablePhysicalAbiError::Result
            }
        })?;
        let layouts = self.layout_dependencies();
        let values = layouts
            .receiver()
            .value()
            .into_iter()
            .chain(layouts.parameters().iter().map(AsRef::as_ref));
        for (index, ((actual, canonical), value)) in physical
            .arguments()
            .iter()
            .zip(self.canonical_signature().arguments())
            .zip(values)
            .enumerate()
        {
            if !value_matches(
                enums,
                canonical.storage().shape(),
                value,
                actual.logical_storage_type(),
                actual.scan(),
            )? {
                return Err(ExactCallablePhysicalAbiError::Argument(index));
            }
        }
        match (self.canonical_signature().result(), physical.result()) {
            (CanonicalReturn::UnitVoid, AbiReturn::UnitVoid) => {}
            (
                CanonicalReturn::ElidedZst(storage)
                | CanonicalReturn::Direct(storage)
                | CanonicalReturn::Indirect(storage),
                result,
            ) => {
                let (ty, scan) = match result {
                    AbiReturn::ElidedZst(value) => (value.storage_type(), value.scan()),
                    AbiReturn::Direct(value) | AbiReturn::Indirect(value) => {
                        (value.storage_type(), value.scan())
                    }
                    AbiReturn::UnitVoid => return Err(ExactCallablePhysicalAbiError::Result),
                };
                if !value_matches(enums, storage.shape(), layouts.result(), ty, scan)? {
                    return Err(ExactCallablePhysicalAbiError::Result);
                }
            }
            _ => return Err(ExactCallablePhysicalAbiError::Result),
        }
        Ok(())
    }
}

fn value_matches(
    enums: &EnumDefs,
    shape: scoop_identity::ScoopAbiValueShape,
    value: &ExactValueLayoutV1,
    physical_type: &LirType,
    scan: &RefScan,
) -> Result<bool, ExactCallablePhysicalAbiError> {
    let actual_shape = crate::scoop_abi_value_shape(enums, physical_type)
        .map_err(ExactCallablePhysicalAbiError::Classification)?;
    let shape_matches = matches!(
        (shape, actual_shape),
        (
            scoop_identity::ScoopAbiValueShape::Scalar,
            crate::ScoopAbiValueShape::Scalar
        ) | (
            scoop_identity::ScoopAbiValueShape::Aggregate,
            crate::ScoopAbiValueShape::Aggregate
        )
    );
    if !shape_matches || !representation_matches(value, physical_type, enums) {
        return Ok(false);
    }
    let expected = match value.value().storage().kind() {
        crate::ValueStorageKindV1::ZeroSized { .. } => {
            return Ok(scan == &RefScan::None);
        }
        crate::ValueStorageKindV1::Inline { scan, .. } => scan,
    };
    // Equality stops at the first different shape or sequence length. Its
    // work is bounded by the complete expected tree, including shared paths.
    Ok(scan == expected.as_ref_scan())
}

fn representation_matches(value: &ExactValueLayoutV1, ty: &LirType, enums: &EnumDefs) -> bool {
    use crate::ExactRepresentationKindV1 as Kind;
    match value.representation().kind() {
        Kind::Scalar(crate::ScalarRepresentationKindV1::Integer(kind)) => ty == &kind.scalar_type(),
        Kind::Scalar(crate::ScalarRepresentationKindV1::Boolean) => ty == &LirType::I1,
        Kind::QualifiedPointer(kind) => ty == &LirType::Ptr(kind.pointer_kind()),
        Kind::Struct(_) => matches!(ty, LirType::Struct(_)),
        Kind::Tuple(_) => matches!(ty, LirType::Aggregate(_)),
        Kind::IntrinsicValue(crate::IntrinsicValueFamilyV1::Unit) => {
            matches!(ty, LirType::Aggregate(fields) if fields.is_empty())
        }
        Kind::NicheEnum(expected) => matches!(
            ty,
            LirType::Enum(id) if enums.get(*id).is_some_and(|actual| {
                matches!(actual.repr, crate::EnumRepr::Niche { kind, .. } if kind == expected.pointer_kind())
            })
        ),
        Kind::TaggedEnum(_) => matches!(
            ty,
            LirType::Enum(id) if enums.get(*id).is_some_and(|actual| {
                matches!(actual.repr, crate::EnumRepr::Tagged { .. })
            })
        ),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExactCallablePhysicalAbiError {
    ArgumentCount,
    Argument(usize),
    Result,
    Protocol,
    Classification(crate::ScoopAbiClassificationError),
    Resource(WireError),
}
impl From<WireError> for ExactCallablePhysicalAbiError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl std::fmt::Display for ExactCallablePhysicalAbiError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "physical callable ABI differs from checked layouts: {self:?}"
        )
    }
}
impl std::error::Error for ExactCallablePhysicalAbiError {}
