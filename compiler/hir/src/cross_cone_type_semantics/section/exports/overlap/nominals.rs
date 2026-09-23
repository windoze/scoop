use super::*;
use scoop_identity::{ExactTypeKey, PersistentExactTypeId, SourceDeclarationKind};

pub(super) fn validate<F: TypeSectionFoundationSemanticAuthority<E>, E>(
    candidate: &CrossConeTypeSemanticsSectionV1,
    public: &CrossConeHirInterfaceSectionV1,
    graph: &CheckedNominalInheritanceGraphV1<'_>,
    foundation: &F,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), TypeSectionExportValidationError<E>> {
    let required = foundation
        .required_representation_owners()
        .map_err(TypeSectionExportValidationError::Source)?
        .values();
    sequence(public.nominal_interfaces().records().len(), meter, path)?;
    for (index, old) in public.nominal_interfaces().records().iter().enumerate() {
        let at = path.clone().field(2).index(index as u64);
        let source = graph
            .source(old.declaration())
            .ok_or(TypeSectionExportValidationError::PublicOverlap)?;
        require(
            source.key.origin() == foundation.current_provider()
                && source.key.duplicate_signature().type_parameter_count()
                    == old.type_parameters().len_u32()
                && kind(source.key.declaration_kind()) == Some(old.kind()),
        )?;
        require(
            graph
                .replay_nominal_access(old.declaration(), meter)
                .map_err(|e| match e {
                    AccessDomainSemanticError::Resource(e) => {
                        TypeSectionExportValidationError::Resource(e)
                    }
                    _ => TypeSectionExportValidationError::PublicOverlap,
                })?
                .lookup()
                .domain()
                .is_universal(),
        )?;
        let SourceNominalId::Concrete(owner) = old.declaration() else {
            continue;
        };
        lookup(required.len(), meter, &at)?;
        if required.binary_search(&owner).is_err() {
            // Source-only declarations have already passed the common source
            // checks. The complete machine inventory is checked separately.
            continue;
        }
        lookup(
            candidate.representation_support().records().len(),
            meter,
            &at,
        )?;
        let representation = candidate
            .representation_support()
            .get(owner)
            .ok_or(TypeSectionExportValidationError::PublicOverlap)?;
        require(representation.public_source_shape_matches(old.source_shape(), meter, &at)?)?;
        meter.charge_work(64, &at)?;
        let exact = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(owner))
            .map_err(|_| TypeSectionExportValidationError::PublicOverlap)?;
        lookup(candidate.inheritance().records().len(), meter, &at)?;
        let new = candidate
            .inheritance()
            .get(exact)
            .ok_or(TypeSectionExportValidationError::PublicOverlap)?;
        require(supertypes(
            old.exact_supertypes().values(),
            new.edges(),
            meter,
            &at,
        )?)?;
        constructors(old, new, public, graph, meter, &at)?;
    }
    Ok(())
}

fn kind(kind: SourceDeclarationKind) -> Option<PublicNominalKindV1> {
    match kind {
        SourceDeclarationKind::Struct => Some(PublicNominalKindV1::Struct),
        SourceDeclarationKind::Enum => Some(PublicNominalKindV1::Enum),
        SourceDeclarationKind::Class => Some(PublicNominalKindV1::Class),
        SourceDeclarationKind::Interface => Some(PublicNominalKindV1::Interface),
        SourceDeclarationKind::Object => Some(PublicNominalKindV1::Object),
        _ => None,
    }
}
fn supertypes(
    old: &[SignatureTypeKey],
    new: &NominalInheritanceEdgesV1,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<bool, WireError> {
    sequence(old.len(), meter, path)?;
    sequence(new.direct_interfaces().len(), meter, path)?;
    let base = match new.direct_base() {
        DirectClassBaseV1::NoClassBase => None,
        DirectClassBaseV1::ClassBase { exact } => Some(exact),
    };
    if old.len() != new.direct_interfaces().len() + usize::from(base.is_some()) {
        return Ok(false);
    }
    for value in old {
        let SignatureTypeKey::Nominal(id) = value else {
            return Ok(false);
        };
        meter.charge_work(64, path)?;
        let exact = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(*id))
            .map_err(|_| overflow(path))?;
        lookup(new.direct_interfaces().len(), meter, path)?;
        if Some(exact) != base && new.direct_interfaces().binary_search(&exact).is_err() {
            return Ok(false);
        }
    }
    Ok(true)
}
fn constructors<E>(
    old: &NominalInterfaceRecordV1,
    new: &NominalInheritanceInterfaceV1,
    public: &CrossConeHirInterfaceSectionV1,
    graph: &CheckedNominalInheritanceGraphV1<'_>,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), TypeSectionExportValidationError<E>> {
    sequence(old.constructors().values().len(), meter, path)?;
    sequence(new.constructors().records().len(), meter, path)?;
    let mut visible = 0;
    for constructor in new.constructors().records() {
        let source = constructor.source();
        callables::validate(
            CallableTemplateOrigin::Constructor(source.declaration()),
            source.declaration_access(),
            source.payload(),
            public,
            graph,
            meter,
            path,
        )?;
        if effective_public(source.declaration_access(), graph, meter, path)? {
            visible += 1;
            lookup(old.constructors().values().len(), meter, path)?;
            require(
                old.constructors()
                    .values()
                    .binary_search(&source.declaration())
                    .is_ok(),
            )?;
        }
    }
    require(visible == old.constructors().values().len())
}
