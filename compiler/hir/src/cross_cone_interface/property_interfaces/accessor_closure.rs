use std::collections::BTreeMap;
use std::fmt;

use scoop_identity::{
    AccessorRole, CallableTemplateOrigin, CoreBuiltinNominal, Effect, PersistentPropertyAccessorId,
    SignatureTypeKey,
};

use super::{
    CanonicalPropertyInterfacesV1, PropertyInterfaceRecordV1, PropertyPublicAccessV1,
    PropertyRepresentationV1, PropertySetterPublicAccessV1,
};
use crate::{
    CallableImplementationV1, CallableInfixV1, CallableInterfaceRecordV1, CallableModalityV1,
    CallableOperatorRoleV1, CanonicalCallableInterfacesV1, PropertyDeclarationId,
    PublicDeclarationOwnerV1, PublicLookupAccessV1,
};

#[derive(Clone, Copy)]
struct AccessorExpectation<'property> {
    property: &'property PropertyInterfaceRecordV1,
    role: AccessorRole,
    public: bool,
}

impl CanonicalPropertyInterfacesV1 {
    pub fn validate_accessor_closure(
        &self,
        callables: &CanonicalCallableInterfacesV1,
    ) -> Result<(), PropertyAccessorClosureValidationError> {
        let mut expected = BTreeMap::new();
        for property in self.records() {
            insert_expectation(
                &mut expected,
                property.capability().getter(),
                AccessorExpectation {
                    property,
                    role: AccessorRole::Getter,
                    public: true,
                },
            )?;
            if let Some(setter) = property.capability().setter() {
                insert_expectation(
                    &mut expected,
                    setter,
                    AccessorExpectation {
                        property,
                        role: AccessorRole::Setter,
                        public: property.capability().setter_access()
                            == Some(PropertySetterPublicAccessV1::Public),
                    },
                )?;
            }
        }

        for (&accessor, expectation) in &expected {
            let callable = callables.get(CallableTemplateOrigin::Accessor(accessor));
            if !expectation.public {
                if callable.is_some() {
                    return Err(
                        PropertyAccessorClosureValidationError::RestrictedSetterExported {
                            property: expectation.property.declaration(),
                            accessor,
                        },
                    );
                }
                continue;
            }
            let callable = callable.ok_or(
                PropertyAccessorClosureValidationError::MissingPublicAccessor {
                    property: expectation.property.declaration(),
                    role: expectation.role,
                    accessor,
                },
            )?;
            validate_public_accessor(accessor, *expectation, callable)?;
        }

        for callable in callables.records() {
            let CallableTemplateOrigin::Accessor(accessor) = callable.declaration() else {
                continue;
            };
            if !expected.contains_key(&accessor) {
                return Err(PropertyAccessorClosureValidationError::OrphanPublicAccessor(accessor));
            }
        }

        for property in self.records() {
            validate_runtime_modality(property, callables)?;
        }
        Ok(())
    }
}

fn insert_expectation<'property>(
    expected: &mut BTreeMap<PersistentPropertyAccessorId, AccessorExpectation<'property>>,
    accessor: PersistentPropertyAccessorId,
    expectation: AccessorExpectation<'property>,
) -> Result<(), PropertyAccessorClosureValidationError> {
    if let Some(previous) = expected.insert(accessor, expectation) {
        return Err(
            PropertyAccessorClosureValidationError::DuplicateAccessorClaim {
                accessor,
                first: previous.property.declaration(),
                second: expectation.property.declaration(),
            },
        );
    }
    Ok(())
}

fn validate_public_accessor(
    accessor: PersistentPropertyAccessorId,
    expectation: AccessorExpectation<'_>,
    callable: &CallableInterfaceRecordV1,
) -> Result<(), PropertyAccessorClosureValidationError> {
    let property = expectation.property;
    if callable.owner() != property.owner() {
        return Err(PropertyAccessorClosureValidationError::Owner {
            accessor,
            expected: property.owner(),
            actual: callable.owner(),
        });
    }
    if callable.receiver() != property.receiver() {
        return Err(PropertyAccessorClosureValidationError::Receiver {
            accessor,
            expected: property.receiver().cloned().map(Box::new),
            actual: callable.receiver().cloned().map(Box::new),
        });
    }

    validate_parameters(accessor, expectation, callable)?;
    let expected_result = match expectation.role {
        AccessorRole::Getter => property.value_type().clone(),
        AccessorRole::Setter => {
            SignatureTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id())
        }
    };
    if callable.result() != &expected_result {
        return Err(PropertyAccessorClosureValidationError::Result {
            accessor,
            expected: Box::new(expected_result),
            actual: Box::new(callable.result().clone()),
        });
    }

    let expected_access = match property.access() {
        PropertyPublicAccessV1::DirectOnly => PublicLookupAccessV1::DirectOnly,
        PropertyPublicAccessV1::PublicSlot => PublicLookupAccessV1::PublicSlot,
    };
    if callable.access() != expected_access {
        return Err(PropertyAccessorClosureValidationError::Access {
            accessor,
            expected: expected_access,
            actual: callable.access(),
        });
    }

    let effects = callable.effects();
    if effects.execution() != Effect::Ordinary {
        return Err(PropertyAccessorClosureValidationError::Execution(accessor));
    }
    if effects.implementation() != CallableImplementationV1::Scoop {
        return Err(PropertyAccessorClosureValidationError::Implementation {
            accessor,
            actual: effects.implementation(),
        });
    }
    if effects.operator_role() != CallableOperatorRoleV1::None {
        return Err(PropertyAccessorClosureValidationError::OperatorRole {
            accessor,
            actual: effects.operator_role(),
        });
    }
    if effects.infix() != CallableInfixV1::Ordinary {
        return Err(PropertyAccessorClosureValidationError::Infix(accessor));
    }

    match property.representation() {
        PropertyRepresentationV1::Const if callable.modality() != CallableModalityV1::Final => {
            Err(PropertyAccessorClosureValidationError::Modality {
                accessor,
                expected: CallableModalityV1::Final,
                actual: callable.modality(),
            })
        }
        PropertyRepresentationV1::AbstractSlot
            if callable.modality() != CallableModalityV1::Abstract =>
        {
            Err(PropertyAccessorClosureValidationError::Modality {
                accessor,
                expected: CallableModalityV1::Abstract,
                actual: callable.modality(),
            })
        }
        PropertyRepresentationV1::Const
        | PropertyRepresentationV1::RuntimeAccessor
        | PropertyRepresentationV1::AbstractSlot => Ok(()),
    }
}

fn validate_parameters(
    accessor: PersistentPropertyAccessorId,
    expectation: AccessorExpectation<'_>,
    callable: &CallableInterfaceRecordV1,
) -> Result<(), PropertyAccessorClosureValidationError> {
    let parameters = callable.parameters().parameters();
    let expected_arity = match expectation.role {
        AccessorRole::Getter => 0,
        AccessorRole::Setter => 1,
    };
    if parameters.len() != expected_arity {
        return Err(PropertyAccessorClosureValidationError::ParameterArity {
            accessor,
            expected: expected_arity,
            actual: parameters.len(),
        });
    }
    if let Some(parameter) = parameters.first()
        && parameter.value_type() != expectation.property.value_type()
    {
        return Err(PropertyAccessorClosureValidationError::ParameterType {
            accessor,
            expected: Box::new(expectation.property.value_type().clone()),
            actual: Box::new(parameter.value_type().clone()),
        });
    }
    Ok(())
}

fn validate_runtime_modality(
    property: &PropertyInterfaceRecordV1,
    callables: &CanonicalCallableInterfacesV1,
) -> Result<(), PropertyAccessorClosureValidationError> {
    if property.representation() != PropertyRepresentationV1::RuntimeAccessor
        || property.capability().setter_access() == Some(PropertySetterPublicAccessV1::Restricted)
    {
        return Ok(());
    }
    let getter_id = property.capability().getter();
    let getter = callables
        .get(CallableTemplateOrigin::Accessor(getter_id))
        .ok_or(
            PropertyAccessorClosureValidationError::MissingPublicAccessor {
                property: property.declaration(),
                role: AccessorRole::Getter,
                accessor: getter_id,
            },
        )?;
    let getter_abstract = getter.modality() == CallableModalityV1::Abstract;
    let setter_abstract = if let Some(setter) = property.capability().setter() {
        callables
            .get(CallableTemplateOrigin::Accessor(setter))
            .ok_or(
                PropertyAccessorClosureValidationError::MissingPublicAccessor {
                    property: property.declaration(),
                    role: AccessorRole::Setter,
                    accessor: setter,
                },
            )?
            .modality()
            == CallableModalityV1::Abstract
    } else {
        true
    };
    if getter_abstract && setter_abstract {
        return Err(
            PropertyAccessorClosureValidationError::RuntimeOnlyAbstractAccessors(
                property.declaration(),
            ),
        );
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PropertyAccessorClosureValidationError {
    DuplicateAccessorClaim {
        accessor: PersistentPropertyAccessorId,
        first: PropertyDeclarationId,
        second: PropertyDeclarationId,
    },
    MissingPublicAccessor {
        property: PropertyDeclarationId,
        role: AccessorRole,
        accessor: PersistentPropertyAccessorId,
    },
    RestrictedSetterExported {
        property: PropertyDeclarationId,
        accessor: PersistentPropertyAccessorId,
    },
    OrphanPublicAccessor(PersistentPropertyAccessorId),
    Owner {
        accessor: PersistentPropertyAccessorId,
        expected: PublicDeclarationOwnerV1,
        actual: PublicDeclarationOwnerV1,
    },
    Receiver {
        accessor: PersistentPropertyAccessorId,
        expected: Option<Box<SignatureTypeKey>>,
        actual: Option<Box<SignatureTypeKey>>,
    },
    ParameterArity {
        accessor: PersistentPropertyAccessorId,
        expected: usize,
        actual: usize,
    },
    ParameterType {
        accessor: PersistentPropertyAccessorId,
        expected: Box<SignatureTypeKey>,
        actual: Box<SignatureTypeKey>,
    },
    Result {
        accessor: PersistentPropertyAccessorId,
        expected: Box<SignatureTypeKey>,
        actual: Box<SignatureTypeKey>,
    },
    Access {
        accessor: PersistentPropertyAccessorId,
        expected: PublicLookupAccessV1,
        actual: PublicLookupAccessV1,
    },
    Execution(PersistentPropertyAccessorId),
    Implementation {
        accessor: PersistentPropertyAccessorId,
        actual: CallableImplementationV1,
    },
    OperatorRole {
        accessor: PersistentPropertyAccessorId,
        actual: CallableOperatorRoleV1,
    },
    Infix(PersistentPropertyAccessorId),
    Modality {
        accessor: PersistentPropertyAccessorId,
        expected: CallableModalityV1,
        actual: CallableModalityV1,
    },
    RuntimeOnlyAbstractAccessors(PropertyDeclarationId),
}

impl fmt::Display for PropertyAccessorClosureValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateAccessorClaim {
                accessor,
                first,
                second,
            } => write!(
                formatter,
                "property accessor {accessor} is claimed by both {first:?} and {second:?}"
            ),
            Self::MissingPublicAccessor {
                property,
                role,
                accessor,
            } => write!(
                formatter,
                "property {property:?} has no public {role:?} callable {accessor}"
            ),
            Self::RestrictedSetterExported { property, accessor } => write!(
                formatter,
                "property {property:?} exports restricted setter {accessor}"
            ),
            Self::OrphanPublicAccessor(accessor) => {
                write!(
                    formatter,
                    "public property accessor {accessor} has no property"
                )
            }
            Self::Owner {
                accessor,
                expected,
                actual,
            } => write!(
                formatter,
                "property accessor {accessor} owner {actual:?} does not match {expected:?}"
            ),
            Self::Receiver {
                accessor,
                expected,
                actual,
            } => write!(
                formatter,
                "property accessor {accessor} receiver {actual:?} does not match {expected:?}"
            ),
            Self::ParameterArity {
                accessor,
                expected,
                actual,
            } => write!(
                formatter,
                "property accessor {accessor} expects {expected} parameters, found {actual}"
            ),
            Self::ParameterType {
                accessor,
                expected,
                actual,
            } => write!(
                formatter,
                "property accessor {accessor} parameter type {actual:?} does not match {expected:?}"
            ),
            Self::Result {
                accessor,
                expected,
                actual,
            } => write!(
                formatter,
                "property accessor {accessor} result {actual:?} does not match {expected:?}"
            ),
            Self::Access {
                accessor,
                expected,
                actual,
            } => write!(
                formatter,
                "property accessor {accessor} access {actual:?} does not match {expected:?}"
            ),
            Self::Execution(accessor) => {
                write!(formatter, "property accessor {accessor} must be ordinary")
            }
            Self::Implementation { accessor, actual } => write!(
                formatter,
                "property accessor {accessor} must use Scoop implementation, found {actual:?}"
            ),
            Self::OperatorRole { accessor, actual } => write!(
                formatter,
                "property accessor {accessor} cannot have operator role {actual:?}"
            ),
            Self::Infix(accessor) => {
                write!(formatter, "property accessor {accessor} cannot be infix")
            }
            Self::Modality {
                accessor,
                expected,
                actual,
            } => write!(
                formatter,
                "property accessor {accessor} modality {actual:?} does not match {expected:?}"
            ),
            Self::RuntimeOnlyAbstractAccessors(property) => write!(
                formatter,
                "runtime property {property:?} has only abstract public accessors"
            ),
        }
    }
}

impl std::error::Error for PropertyAccessorClosureValidationError {}

#[cfg(test)]
mod tests;
