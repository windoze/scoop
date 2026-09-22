use super::*;
use scoop_lir::*;

pub(super) const TARGET: LirTargetProfile = LirTargetProfile::DARWIN_AARCH64;

pub(super) fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}

pub(super) fn site(provider: ConeIdentity) -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        provider,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}

pub(super) fn table(
    provider: ConeIdentity,
    role: RepresentationRole,
) -> CanonicalExactLayoutExportsV1 {
    let source = SourceDeclarationKey::nominal(
        site(provider),
        CanonicalIdentifier::new("Word").unwrap(),
        SourceNominalKind::Struct,
        0,
    );
    let exact = CborIdentityRecord::from_key(ExactTypeKey::Nominal(
        PersistentTypeId::from_source_declaration(&source).unwrap(),
    ))
    .unwrap();
    let layout =
        CborIdentityRecord::from_key(LayoutKey::new(exact.id(), TARGET.wire_id(), role)).unwrap();
    let scan =
        CborIdentityRecord::from_key(ScanKey::new(layout.id(), ScanRole::InlineValue)).unwrap();
    let mut canonical = CanonicalLirFoundation::empty();
    let mut plans = Vec::new();
    let mut atoms = Vec::new();
    let mut symbols = Vec::new();
    for subject in [
        ExternalStrongShapeSubjectV1::Layout(layout.id()),
        ExternalStrongShapeSubjectV1::Scan(scan.id()),
    ] {
        let (plan, symbol) = subject.expected_definition(provider).unwrap();
        let plan = CborIdentityRecord::from_key(plan).unwrap();
        atoms.push(
            CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
                plan.id(),
                DefinitionAtomRole::Primary,
                DefinitionAtomSubkey::Singleton,
            ))
            .unwrap(),
        );
        plans.push(plan);
        symbols.push(PersistentSymbolRequest::new(symbol, LinkageClass::ConeStrong).unwrap());
    }
    canonical.set_layouts(vec![layout]).unwrap();
    canonical.set_scans(vec![scan]).unwrap();
    canonical.set_definition_plans(plans).unwrap();
    canonical.set_definition_atoms(atoms).unwrap();
    canonical.set_symbol_requests(PersistentSymbolRequestTable::new(symbols).unwrap());
    let foundation = OdrFreeLirFoundation::try_new(provider, canonical).unwrap();
    let identity =
        ExactLayoutIdentityV1::from_foundation(TARGET, exact, role, &foundation, &mut meter())
            .unwrap();
    let value = ExactValueLayoutV1::scalar(
        identity,
        ScalarRepresentationKindV1::Integer(IntegerKind::SIGNED_64),
        &foundation,
        &mut meter(),
    )
    .unwrap();
    CanonicalExactLayoutExportsV1::try_new(TARGET, &foundation, vec![value.into()], &mut meter())
        .unwrap()
}
