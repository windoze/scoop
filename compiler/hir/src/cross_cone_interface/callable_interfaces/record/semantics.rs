use scoop_identity::{CallableTemplateOrigin, NominalDeclarationOwner, SignatureTypeKey};

use super::CallableDeclarationRecordV1;
use crate::{CallableDeclarationId, NominalInterfaceShapeAuthority, PublicDeclarationOwnerV1};

mod errors;

pub use errors::CallableInterfaceSemanticValidationError;

/// Identity-derived declaration facts used to validate one callable interface.
///
/// Implementations derive these facts from the exact kind-specific identity
/// and, for accessors and enum variant constructors, the already validated
/// property or nominal source-shape record. `outer_type_parameter_arity` is
/// the single enclosing binder frame visible to the callable signature.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableDeclarationIdentityShapeV1 {
    owner: PublicDeclarationOwnerV1,
    own_type_parameter_arity: u32,
    outer_type_parameter_arity: u32,
    receiver: Option<SignatureTypeKey>,
    parameter_types: Vec<SignatureTypeKey>,
}

impl CallableDeclarationIdentityShapeV1 {
    pub fn new(
        owner: PublicDeclarationOwnerV1,
        own_type_parameter_arity: u32,
        outer_type_parameter_arity: u32,
        receiver: Option<SignatureTypeKey>,
        parameter_types: Vec<SignatureTypeKey>,
    ) -> Self {
        Self {
            owner,
            own_type_parameter_arity,
            outer_type_parameter_arity,
            receiver,
            parameter_types,
        }
    }

    pub const fn owner(&self) -> PublicDeclarationOwnerV1 {
        self.owner
    }

    pub const fn own_type_parameter_arity(&self) -> u32 {
        self.own_type_parameter_arity
    }

    pub const fn outer_type_parameter_arity(&self) -> u32 {
        self.outer_type_parameter_arity
    }

    pub fn receiver(&self) -> Option<&SignatureTypeKey> {
        self.receiver.as_ref()
    }

    pub fn parameter_types(&self) -> &[SignatureTypeKey] {
        &self.parameter_types
    }
}

/// Supplies canonical identity and nominal-shape facts for a resolved
/// callable interface. The returned shape must be derived from the exact
/// typed declaration passed to the method, never from a name or table index.
pub trait CallableInterfaceSemanticAuthority<E>: NominalInterfaceShapeAuthority<E> {
    fn callable_declaration_identity_shape(
        &mut self,
        declaration: CallableDeclarationId,
    ) -> Result<CallableDeclarationIdentityShapeV1, E>;
}

impl CallableDeclarationRecordV1 {
    pub fn validate_semantics<A, E>(
        &self,
        authority: &mut A,
    ) -> Result<(), CallableInterfaceSemanticValidationError<E>>
    where
        A: CallableInterfaceSemanticAuthority<E>,
    {
        let identity = self.validate_identity_semantics(authority)?;

        let outer_arity = (identity.outer_type_parameter_arity != 0)
            .then_some(identity.outer_type_parameter_arity);
        self.type_parameters
            .validate_bound_semantics(outer_arity, authority)
            .map_err(CallableInterfaceSemanticValidationError::TypeParameters)?;
        let scope = self.type_parameters.signature_scope(outer_arity);
        if let crate::CallableReleaseCallabilityV1::NoTransition { requirements } =
            self.effects.release_callability()
        {
            for requirement in requirements {
                scope
                    .validate(&requirement.signature())
                    .map_err(CallableInterfaceSemanticValidationError::ReleaseRequirement)?;
            }
        }
        if let Some(receiver) = &self.receiver {
            scope
                .validate_signature_semantics(receiver, authority)
                .map_err(CallableInterfaceSemanticValidationError::Receiver)?;
        }
        for (index, parameter) in self.parameters.parameters().iter().enumerate() {
            scope
                .validate_signature_semantics(parameter.value_type(), authority)
                .map_err(
                    |error| CallableInterfaceSemanticValidationError::Parameter { index, error },
                )?;
        }
        for (index, parameter) in self.context_parameters.iter().enumerate() {
            scope
                .validate_signature_semantics(parameter.value_type(), authority)
                .map_err(
                    |error| CallableInterfaceSemanticValidationError::ContextParameter {
                        index,
                        error,
                    },
                )?;
        }
        scope
            .validate_signature_semantics(&self.result, authority)
            .map_err(CallableInterfaceSemanticValidationError::Result)
    }

    pub(crate) fn validate_identity_semantics<A, E>(
        &self,
        authority: &mut A,
    ) -> Result<CallableDeclarationIdentityShapeV1, CallableInterfaceSemanticValidationError<E>>
    where
        A: CallableInterfaceSemanticAuthority<E>,
    {
        let identity = authority
            .callable_declaration_identity_shape(self.declaration)
            .map_err(CallableInterfaceSemanticValidationError::Declaration)?;
        self.validate_identity_shape(&identity)?;
        Ok(identity)
    }

    fn validate_identity_shape<E>(
        &self,
        identity: &CallableDeclarationIdentityShapeV1,
    ) -> Result<(), CallableInterfaceSemanticValidationError<E>> {
        if self.owner != identity.owner {
            return Err(CallableInterfaceSemanticValidationError::Owner {
                expected: identity.owner,
                actual: self.owner,
            });
        }
        let actual_arity = self.type_parameters.len_u32();
        if actual_arity != identity.own_type_parameter_arity {
            return Err(
                CallableInterfaceSemanticValidationError::TypeParameterArity {
                    expected: identity.own_type_parameter_arity,
                    actual: actual_arity,
                },
            );
        }
        if self.receiver.as_ref() != identity.receiver.as_ref() {
            return Err(CallableInterfaceSemanticValidationError::ReceiverMismatch {
                expected: identity.receiver.clone().map(Box::new),
                actual: self.receiver.clone().map(Box::new),
            });
        }
        let actual_parameters = self.parameters.parameters();
        if actual_parameters.len() != identity.parameter_types.len() {
            return Err(CallableInterfaceSemanticValidationError::ParameterArity {
                expected: identity.parameter_types.len(),
                actual: actual_parameters.len(),
            });
        }
        for (index, (actual, expected)) in actual_parameters
            .iter()
            .zip(&identity.parameter_types)
            .enumerate()
        {
            if actual.value_type() != expected {
                return Err(
                    CallableInterfaceSemanticValidationError::ParameterTypeMismatch {
                        index,
                        expected: Box::new(expected.clone()),
                        actual: Box::new(actual.value_type().clone()),
                    },
                );
            }
        }
        if matches!(
            self.declaration,
            CallableTemplateOrigin::Constructor(_) | CallableTemplateOrigin::VariantConstructor(_)
        ) && !self.has_constructed_result(identity.outer_type_parameter_arity)
        {
            return Err(CallableInterfaceSemanticValidationError::ConstructedType {
                owner: self.owner,
                actual: Box::new(self.result.clone()),
            });
        }
        Ok(())
    }

    fn has_constructed_result(&self, arity: u32) -> bool {
        match (self.owner, &self.result) {
            (
                PublicDeclarationOwnerV1::Nominal(NominalDeclarationOwner::Concrete(owner)),
                SignatureTypeKey::Nominal(actual),
            ) => owner == *actual && arity == 0,
            (
                PublicDeclarationOwnerV1::Nominal(NominalDeclarationOwner::GenericTemplate(owner)),
                SignatureTypeKey::NominalApplication { origin, arguments },
            ) => {
                owner == *origin
                    && arguments.as_slice().len() == arity as usize
                    && arguments
                        .as_slice()
                        .iter()
                        .enumerate()
                        .all(|(index, argument)| {
                            *argument
                                == SignatureTypeKey::Binder {
                                    depth: 0,
                                    index: index as u32,
                                }
                        })
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests;
