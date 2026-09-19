use super::*;

mod records;

pub(super) struct Fixture {
    pub owner: CborIdentityRecord<PersistentTypeId, SourceDeclarationKey>,
    pub unit: CborIdentityRecord<PersistentTypeId, SourceDeclarationKey>,
    pub exact: CborIdentityRecord<PersistentExactTypeId, ExactTypeKey>,
    pub function: CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey>,
    pub context: CborIdentityRecord<PersistentSourceContextId, SourceContextKey>,
    pub origin: ExportDefinitionSourceV1,
}
impl Fixture {
    pub fn new() -> Self {
        let owner = CborIdentityRecord::from_key(SourceDeclarationKey::nominal(
            site(vec![]),
            name("Owner"),
            SourceNominalKind::Class,
            0,
        ))
        .unwrap();
        let unit = CoreBuiltinNominal::Unit.identity_record();
        let exact = CborIdentityRecord::from_key(ExactTypeKey::Nominal(owner.id())).unwrap();
        let function = CborIdentityRecord::from_key(SourceDeclarationKey::function(
            site(vec![DefinitionOwnerAtom::Type(owner.id())]),
            name("choose"),
            0,
            None,
            vec![SignatureTypeKey::Nominal(unit.id())],
        ))
        .unwrap();
        let source = SourceIdentity::single_file();
        let context = CborIdentityRecord::from_key(SourceContextKey::File {
            source: source.clone(),
        })
        .unwrap();
        let origin = ExportDefinitionSourceV1::new(
            scoop_identity::DefinitionOrigin::new(
                source,
                SourceSpan::new(4, 9).unwrap(),
                context.key(),
            )
            .unwrap(),
        );
        Self {
            owner,
            unit,
            exact,
            function,
            context,
            origin,
        }
    }
    pub fn resolver(&self) -> ValidatedIdentityGraph {
        let mut pending = PendingIdentityValidation::new();
        pending
            .register_authority(ConeIdentity::SINGLE_FILE)
            .unwrap();
        pending
            .register_external_canonical_authority(self.owner.clone())
            .unwrap();
        pending
            .register_external_canonical_authority(self.unit.clone())
            .unwrap();
        pending
            .register_external_canonical_authority(self.exact.clone())
            .unwrap();
        pending
            .register_external_canonical_authority(self.function.clone())
            .unwrap();
        pending
            .register_external_canonical_authority(self.context.clone())
            .unwrap();
        pending.finish().unwrap()
    }
    pub fn value_type(&self) -> SignatureTypeKey {
        SignatureTypeKey::Nominal(self.unit.id())
    }
    pub fn key(&self) -> ProtectedDefaultTemplateKeyV1 {
        ProtectedDefaultTemplateKeyV1::try_new(
            CallableTemplateOrigin::Function(self.function.id()),
            0,
        )
        .unwrap()
    }
    pub fn section(&self) -> CrossConeTypeSemanticsSectionV1 {
        let protected = self.declaration();
        CrossConeTypeSemanticsSectionV1::new(
            CanonicalExactTypeFactsV1::try_new(vec![
                ExactTypeFactsV1::try_new(
                    self.exact.id(),
                    ExactTypeKindV1::Reference,
                    ExactTypeGcV1::ContainsManagedReferences,
                )
                .unwrap(),
            ])
            .unwrap(),
            CanonicalNominalRepresentationSupportV1::try_new(vec![
                NominalRepresentationSupportV1::try_new(
                    self.owner.key(),
                    DeclarationAccessSourceV1::try_new(
                        DeclaredVisibilityV1::Public,
                        vec![],
                        self.origin.clone(),
                    )
                    .unwrap(),
                    NominalRepresentationShapeV1::Class {
                        base: OptionalSignatureType::Absent,
                        declared_fields: vec![],
                    },
                )
                .unwrap(),
            ])
            .unwrap(),
            CanonicalNominalInheritanceInterfacesV1::try_new(vec![
                self.inheritance(protected.reference()),
            ])
            .unwrap(),
            CanonicalProtectedDeclarationInterfacesV1::try_new(vec![protected]).unwrap(),
            CanonicalProtectedCallableSourceInterfacesV1::try_new(vec![self.protocol()]).unwrap(),
            CanonicalProtectedDefaultTemplatesV1::try_new(vec![self.template()]).unwrap(),
            CanonicalExportDefinitionSourcesV1::try_new(vec![self.origin.clone()]).unwrap(),
            // This is a transport fixture; local-looking selected records must
            // still be rejected by the later complete semantic transaction.
            CanonicalSelectedExternalTypeUsesV1::try_new(vec![SelectedExternalTypeUseV1::new(
                ConeIdentity::SINGLE_FILE,
                SelectedTypeUseV1::Signature {
                    exact: self.exact.id(),
                },
            )])
            .unwrap(),
        )
    }
}
fn name(value: &str) -> CanonicalIdentifier {
    CanonicalIdentifier::new(value).unwrap()
}
fn site(owners: Vec<DefinitionOwnerAtom>) -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::from_outer_to_inner(owners),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}
