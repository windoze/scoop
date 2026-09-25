use super::*;

pub(super) struct Fixture {
    pub representations: CanonicalNominalRepresentationSupportV1,
    pub inheritance: CanonicalNominalInheritanceInterfacesV1,
    pub protected: CanonicalProtectedDeclarationInterfacesV1,
    pub sources: CanonicalProtectedCallableSourceInterfacesV1,
    pub defaults: CanonicalProtectedDefaultTemplatesV1,
    pub expected: Vec<Expected>,
}
impl Default for Fixture {
    fn default() -> Self {
        Self {
            representations: CanonicalNominalRepresentationSupportV1::try_new(vec![]).unwrap(),
            inheritance: CanonicalNominalInheritanceInterfacesV1::try_new(vec![]).unwrap(),
            protected: CanonicalProtectedDeclarationInterfacesV1::try_new(vec![]).unwrap(),
            sources: CanonicalProtectedCallableSourceInterfacesV1::try_new(vec![]).unwrap(),
            defaults: CanonicalProtectedDefaultTemplatesV1::try_new(vec![]).unwrap(),
            expected: vec![],
        }
    }
}
impl Fixture {
    pub fn inputs(&self) -> TypeDefinitionSourceInputsV1<'_> {
        TypeDefinitionSourceInputsV1 {
            representations: &self.representations,
            inheritance: &self.inheritance,
            protected_declarations: &self.protected,
            source_interfaces: &self.sources,
            defaults: &self.defaults,
        }
    }
    pub fn expect(&mut self, key: UseKey, origin: &ExportDefinitionSourceV1) {
        self.expected.push(Expected {
            key,
            origin: origin.clone(),
        });
    }
    pub fn declared(&self) -> CanonicalExportDefinitionSourcesV1 {
        let mut origins = self
            .expected
            .iter()
            .map(|expected| expected.origin.clone())
            .collect::<Vec<_>>();
        origins.sort_unstable();
        origins.dedup();
        CanonicalExportDefinitionSourcesV1::try_new(origins).unwrap()
    }
    pub fn validate(
        &self,
        declared: &CanonicalExportDefinitionSourcesV1,
    ) -> Result<usize, TypeDefinitionSourceClosureError<&'static str>> {
        let path = WirePath::root().field(19);
        let mut authority = authority::Authority::new(&self.expected, &path);
        self.inputs()
            .validate_definition_sources(declared, &mut authority, &path)?;
        assert!(authority.seen.iter().all(|seen| *seen));
        Ok(authority.calls)
    }
}
pub(super) fn origin(cone: ConeIdentity, start: u64) -> ExportDefinitionSourceV1 {
    let source = SourceIdentity::new(
        cone,
        NormalizedSourcePath::new("src/Origins.scoop").unwrap(),
    )
    .unwrap();
    let context = SourceContextKey::File {
        source: source.clone(),
    };
    ExportDefinitionSourceV1::new(
        DefinitionOrigin::new(source, SourceSpan::new(start, start + 1).unwrap(), &context)
            .unwrap(),
    )
}
pub(super) fn site(cone: ConeIdentity, owners: &[SourceNominalId]) -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        cone,
        PackagePath::root(),
        DefinitionOwnerChain::from_outer_to_inner(
            owners
                .iter()
                .map(|owner| match owner {
                    SourceNominalId::Concrete(id) => DefinitionOwnerAtom::Type(*id),
                    SourceNominalId::GenericTemplate(id) => DefinitionOwnerAtom::GenericType(*id),
                })
                .collect(),
        ),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}
pub(super) fn nominal_key(
    cone: ConeIdentity,
    owners: &[SourceNominalId],
    name: &str,
    kind: SourceNominalKind,
    arity: u32,
) -> SourceDeclarationKey {
    SourceDeclarationKey::nominal(
        site(cone, owners),
        CanonicalIdentifier::new(name).unwrap(),
        kind,
        arity,
    )
}
pub(super) fn access(
    owners: Vec<SourceNominalId>,
    visibility: DeclaredVisibilityV1,
    origin: &ExportDefinitionSourceV1,
) -> DeclarationAccessSourceV1 {
    DeclarationAccessSourceV1::try_new(visibility, owners, origin.clone()).unwrap()
}
pub(super) fn unit() -> SignatureTypeKey {
    SignatureTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id())
}
pub(super) fn default_path() -> StructuralDefinitionPath {
    StructuralDefinitionPath::from_first(
        StructuralPathSegment::new(StructuralDefinitionSiteRole::DefaultValue, 0),
        [],
    )
}
