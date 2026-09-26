use super::*;
use scoop_identity::*;

pub(super) const TARGET: crate::LirTargetProfile = crate::LirTargetProfile::DARWIN_AARCH64;

#[derive(Default)]
pub(super) struct Source(pub Vec<LayoutAbiDependencyV1>);

impl LayoutAbiSectionSourceAuthorityV1<()> for Source {
    fn validate_local_exports(&self, _exports: &LayoutAbiExportConstituentsV1) -> Result<(), ()> {
        Ok(())
    }

    fn committed_semantic_roots(&self) -> Result<&[LayoutAbiDependencyV1], ()> {
        Ok(&self.0)
    }

    fn validate_physical_imports(
        &self,
        imports: &[crate::ExternalShapeLinkImportV1],
    ) -> Result<(), ()> {
        if imports.is_empty() { Ok(()) } else { Err(()) }
    }
}

pub(super) fn cone(name: &str) -> ConeIdentity {
    ConeCoordinate::new("test", name, "1.0.0")
        .unwrap()
        .identity()
        .unwrap()
}

pub(super) fn source(provider: ConeIdentity, name: &str) -> SourceDeclarationKey {
    SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            provider,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
        SourceNominalKind::Struct,
        0,
    )
}

pub(super) fn empty_exports(provider: ConeIdentity) -> LayoutAbiExportConstituentsV1 {
    let foundation =
        crate::OdrFreeLirFoundation::try_new(provider, crate::CanonicalLirFoundation::empty())
            .unwrap();
    exports(&foundation, Vec::new())
}

pub(super) fn exports(
    foundation: &crate::OdrFreeLirFoundation,
    records: Vec<crate::ExactLayoutExportV1>,
) -> LayoutAbiExportConstituentsV1 {
    let layouts =
        crate::CanonicalExactLayoutExportsV1::try_new(TARGET, foundation, records).unwrap();
    let descriptors =
        crate::CanonicalExactDescriptorExportsV1::try_new(TARGET, foundation, Vec::new()).unwrap();
    let dispatch =
        crate::CanonicalExactDispatchExportsV1::try_new(TARGET, foundation, Vec::new()).unwrap();
    let callables =
        crate::CanonicalExactCallableAbiExportsV1::try_new(TARGET, foundation, Vec::new()).unwrap();
    let shape_support = crate::CanonicalParamFreeShapeSupportExportsV1::from_sources(
        &[],
        &layouts,
        &descriptors,
        foundation,
    )
    .unwrap();
    LayoutAbiExportConstituentsV1::try_new(layouts, descriptors, dispatch, callables, shape_support)
        .unwrap()
}

pub(super) fn section<'a>(
    exports: LayoutAbiExportConstituentsV1,
    dependencies: &[&'a LayoutAbiExportConstituentsV1],
    source: &Source,
) -> Result<CrossConeLayoutAbiSectionV1<'a>, LayoutAbiSectionError<()>> {
    CrossConeLayoutAbiSectionV1::try_new(exports, dependencies, Vec::new(), source)
}

pub(super) fn empty_struct(
    provider: ConeIdentity,
    name: &str,
) -> (crate::ExactValueLayoutV1, crate::OdrFreeLirFoundation) {
    let identity = CborIdentityRecord::from_key(ExactTypeKey::Nominal(
        PersistentTypeId::from_source_declaration(&source(provider, name)).unwrap(),
    ))
    .unwrap();
    let (identity, foundation) = bound(
        provider,
        identity,
        RepresentationRole::ManagedValue,
        ScanRole::InlineValue,
    );
    let value =
        crate::ExactValueLayoutV1::ordinary_struct(identity, false, &[], &foundation).unwrap();
    (value, foundation)
}

pub(super) fn struct_with_field(
    provider: ConeIdentity,
    name: &str,
    field_name: &str,
    external: &crate::ExactLayoutExportV1,
) -> (crate::ExactValueLayoutV1, crate::OdrFreeLirFoundation) {
    let owner = source(provider, name);
    let exact = CborIdentityRecord::from_key(ExactTypeKey::Nominal(
        PersistentTypeId::from_source_declaration(&owner).unwrap(),
    ))
    .unwrap();
    let field = CborIdentityRecord::from_key(
        FieldIdentityKey::source_declared(&owner, CanonicalIdentifier::new(field_name).unwrap())
            .unwrap(),
    )
    .unwrap();
    let external = external.value_handle().unwrap();
    let (identity, foundation) = bound(
        provider,
        exact,
        RepresentationRole::ManagedValue,
        ScanRole::InlineValue,
    );
    let value = crate::ExactValueLayoutV1::ordinary_struct(
        identity,
        false,
        &[crate::NominalLayoutFieldInputV1 {
            field: &field,
            value: &external,
        }],
        &foundation,
    )
    .unwrap();
    (value, foundation)
}

pub(super) fn scalar_with_identity(
    provider: ConeIdentity,
    exact: CborIdentityRecord<PersistentExactTypeId, ExactTypeKey>,
) -> crate::ExactValueLayoutV1 {
    let (identity, foundation) = bound(
        provider,
        exact,
        RepresentationRole::ManagedValue,
        ScanRole::InlineValue,
    );
    crate::ExactValueLayoutV1::scalar(
        identity,
        crate::ScalarRepresentationKindV1::Integer(crate::IntegerKind::SIGNED_8),
        &foundation,
    )
    .unwrap()
}

pub(super) fn boxed_with_payload(
    provider: ConeIdentity,
    payload: &crate::ExactValueLayoutV1,
) -> (crate::ExactInstanceLayoutV1, crate::OdrFreeLirFoundation) {
    let nominal = PersistentTypeId::from_generated_key(&GeneratedNominalKey::BoxedValue {
        payload: payload.identity().exact(),
    })
    .unwrap();
    let exact = CborIdentityRecord::from_key(ExactTypeKey::Nominal(nominal)).unwrap();
    let (identity, foundation) = bound(
        provider,
        exact,
        RepresentationRole::ManagedObject,
        ScanRole::ManagedObject,
    );
    let value =
        crate::ExactInstanceLayoutV1::boxed_payload(identity, payload, &foundation).unwrap();
    (value, foundation)
}

fn bound(
    provider: ConeIdentity,
    exact: CborIdentityRecord<PersistentExactTypeId, ExactTypeKey>,
    role: RepresentationRole,
    scan_role: ScanRole,
) -> (crate::ExactLayoutIdentityV1, crate::OdrFreeLirFoundation) {
    let layout =
        CborIdentityRecord::from_key(LayoutKey::new(exact.id(), TARGET.wire_id(), role)).unwrap();
    let scan = CborIdentityRecord::from_key(ScanKey::new(layout.id(), scan_role)).unwrap();
    let mut foundation = crate::CanonicalLirFoundation::empty();
    let mut plans = Vec::new();
    let mut atoms = Vec::new();
    let mut symbols = Vec::new();
    for subject in [
        crate::ExternalStrongShapeSubjectV1::Layout(layout.id()),
        crate::ExternalStrongShapeSubjectV1::Scan(scan.id()),
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
    foundation.set_layouts(vec![layout]).unwrap();
    foundation.set_scans(vec![scan]).unwrap();
    foundation.set_definition_plans(plans).unwrap();
    foundation.set_definition_atoms(atoms).unwrap();
    foundation.set_symbol_requests(PersistentSymbolRequestTable::new(symbols).unwrap());
    let foundation = crate::OdrFreeLirFoundation::try_new(provider, foundation).unwrap();
    let identity =
        crate::ExactLayoutIdentityV1::from_foundation(TARGET, exact, role, &foundation).unwrap();
    (identity, foundation)
}
