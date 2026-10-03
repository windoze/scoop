use super::*;
use crate::SourceNominalId;
use scoop_identity::{PersistentConstructorId, SourceDeclarationKey};

/// Borrows the complete source constructors required by materialized owners.
/// Object initialization and enum construction have separate typed protocols.
pub fn select_param_free_source_constructors<'a>(
    provider: ConeIdentity,
    public: &'a CrossConeHirInterfaceSectionV1,
    identities: &ValidatedIdentityGraph,
) -> Result<BTreeMap<PersistentConstructorId, &'a CallableDeclarationRecordV1>, Error> {
    let signatures = signatures::MaterializableSignatures::new(public)?;
    let mut required = BTreeMap::new();
    for nominal in public.nominal_interfaces().all_records() {
        let SourceNominalId::Concrete(owner) = nominal.declaration() else {
            continue;
        };

        let key = identities.canonical_key::<_, SourceDeclarationKey>(owner)?;

        if key.origin() != provider || !signatures.owner(Some(nominal.declaration())) {
            continue;
        }
        for &id in nominal.declaration_details().constructors().values() {
            let origin = Origin::Constructor(id);

            let source = public
                .callable_interfaces()
                .declaration(origin)
                .ok_or(Error::CallableContract(origin))?;

            if source.owner().nominal_owner() != Some(nominal.declaration())
                || identities
                    .canonical_key::<_, SourceDeclarationKey>(id)?
                    .origin()
                    != provider
            {
                return Err(Error::CallableContract(origin));
            }
            if !signatures.callable(source) {
                continue;
            }

            if required.insert(id, source).is_some() {
                return Err(Error::CallableContract(origin));
            }
        }
    }
    Ok(required)
}
