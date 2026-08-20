use std::fmt;

use scoop_identity::{PersistentPropertyId, PropertyOwner};

use super::CanonicalExportConstValuesV1;
use crate::{CanonicalPropertyInterfacesV1, PropertyDeclarationId, PropertyRepresentationV1};

impl CanonicalExportConstValuesV1 {
    pub fn validate_property_closure(
        &self,
        properties: &CanonicalPropertyInterfacesV1,
    ) -> Result<(), ExportConstValueClosureValidationError> {
        let mut actual_index = 0;
        for interface in properties
            .records()
            .iter()
            .filter(|record| record.representation() == PropertyRepresentationV1::Const)
        {
            let PropertyOwner::Property(expected) = interface.declaration() else {
                return Err(
                    ExportConstValueClosureValidationError::InvalidConstPropertyDeclaration(
                        interface.declaration(),
                    ),
                );
            };
            let Some(actual) = self.records().get(actual_index) else {
                return Err(ExportConstValueClosureValidationError::MissingConstValue(
                    expected,
                ));
            };
            match actual.property().cmp(&expected) {
                std::cmp::Ordering::Less => {
                    return Err(ExportConstValueClosureValidationError::OrphanConstValue {
                        index: actual_index,
                        property: actual.property(),
                    });
                }
                std::cmp::Ordering::Greater => {
                    return Err(ExportConstValueClosureValidationError::MissingConstValue(
                        expected,
                    ));
                }
                std::cmp::Ordering::Equal => actual_index += 1,
            }
        }

        if let Some(actual) = self.records().get(actual_index) {
            return Err(ExportConstValueClosureValidationError::OrphanConstValue {
                index: actual_index,
                property: actual.property(),
            });
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExportConstValueClosureValidationError {
    InvalidConstPropertyDeclaration(PropertyDeclarationId),
    MissingConstValue(PersistentPropertyId),
    OrphanConstValue {
        index: usize,
        property: PersistentPropertyId,
    },
}

impl fmt::Display for ExportConstValueClosureValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConstPropertyDeclaration(declaration) => write!(
                formatter,
                "const property interface {declaration:?} is not an ordinary property"
            ),
            Self::MissingConstValue(property) => {
                write!(
                    formatter,
                    "missing exported value for const property {property}"
                )
            }
            Self::OrphanConstValue { index, property } => write!(
                formatter,
                "exported const value {property} at index {index} has no const property interface"
            ),
        }
    }
}

impl std::error::Error for ExportConstValueClosureValidationError {}
