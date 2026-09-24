use super::*;

pub(super) fn validate_source_accessor(
    accessor: PersistentPropertyAccessorId,
    expectation: AccessorExpectation<'_>,
    callable: &CallableDeclarationRecordV1,
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
    let unit = SignatureTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id());
    let expected_result = match expectation.role {
        AccessorRole::Getter => property.value_type(),
        AccessorRole::Setter => &unit,
    };
    if callable.result() != expected_result {
        return Err(PropertyAccessorClosureValidationError::Result {
            accessor,
            expected: Box::new(expected_result.clone()),
            actual: Box::new(callable.result().clone()),
        });
    }

    if expectation.role == AccessorRole::Getter
        && callable.declared_visibility() != property.declared_visibility()
    {
        return Err(PropertyAccessorClosureValidationError::Visibility {
            accessor,
            expected: property.declared_visibility(),
            actual: callable.declared_visibility(),
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
    }?;
    use crate::PropertyAccessorImplementationV1 as Form;
    if (expectation.implementation == Form::AbstractSlot)
        != (callable.modality() == CallableModalityV1::Abstract)
        || (!expectation.implementation.requires_body() && !callable.slot_relations().is_empty())
    {
        return Err(PropertyAccessorClosureValidationError::SourceForm {
            accessor,
            implementation: expectation.implementation,
            modality: callable.modality(),
        });
    }
    Ok(())
}

fn validate_parameters(
    accessor: PersistentPropertyAccessorId,
    expectation: AccessorExpectation<'_>,
    callable: &CallableDeclarationRecordV1,
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
