use super::*;

pub(super) fn validate(
    members: &BoundNominalMemberSourcesV1<'_, '_, '_>,
    constructors: &BoundNominalConstructorSourcesV1<'_, '_, '_>,
    protocol: &NominalSourceParameterProtocolV1,
    array: PersistentGenericTypeId,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let owner = protocol.owner();
    let (expected, access) = match owner {
        CallableTemplateOrigin::Constructor(id) => {
            query(constructors.table().records().len(), meter)?;
            let record = constructors.constructor_source(id)?;
            (record.payload().parameters(), record.declaration_access())
        }
        CallableTemplateOrigin::Function(_)
        | CallableTemplateOrigin::GenericFunction(_)
        | CallableTemplateOrigin::VariantConstructor(_) => {
            query(members.callables().records().len(), meter)?;
            let record = members.callable_source(owner)?;
            (record.payload().parameters(), record.declaration_access())
        }
        CallableTemplateOrigin::Accessor(_) => return Err(Error::Declaration(owner)),
    };
    super::super::source_parameter_contracts::validate(
        members.nominals.foundation,
        owner,
        protocol.parameters(),
        expected,
        access,
        array,
        meter,
    )?;
    Ok(())
}
