//! Normalize the language's C projections from the actual typed declarations.

use super::{HirNativeBoundaryTypeDefinitionError as Error, HirNativeBoundaryTypeDefinitionInputs};
use crate::{
    CoreProtocols, NativeBoundaryCAbiV1 as CAbi, NativeBoundaryNominalOwner as Owner,
    NativeBoundaryNominalShape as Shape, NativeBoundaryTypeDefinitionRecord as Record,
};

pub(super) fn project(
    record: Record,
    inputs: &HirNativeBoundaryTypeDefinitionInputs<'_>,
) -> Result<Record, Error> {
    let Owner::GenericTemplate(owner) = record.owner() else {
        return Ok(record);
    };
    let (scalars, option, none, payload) = match inputs.protocols {
        CoreProtocols::Defined(protocols) => {
            let generic = |id: crate::StructId| {
                inputs.nominal_identities[id]
                    .generic_type_id()
                    .ok_or(Error::UnexpectedGeneratedNominal)
            };
            (
                [
                    generic(protocols.ffi.pinned_ptr)?,
                    generic(protocols.ffi.gc_handle)?,
                ],
                inputs.nominal_identities[protocols.option.enumeration()]
                    .generic_type_id()
                    .ok_or(Error::UnexpectedGeneratedNominal)?,
                inputs.enum_member_identities[protocols.option.none()].id(),
                inputs.enum_member_identities[protocols.option.some_payload()].id(),
            )
        }
        CoreProtocols::Imported(protocols) => (
            [
                protocols.ffi().pinned_ptr().persistent(),
                protocols.ffi().gc_handle().persistent(),
            ],
            protocols.option().option().persistent(),
            protocols.option().none().persistent(),
            protocols.option().some_payload().persistent(),
        ),
    };
    let projection = if scalars.contains(&owner) {
        let Shape::Struct { fields, .. } = record.shape() else {
            return Err(invalid_projection());
        };
        let [field] = fields.as_slice() else {
            return Err(invalid_projection());
        };
        CAbi::UInt64Field {
            field: field.field(),
        }
    } else if owner == option {
        CAbi::NullablePointer { none, payload }
    } else {
        return Ok(record);
    };
    if record.c_abi() != CAbi::SourceRepresentation && record.c_abi() != projection {
        return Err(Error::DependencyDefinitionMismatch {
            owner: record.owner(),
        });
    }
    record
        .with_c_abi(projection)
        .map_err(Error::InvalidDefinition)
}

fn invalid_projection() -> Error {
    Error::InvalidDefinition(crate::NativeBoundaryDefinitionError::CAbiProjectionMismatch)
}
