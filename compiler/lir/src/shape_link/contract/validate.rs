use scoop_wire::{BudgetMeter, WirePath, encode_canonical_temporary_with_meter};

use super::*;
use crate::shape_link::ShapeLinkError;

impl DecodedShapeLinkContractV1 {
    pub fn validate_against(
        self,
        expected: &ShapeLinkContractV1<'_>,
        meter: &mut BudgetMeter,
    ) -> Result<(), ShapeLinkError> {
        let path = WirePath::root();
        meter.charge_work(1, &path)?;
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
                meter.charge_work(
                    (canonical_signature.argument_count() as u64)
                        .saturating_add(canonical_signature.signature_parameter_count() as u64)
                        .saturating_add(expected_signature.arguments().len() as u64)
                        .saturating_add(expected_signature.signature().parameters().len() as u64),
                    &path,
                )?;
                let actual =
                    encode_canonical_temporary_with_meter(&canonical_signature, meter, &path)?;
                let wanted =
                    encode_canonical_temporary_with_meter(*expected_signature, meter, &path)?;
                meter.charge_work(actual.len() as u64, &path)?;
                if actual != wanted {
                    return Err(ShapeLinkError::Contract);
                }
            }
            (Self::Layout(actual), ShapeLinkContractV1::Layout { record }) => {
                actual.validate_against(record, meter)?
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
                let checked = canonical_scan.validate_metered(meter)?;
                if checked.as_ref_scan() != *expected_scan {
                    return Err(ShapeLinkError::Contract);
                }
            }
            (
                Self::Type(actual),
                ShapeLinkContractV1::Type {
                    descriptor_projection,
                },
            ) => actual.validate_against(descriptor_projection, meter)?,
            (Self::Dispatch(actual), ShapeLinkContractV1::Dispatch { table_projection }) => {
                actual.validate_against(table_projection, meter)?
            }
            (
                Self::StaticStorage(actual),
                ShapeLinkContractV1::StaticStorage { storage_projection },
            ) => actual.validate_against(storage_projection, meter)?,
            (
                Self::Initialization(actual),
                ShapeLinkContractV1::Initialization { unit_projection },
            ) => actual.validate_against(*unit_projection, meter)?,
            _ => return Err(ShapeLinkError::Contract),
        }
        Ok(())
    }
}
