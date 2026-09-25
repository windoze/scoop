use super::*;

pub(super) fn validate(
    callables: &BoundInheritanceProtectedCallableSourcesV1<'_, '_>,
    constructors: &BoundInheritanceConstructorSourcesV1<'_, '_>,
    protocol: &InheritanceSourceParameterProtocolV1,
    array: PersistentGenericTypeId,
) -> Result<(), InheritanceParameterBindingError> {
    use InheritanceParameterBindingError as Error;
    let owner = protocol.owner();
    let (expected, access) = match owner {
        CallableTemplateOrigin::Constructor(id) => {
            let record = constructors.constructor_source(id)?;
            (record.payload().parameters(), record.declaration_access())
        }
        CallableTemplateOrigin::Function(_) | CallableTemplateOrigin::GenericFunction(_) => {
            let record = callables.callable_source(owner)?;
            (record.payload().parameters(), record.declaration_access())
        }
        _ => return Err(Error::Declaration(owner)),
    };
    super::super::source_parameter_contracts::validate(
        callables.foundation,
        owner,
        protocol.parameters(),
        expected,
        access,
        array,
    )?;
    Ok(())
}
