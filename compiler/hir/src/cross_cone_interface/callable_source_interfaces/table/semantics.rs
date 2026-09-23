use std::fmt;

use scoop_identity::CallableTemplateOrigin;

use super::CanonicalCallableSourceInterfacesV1;
use crate::{
    CallableSourceInterfaceSemanticAuthority, CallableSourceInterfaceSemanticValidationError,
    CanonicalCallableInterfacesV1,
};

impl CanonicalCallableSourceInterfacesV1 {
    /// Validates this table against an already validated callable-interface
    /// table and the current artifact's shared source authorities.
    pub fn validate_semantics<A, E>(
        &self,
        callables: &CanonicalCallableInterfacesV1,
        authority: &mut A,
    ) -> Result<(), CallableSourceInterfaceSetSemanticValidationError<E>>
    where
        A: CallableSourceInterfaceSemanticAuthority<E>,
    {
        for (index, source) in self.records().iter().enumerate() {
            let callable = callables.declaration(source.owner()).ok_or(
                CallableSourceInterfaceSetSemanticValidationError::OrphanSourceInterface {
                    index,
                    owner: source.owner(),
                },
            )?;
            source
                .validate_semantics(callable, authority)
                .map_err(
                    |error| CallableSourceInterfaceSetSemanticValidationError::Record {
                        index,
                        error,
                    },
                )?;
        }

        for callable in callables.all_declarations() {
            let declaration = callable.declaration();
            if matches!(declaration, CallableTemplateOrigin::Accessor(_)) {
                continue;
            }
            if self.get(declaration).is_none() {
                return Err(
                    CallableSourceInterfaceSetSemanticValidationError::MissingSourceInterface(
                        declaration,
                    ),
                );
            }
        }
        Ok(())
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum CallableSourceInterfaceSetSemanticValidationError<E> {
    OrphanSourceInterface {
        index: usize,
        owner: CallableTemplateOrigin,
    },
    Record {
        index: usize,
        error: CallableSourceInterfaceSemanticValidationError<E>,
    },
    MissingSourceInterface(CallableTemplateOrigin),
}

impl<E: fmt::Display> fmt::Display for CallableSourceInterfaceSetSemanticValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OrphanSourceInterface { index, owner } => write!(
                formatter,
                "callable source interface {owner:?} at index {index} has no callable interface"
            ),
            Self::Record { index, error } => write!(
                formatter,
                "invalid callable source interface semantics at index {index}: {error}"
            ),
            Self::MissingSourceInterface(declaration) => write!(
                formatter,
                "callable interface {declaration:?} has no source interface"
            ),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for CallableSourceInterfaceSetSemanticValidationError<E>
{
}

#[cfg(test)]
mod tests;
