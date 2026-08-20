use scoop_wire::{WirePath, encode_canonical_temporary};

use super::*;
use crate::shape_link::ShapeLinkError;

impl DecodedShapeLinkContractV1 {
    pub fn validate_against(self, expected: &ShapeLinkContractV1) -> Result<(), ShapeLinkError> {
        let path = WirePath::root();

        match (self, expected) {
            (
                Self::CallableAbi {
                    canonical_signature,
                    calling_convention,
                    protocol,
                },
                ShapeLinkContractV1::CallableAbi {
                    canonical_signature: expected_signature,
                    calling_convention: expected_cc,
                    protocol: expected_protocol,
                },
            ) => {
                if calling_convention != *expected_cc || protocol != *expected_protocol {
                    return Err(ShapeLinkError::Contract);
                }

                let actual = encode_canonical_temporary(&canonical_signature, &path)?;
                let wanted = encode_canonical_temporary(expected_signature, &path)?;

                if actual != wanted {
                    return Err(ShapeLinkError::Contract);
                }
            }
            (Self::Layout(actual), ShapeLinkContractV1::Layout { record }) => {
                actual.validate_against(record)?
            }
            (
                Self::Scan {
                    layout,
                    role,
                    canonical_scan,
                },
                ShapeLinkContractV1::Scan {
                    layout: expected_layout,
                    role: expected_role,
                    canonical_scan: expected_scan,
                },
            ) => {
                if layout.verify(*expected_layout).is_err() || role != *expected_role {
                    return Err(ShapeLinkError::Contract);
                }
                let checked = canonical_scan.validate()?;
                if checked.as_ref_scan() != expected_scan {
                    return Err(ShapeLinkError::Contract);
                }
            }
            (
                Self::Type(actual),
                ShapeLinkContractV1::Type {
                    descriptor_projection,
                },
            ) => actual.validate_against(descriptor_projection)?,
            (Self::Dispatch(actual), ShapeLinkContractV1::Dispatch { table_projection }) => {
                actual.validate_against(table_projection)?
            }
            (
                Self::StaticStorage(actual),
                ShapeLinkContractV1::StaticStorage { storage_projection },
            ) => actual.validate_against(storage_projection)?,
            (
                Self::Initialization(actual),
                ShapeLinkContractV1::Initialization { unit_projection },
            ) => actual.validate_against(unit_projection)?,
            _ => return Err(ShapeLinkError::Contract),
        }
        Ok(())
    }
}
