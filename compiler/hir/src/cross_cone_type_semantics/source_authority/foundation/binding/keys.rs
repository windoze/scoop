use super::super::super::binding_keys::{charge_map, index, select, verify};
use super::*;

pub(super) fn bind<'a>(
    source: &'a TypeFoundationSourceAuthorityV1,
    foundation: &'a OdrFreeHirFoundation,
    identities: &'a ValidatedIdentityGraph,
    meter: &mut BudgetMeter,
) -> Result<BoundTypeFoundationSourcesV1<'a>, TypeFoundationBindingError> {
    let root = WirePath::root();
    meter.check_semantic_depth(1, &root)?;
    meter.charge_nodes(1, &root)?;
    let canonical = foundation.as_canonical();
    let entries = source.entries();
    let exacts = index(
        canonical.type_source_exact_records(),
        meter,
        &root.clone().field(2),
    )?;
    let exact_keys = select(
        entries.exact_keys.values(),
        &exacts,
        identities,
        meter,
        &root.clone().field(2),
        TypeFoundationBindingError::MissingExact,
    )?;

    let concrete = index(
        canonical.type_source_nominal_records(),
        meter,
        &root.clone().field(3),
    )?;
    let generic = index(
        canonical.type_source_generic_records(),
        meter,
        &root.clone().field(3),
    )?;
    let mut nominal_keys = BTreeMap::new();
    charge_map(
        entries.sources.records().len(),
        meter,
        &root.clone().field(3),
    )?;
    for record in entries.sources.records() {
        let owner = record.owner();
        let key = match owner {
            SourceNominalId::Concrete(id) => {
                let key = concrete
                    .get(&id)
                    .copied()
                    .ok_or(TypeFoundationBindingError::MissingNominal(owner))?;
                verify(id, key, identities, meter, &root.clone().field(3))?;
                key
            }
            SourceNominalId::GenericTemplate(id) => {
                let key = generic
                    .get(&id)
                    .copied()
                    .ok_or(TypeFoundationBindingError::MissingNominal(owner))?;
                verify(id, key, identities, meter, &root.clone().field(3))?;
                key
            }
        };
        if key.origin() != entries.provider {
            return Err(TypeFoundationBindingError::ForeignNominal(owner));
        }
        nominal_keys.insert(owner, key);
    }
    let generated = index(
        canonical.type_source_generated_records(),
        meter,
        &root.clone().field(5),
    )?;
    let generated_keys = select(
        entries.generated_nominals.values(),
        &generated,
        identities,
        meter,
        &root.clone().field(5),
        TypeFoundationBindingError::MissingGenerated,
    )?;
    let accessors = index(
        canonical.type_source_accessor_records(),
        meter,
        &root.clone().field(6),
    )?;
    let accessor_keys = select(
        entries.accessor_keys.values(),
        &accessors,
        identities,
        meter,
        &root.clone().field(6),
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
