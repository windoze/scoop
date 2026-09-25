use std::fmt;

use scoop_wire::WirePath;

use super::{
    CrossConeHirInterfaceSectionV1, ExternalHirAliasClosureValidationError,
    ExternalHirConstTypeClosureValidationError, ExternalHirDefaultClosureValidationError,
    ExternalHirInheritanceClosureValidationError, ExternalHirSignatureClosureValidationError,
};
use crate::{
    ExternalHirReexportClosureValidationError, ExternalHirReferenceSemanticAuthority,
    ExternalHirReferenceSetSemanticValidationError,
};

impl CrossConeHirInterfaceSectionV1 {
    /// Validates every external reference record and the six export roles
    /// that can be reconstructed from fields 1 through 8.
    ///
    /// `ConcreteSelectedUse` is intentionally not reconstructed here. Its
    /// explicit selected edge is validated with the later MIR/LIR bridges.
    pub fn validate_external_reference_closure<A, E>(
        &self,
        authority: &mut A,

        path: &WirePath,
    ) -> Result<(), CrossConeHirExternalReferenceValidationError<E>>
    where
        A: ExternalHirReferenceSemanticAuthority<E>,
    {
        self.external_references()
            .validate_semantics(authority)
            .map_err(CrossConeHirExternalReferenceValidationError::Records)?;
        self.external_references()
            .validate_reexport_closure(self.public_bindings(), authority, &path.clone().field(10))
            .map_err(|error| {
                CrossConeHirExternalReferenceValidationError::Reexports(Box::new(error))
            })?;
        self.validate_signature_reference_closure(authority, path)
            .map_err(|error| {
                CrossConeHirExternalReferenceValidationError::Signatures(Box::new(error))
            })?;
        self.validate_alias_reference_closure(authority, path)
            .map_err(|error| {
                CrossConeHirExternalReferenceValidationError::Aliases(Box::new(error))
            })?;
        self.validate_default_reference_closure(authority, path)
            .map_err(|error| {
                CrossConeHirExternalReferenceValidationError::Defaults(Box::new(error))
            })?;
        self.validate_const_type_reference_closure(authority, path)
            .map_err(|error| {
                CrossConeHirExternalReferenceValidationError::ConstTypes(Box::new(error))
            })?;
        self.validate_inheritance_reference_closure(authority)
            .map_err(|error| {
                CrossConeHirExternalReferenceValidationError::Inheritance(Box::new(error))
            })
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum CrossConeHirExternalReferenceValidationError<E> {
    Records(ExternalHirReferenceSetSemanticValidationError<E>),
    Reexports(Box<ExternalHirReexportClosureValidationError<E>>),
    Signatures(Box<ExternalHirSignatureClosureValidationError<E>>),
    Aliases(Box<ExternalHirAliasClosureValidationError<E>>),
    Defaults(Box<ExternalHirDefaultClosureValidationError<E>>),
    ConstTypes(Box<ExternalHirConstTypeClosureValidationError<E>>),
    Inheritance(Box<ExternalHirInheritanceClosureValidationError<E>>),
}

impl<E: fmt::Display> fmt::Display for CrossConeHirExternalReferenceValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (relation, error): (&str, &dyn fmt::Display) = match self {
            Self::Records(error) => ("record semantics", error),
            Self::Reexports(error) => ("re-export closure", error.as_ref()),
            Self::Signatures(error) => ("signature closure", error.as_ref()),
            Self::Aliases(error) => ("type-alias closure", error.as_ref()),
            Self::Defaults(error) => ("default dependency closure", error.as_ref()),
            Self::ConstTypes(error) => ("constant type closure", error.as_ref()),
            Self::Inheritance(error) => ("inheritance dependency closure", error.as_ref()),
        };
        write!(
            formatter,
            "invalid cross-Cone HIR external-reference {relation}: {error}"
        )
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for CrossConeHirExternalReferenceValidationError<E>
{
}

#[cfg(test)]
mod tests;
