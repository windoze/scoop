use super::*;
use scoop_identity::{ExactTypeKey, PersistentExactTypeId};
use scoop_wire::{WireError, WireErrorKind};
use std::collections::BTreeMap;

pub(super) fn dependencies<E>(
    provider: ConeIdentity,
    dependencies: &[&CheckedCrossConeTypeSemanticsSectionV1<'_>],
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), TypeSectionExportValidationError<E>> {
    meter.check_table_entries(dependencies.len() as u64, path)?;
    meter.charge_work(dependencies.len() as u64, path)?;
    let mut previous = None;
    for section in dependencies {
        let next = section.provider();
        if next == provider || previous.is_some_and(|previous| previous >= next) {
            return Err(TypeSectionExportValidationError::DependencyOrder);
        }
        previous = Some(next);
    }
    Ok(())
}

pub(super) fn sources<F: TypeSectionFoundationSemanticAuthority<E>, E>(
    candidate: &CrossConeTypeSemanticsSectionV1,
    roots: &[SourceNominalId],
    edges: &[NominalInheritanceEdgesV1],
    foundation: &F,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), TypeSectionExportValidationError<E>> {
    use TypeSectionExportValidationError as Error;
    meter.check_table_entries(roots.len() as u64, path)?;
    meter.check_table_entries(edges.len() as u64, path)?;
    meter.charge_work((roots.len() + edges.len()) as u64, path)?;
    if roots.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(Error::SourceRootOrder);
    }
    if edges
        .windows(2)
        .any(|pair| pair[0].owner() >= pair[1].owner())
        || !edges
            .iter()
            .map(NominalInheritanceEdgesV1::owner)
            .eq(candidate
                .inheritance
                .records()
                .iter()
                .map(NominalInheritanceInterfaceV1::owner))
    {
        return Err(Error::InheritanceInventory);
    }
    for source in roots {
        let key = foundation
            .nominal_declaration_key(*source)
            .map_err(Error::Source)?;
        if key.origin() != foundation.current_provider() {
            return Err(Error::SourceRoot(*source));
        }
    }
    for record in candidate.representation_support.records() {
        meter.charge_work((roots.len() as u64 + 1).ilog2() as u64 + 1, path)?;
        if roots
            .binary_search(&SourceNominalId::Concrete(record.owner()))
            .is_err()
        {
            return Err(Error::SourceRoot(SourceNominalId::Concrete(record.owner())));
        }
        let exact = nominal_exact(record.owner(), meter, path)?;
        if candidate.exact_facts.get(exact).is_none() || candidate.inheritance.get(exact).is_none()
        {
            return Err(Error::MissingNominalSupport(record.owner()));
        }
    }
    for edge in edges {
        let key = foundation
            .exact_type_key(edge.owner())
            .map_err(Error::Source)?;
        let ExactTypeKey::Nominal(owner) = key else {
            return Err(Error::InheritanceInventory);
        };
        meter.charge_work((roots.len() as u64 + 1).ilog2() as u64 + 1, path)?;
        if roots
            .binary_search(&SourceNominalId::Concrete(*owner))
            .is_err()
        {
            return Err(Error::SourceRoot(SourceNominalId::Concrete(*owner)));
        }
        if candidate.representation_support.get(*owner).is_none()
            || candidate.exact_facts.get(edge.owner()).is_none()
        {
            return Err(Error::MissingNominalSupport(*owner));
        }
    }
    Ok(())
}

pub(super) fn nominal_exact(
    owner: scoop_identity::PersistentTypeId,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<PersistentExactTypeId, WireError> {
    let key = ExactTypeKey::Nominal(owner);
    let bytes = PersistentExactTypeId::hash_stream_length(&key)
        .map_err(|_| WireError::new(WireErrorKind::IntegerOutOfRange, path.clone(), None))?;
    meter.charge_sha256(bytes, path)?;
    PersistentExactTypeId::from_key(&key)
        .map_err(|_| WireError::new(WireErrorKind::IntegerOutOfRange, path.clone(), None))
}

pub(super) struct FactDependencies<'a>(BTreeMap<PersistentExactTypeId, CheckedExactTypeFactV1<'a>>);

pub(super) fn inheritance_records<'a, E>(
    local: CheckedNominalInheritanceInterfacesV1<'a>,
    dependencies: &[&'a CheckedCrossConeTypeSemanticsSectionV1<'a>],
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<
    BTreeMap<PersistentExactTypeId, &'a NominalInheritanceInterfaceV1>,
    TypeSectionExportValidationError<E>,
> {
    let mut result = BTreeMap::new();
    for record in local.table().records().iter().chain(
        dependencies
            .iter()
            .flat_map(|section| section.exports.inheritance.table().records()),
    ) {
        meter.charge_work((result.len() as u64 + 1).ilog2() as u64 + 1, path)?;
        meter.charge_collection_slots(1, path)?;
        if result.insert(record.owner(), record).is_some() {
            return Err(TypeSectionExportValidationError::InheritanceInventory);
        }
    }
    Ok(result)
}
impl ExactTypeFactsDependencyLookupV1 for FactDependencies<'_> {
    fn get_dependency_fact(
        &self,
        exact: PersistentExactTypeId,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<Option<CheckedExactTypeFactV1<'_>>, WireError> {
        meter.charge_work((self.0.len() as u64 + 1).ilog2() as u64 + 1, path)?;
        Ok(self.0.get(&exact).copied())
    }
}

pub(super) fn facts<'a, F: TypeSectionFoundationSemanticAuthority<E>, E>(
    candidate: &CrossConeTypeSemanticsSectionV1,
    dependencies: &[&'a CheckedCrossConeTypeSemanticsSectionV1<'a>],
    foundation: &F,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<FactDependencies<'a>, TypeSectionExportValidationError<E>> {
    use TypeSectionExportValidationError as Error;
    let local = foundation
        .local_exact_facts()
        .map_err(Error::Source)?
        .values();
    let foreign = foundation.dependency_facts().map_err(Error::Source)?;
    meter.check_table_entries(local.len() as u64, path)?;
    meter.check_table_entries(foreign.len() as u64, path)?;
    meter.charge_work(
        (local.len() + foreign.len() + candidate.exact_facts.records().len()) as u64,
        path,
    )?;
    if !local.iter().copied().eq(candidate
        .exact_facts
        .records()
        .iter()
        .map(|fact| fact.exact()))
    {
        return Err(Error::FactsInventory);
    }
    if foreign
        .windows(2)
        .any(|pair| pair[0].exact >= pair[1].exact)
    {
        return Err(Error::DependencyFactOrder);
    }
    let mut facts = BTreeMap::new();
    for required in foreign {
        meter.charge_work(
            (local.len() as u64 + dependencies.len() as u64 + 2).ilog2() as u64 + 2,
            path,
        )?;
        if local.binary_search(&required.exact).is_ok() {
            return Err(Error::DependencyFact(required.exact));
        }
        let provider = dependencies
            .binary_search_by_key(&required.provider, |section| section.provider())
            .ok()
            .map(|index| dependencies[index])
            .ok_or(Error::DependencyFact(required.exact))?;
        let fact = provider
            .exports
            .facts
            .get_checked(required.exact)
            .ok_or(Error::DependencyFact(required.exact))?;
        meter.charge_collection_slots(1, path)?;
        meter.charge_work((facts.len() as u64 + 1).ilog2() as u64 + 1, path)?;
        facts.insert(required.exact, fact);
    }
    Ok(FactDependencies(facts))
}
