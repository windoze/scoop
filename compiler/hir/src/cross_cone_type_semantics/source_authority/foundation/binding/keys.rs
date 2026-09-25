use super::super::super::binding_keys::{index, select, verify};
use super::*;

pub(super) fn bind<'a>(
    source: &'a TypeFoundationSourceAuthorityV1,
    foundation: &'a OdrFreeHirFoundation,
    identities: &'a ValidatedIdentityGraph,
) -> Result<BoundTypeFoundationSourcesV1<'a>, TypeFoundationBindingError> {
    let canonical = foundation.as_canonical();
    let entries = source.entries();
    let exacts = index(canonical.type_source_exact_records())?;
    let exact_keys = select(
        entries.exact_keys.values(),
        &exacts,
        identities,
        TypeFoundationBindingError::MissingExact,
    )?;

    let concrete = index(canonical.type_source_nominal_records())?;
    let generic = index(canonical.type_source_generic_records())?;
    let mut nominal_keys = BTreeMap::new();

    for record in entries.sources.records() {
        let owner = record.owner();
        let key = match owner {
            SourceNominalId::Concrete(id) => {
                let key = concrete
                    .get(&id)
                    .copied()
                    .ok_or(TypeFoundationBindingError::MissingNominal(owner))?;
                verify(id, key, identities)?;
                key
            }
            SourceNominalId::GenericTemplate(id) => {
                let key = generic
                    .get(&id)
                    .copied()
                    .ok_or(TypeFoundationBindingError::MissingNominal(owner))?;
                verify(id, key, identities)?;
                key
            }
        };
        if key.origin() != entries.provider {
            return Err(TypeFoundationBindingError::ForeignNominal(owner));
        }
        nominal_keys.insert(owner, key);
    }
    let generated = index(canonical.type_source_generated_records())?;
    let generated_keys = select(
        entries.generated_nominals.values(),
        &generated,
        identities,
        TypeFoundationBindingError::MissingGenerated,
    )?;
    let accessors = index(canonical.type_source_accessor_records())?;
    let accessor_keys = select(
        entries.accessor_keys.values(),
        &accessors,
        identities,
        TypeFoundationBindingError::MissingAccessor,
    )?;
    Ok(BoundTypeFoundationSourcesV1 {
        source,
        foundation,
        identities,
        exact_keys,
        nominal_keys,
        generated_keys,
        accessor_keys,
    })
}
