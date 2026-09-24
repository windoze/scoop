use super::*;
use crate::{DeclaredVisibilityV1, SourceNominalId};
use scoop_identity::{DefinitionOwnerAtom, PersistentConstructorId, SourceDeclarationKey};

/// Borrows the complete source constructors required by materialized owners.
/// Object initialization and enum construction have separate typed protocols.
pub fn select_param_free_source_constructors<'a>(
    provider: ConeIdentity,
    public: &'a CrossConeHirInterfaceSectionV1,
    identities: &ValidatedIdentityGraph,
    meter: &mut BudgetMeter,
) -> Result<BTreeMap<PersistentConstructorId, &'a CallableDeclarationRecordV1>, Error> {
    let signatures = signatures::MaterializableSignatures::new(public, meter)?;
    let mut required = BTreeMap::new();
    for nominal in public.nominal_interfaces().all_records() {
        meter.charge_work(1, &WirePath::root())?;
        let SourceNominalId::Concrete(owner) = nominal.declaration() else {
            continue;
        };
        lookup(identities.identity_count(), meter)?;
        let key = identities.canonical_key::<_, SourceDeclarationKey>(owner)?;
        meter.charge_work(key.owners().owners().len() as u64, &WirePath::root())?;
        if key.origin() != provider
            || key
                .owners()
                .owners()
                .iter()
                .any(|owner| matches!(owner, DefinitionOwnerAtom::GenericType(_)))
            || !signatures.owner(Some(nominal.declaration()), meter)?
        {
            continue;
        }
        for &id in nominal.declaration_details().constructors().values() {
            let origin = Origin::Constructor(id);
            lookup(public.callable_interfaces().declaration_count(), meter)?;
            let source = public
                .callable_interfaces()
                .declaration(origin)
                .ok_or(Error::CallableContract(origin))?;
            lookup(identities.identity_count(), meter)?;
            if source.owner().nominal_owner() != Some(nominal.declaration())
                || identities
                    .canonical_key::<_, SourceDeclarationKey>(id)?
                    .origin()
                    != provider
            {
                return Err(Error::CallableContract(origin));
            }
            if !matches!(
                source.declared_visibility(),
                DeclaredVisibilityV1::Public | DeclaredVisibilityV1::Protected
            ) || !signatures.callable(source, meter)?
            {
                continue;
            }
            meter.check_table_entries(required.len() as u64 + 1, &WirePath::root())?;
            meter.charge_collection_slots(1, &WirePath::root())?;
            lookup(required.len(), meter)?;
            if required.insert(id, source).is_some() {
                return Err(Error::CallableContract(origin));
            }
        }
    }
    Ok(required)
}
