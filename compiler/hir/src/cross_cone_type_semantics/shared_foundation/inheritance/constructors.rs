use super::*;
use crate::{
    DeclaredVisibilityV1, NominalInheritanceInterfaceV1, NominalInterfaceRecordV1,
    ProtectedDeclarationInterfaceV1, ProtectedDeclarationRefV1,
};
use scoop_identity::CallableTemplateOrigin;

pub(super) fn validate(
    provider: CheckedSharedTypeFoundationV1<'_>,
    nominal: &NominalInterfaceRecordV1,
    record: &NominalInheritanceInterfaceV1,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let metadata = provider.metadata;
    let required = nominal.declaration_details().constructors().values();
    meter.charge_work(
        required.len() as u64 + record.constructors().records().len() as u64,
        &WirePath::root(),
    )?;
    let mut candidates = record.constructors().records().iter();
    for id in required {
        let declaration = CallableTemplateOrigin::Constructor(*id);
        let source = contracts::callable(metadata, declaration, meter)?;
        if source.owner().nominal_owner() != Some(nominal.declaration()) {
            return Err(Error::CallableContract(declaration));
        }
        if !matches!(
            source.declared_visibility(),
            DeclaredVisibilityV1::Public | DeclaredVisibilityV1::Protected
        ) {
            continue;
        }
        let candidate = candidates
            .next()
            .filter(|candidate| candidate.declaration() == *id)
            .ok_or(Error::ConstructorInventory(record.owner()))?
            .source();
        contracts::validate_callable(
            metadata,
            declaration,
            candidate.declaration_access(),
            candidate.payload(),
            meter,
        )?;
        if source.declared_visibility() == DeclaredVisibilityV1::Protected {
            let reference = ProtectedDeclarationRefV1::Constructor(*id);
            let table = provider.section.protected_declarations();
            contracts::lookup(table.records().len(), meter)?;
            let Some(ProtectedDeclarationInterfaceV1::Constructor(protected)) =
                table.get(reference)
            else {
                return Err(Error::ProtectedMember(reference));
            };
            contracts::validate_callable(
                metadata,
                declaration,
                protected.declaration_access(),
                protected.payload(),
                meter,
            )?;
        }
    }
    if candidates.next().is_some() {
        return Err(Error::ConstructorInventory(record.owner()));
    }
    Ok(())
}
