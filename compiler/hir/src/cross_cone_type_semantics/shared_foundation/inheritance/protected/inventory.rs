use super::*;
use crate::{CanonicalProtectedDeclarationRefsV1, ProtectedCallableDeclarationRefV1};

pub(super) fn validate(
    provider: CheckedSharedTypeFoundationV1<'_>,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let metadata = provider.metadata;
    let mut required = Vec::new();
    let path = WirePath::root();
    for source in metadata.public.callable_interfaces().all_declarations() {
        meter.charge_work(1, &path)?;
        if source.declared_visibility() != DeclaredVisibilityV1::Protected {
            continue;
        }
        let reference = match source.declaration() {
            CallableTemplateOrigin::Constructor(id) => ProtectedDeclarationRefV1::Constructor(id),
            declaration => ProtectedDeclarationRefV1::Callable(
                ProtectedCallableDeclarationRefV1::try_new(declaration)
                    .map_err(|_| Error::CallableContract(declaration))?,
            ),
        };
        meter.try_reserve_collection_slots(&mut required, 1, &path)?;
        required.push(reference);
    }
    for source in metadata.public.property_interfaces().all_declarations() {
        meter.charge_work(1, &path)?;
        if source.declared_visibility() != DeclaredVisibilityV1::Protected {
            continue;
        }
        let PropertyOwner::Property(id) = source.declaration() else {
            return Err(Error::ProtectedInventory(metadata.provider));
        };
        meter.try_reserve_collection_slots(&mut required, 1, &path)?;
        required.push(ProtectedDeclarationRefV1::Property(id));
    }
    for source in metadata.public.nominal_interfaces().all_records() {
        meter.charge_work(1, &path)?;
        if source.declaration_details().declared_visibility() == DeclaredVisibilityV1::Protected {
            meter.try_reserve_collection_slots(&mut required, 1, &path)?;
            required.push(ProtectedDeclarationRefV1::NestedNominal(
                source.declaration(),
            ));
        }
    }
    meter.charge_work(
        (required.len() as u64).saturating_mul(1 + u64::from(required.len().max(1).ilog2())),
        &path,
    )?;
    let required = CanonicalProtectedDeclarationRefsV1::try_new(required)
        .map_err(|_| Error::ProtectedInventory(metadata.provider))?;
    let table = provider.section.protected_declarations();
    for reference in required.values() {
        contracts::lookup(table.records().len(), meter)?;
        if table.get(*reference).is_none() {
            return Err(Error::ProtectedMember(*reference));
        }
    }
    if required.values().len() != table.records().len() {
        return Err(Error::ProtectedInventory(metadata.provider));
    }
    Ok(())
}
