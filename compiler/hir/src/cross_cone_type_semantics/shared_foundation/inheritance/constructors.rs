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
    context: &Context<'_>,
) -> Result<(), Error> {
    let owners = context
        .source(nominal.declaration())?
        .access
        .lexical_owners();

    if owners
        .iter()
        .any(|owner| matches!(owner, SourceNominalId::GenericTemplate(_)))
    {
        return if record.constructors().records().is_empty() {
            Ok(())
        } else {
            Err(Error::ConstructorInventory(record.owner()))
        };
    }
    let metadata = provider.metadata;
    let required = nominal.declaration_details().constructors().values();

    let mut candidates = record.constructors().records().iter();
    for id in required {
        let declaration = CallableTemplateOrigin::Constructor(*id);
        let source = contracts::callable(metadata, declaration)?;
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
        )?;
        if source.declared_visibility() == DeclaredVisibilityV1::Protected {
            let reference = ProtectedDeclarationRefV1::Constructor(*id);
            let table = provider.section.protected_declarations();

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
            )?;
        }
    }
    if candidates.next().is_some() {
        return Err(Error::ConstructorInventory(record.owner()));
    }
    Ok(())
}
