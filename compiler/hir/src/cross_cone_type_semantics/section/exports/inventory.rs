use super::*;
use scoop_identity::{ExactTypeKey, PersistentExactTypeId};
use scoop_wire::{WireError, WireErrorKind};
use std::collections::BTreeMap;

pub(super) fn dependencies<E>(
    provider: ConeIdentity,
    dependencies: &[&CheckedCrossConeTypeSemanticsSectionV1<'_>],
) -> Result<(), TypeSectionExportValidationError<E>> {
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

    path: &WirePath,
) -> Result<(), TypeSectionExportValidationError<E>> {
    use TypeSectionExportValidationError as Error;

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
        if roots
            .binary_search(&SourceNominalId::Concrete(record.owner()))
            .is_err()
        {
            return Err(Error::SourceRoot(SourceNominalId::Concrete(record.owner())));
        }
        let exact = nominal_exact(record.owner(), path)?;
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

    path: &WirePath,
) -> Result<PersistentExactTypeId, WireError> {
    let key = ExactTypeKey::Nominal(owner);

    PersistentExactTypeId::from_key(&key)
        .map_err(|_| WireError::new(WireErrorKind::IntegerOutOfRange, path.clone(), None))
}

pub(super) struct FactDependencies<'a>(BTreeMap<PersistentExactTypeId, CheckedExactTypeFactV1<'a>>);

pub(super) fn inheritance_records<'a, E>(
    local: CheckedNominalInheritanceInterfacesV1<'a>,
    dependencies: &[&'a CheckedCrossConeTypeSemanticsSectionV1<'a>],
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
    ) -> Option<CheckedExactTypeFactV1<'_>> {
        self.0.get(&exact).copied()
    }
}

pub(super) fn facts<'a, F: TypeSectionFoundationSemanticAuthority<E>, E>(
    candidate: &CrossConeTypeSemanticsSectionV1,
    dependencies: &[&'a CheckedCrossConeTypeSemanticsSectionV1<'a>],
    foundation: &F,
) -> Result<FactDependencies<'a>, TypeSectionExportValidationError<E>> {
    use TypeSectionExportValidationError as Error;
    let local = foundation
        .local_exact_facts()
        .map_err(Error::Source)?
        .values();
    let foreign = foundation.dependency_facts().map_err(Error::Source)?;

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

        facts.insert(required.exact, fact);
    }
    Ok(FactDependencies(facts))
}
