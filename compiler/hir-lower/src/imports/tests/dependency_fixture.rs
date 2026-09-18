use scoop_hir as hir;
use scoop_identity::{
    BindingTarget, CanonicalIdentifier, CborIdentityRecord, ConeCoordinate, ConeIdentity,
    DeclarationScope, DefinitionOwnerAtom, DefinitionOwnerChain, ExportBindingKey, PackagePath,
    PendingIdentityValidation, PersistentExportBindingId, PersistentTypeAliasId, PersistentTypeId,
    SemanticIdentitySession, SemanticOriginFingerprint, SourceDeclarationKey,
    SourceDeclarationSite, SourceNominalKind,
};
use scoop_wire::{BudgetMeter, DecodeLimits, WirePath, decode_canonical, encode};

pub(super) struct DependencyWorldFixture {
    core: ProviderFixture,
    direct: ProviderFixture,
    core_foundation: hir::ImportedHirFoundation,
    direct_foundation: hir::ImportedHirFoundation,
    aliases: hir::CanonicalTypeAliasExpansionsV1,
}

impl DependencyWorldFixture {
    pub(super) fn with_nested_type(package: &[&str], outer: &str, nested: &str) -> Self {
        let core = ProviderFixture::empty(ConeCoordinate::reserved_core());
        let direct = ProviderFixture::with_nested_type(
            ConeCoordinate::new("test", "dependency", "1.0.0").unwrap(),
            package_path(package),
            outer,
            nested,
        );
        let mut session = SemanticIdentitySession::new();
        let core_foundation = import_foundation(&mut session, &core, 31);
        let direct_foundation = import_foundation(&mut session, &direct, 37);
        Self {
            core,
            direct,
            core_foundation,
            direct_foundation,
            aliases: empty_alias_expansions(),
        }
    }

    pub(super) fn direct_identity(&self) -> ConeIdentity {
        self.direct.identity()
    }

    pub(super) fn world(&self, current: ConeIdentity) -> hir::ImportedSemanticWorld<'_> {
        hir::ImportedSemanticWorld::from_validated_closure(
            current,
            Some(hir::TrustedCoreImportedProviderInput::from_validated(
                certificate(&self.core.coordinate, 31),
                &self.core_foundation,
                &self.core.interface,
                &self.aliases,
            )),
            vec![hir::DirectImportedProviderInput::from_validated(
                certificate(&self.direct.coordinate, 37),
                &self.direct_foundation,
                &self.direct.interface,
                &self.aliases,
            )],
            Vec::new(),
        )
        .unwrap()
    }
}

struct ProviderFixture {
    coordinate: ConeCoordinate,
    foundation: hir::CanonicalHirFoundation,
    interface: hir::CrossConeHirInterfaceSectionV1,
}

impl ProviderFixture {
    fn empty(coordinate: ConeCoordinate) -> Self {
        Self {
            coordinate,
            foundation: hir::CanonicalHirFoundation::empty(),
            interface: interface(Vec::new(), Vec::new()),
        }
    }

    fn with_nested_type(
        coordinate: ConeCoordinate,
        package: PackagePath,
        outer_name: &str,
        nested_name: &str,
    ) -> Self {
        let origin = coordinate.identity().unwrap();
        let outer_key = nominal_key(
            origin,
            package.clone(),
            DefinitionOwnerChain::top_level(),
            outer_name,
        );
        let outer: CborIdentityRecord<PersistentTypeId, SourceDeclarationKey> =
            CborIdentityRecord::from_key(outer_key.clone()).unwrap();
        let outer_binding = binding(origin, package.clone(), outer_name, &outer_key);
        let nested_key = nominal_key(
            origin,
            package.clone(),
            DefinitionOwnerChain::from_outer_to_inner(vec![DefinitionOwnerAtom::Type(outer.id())]),
            nested_name,
        );
        let nested: CborIdentityRecord<PersistentTypeId, SourceDeclarationKey> =
            CborIdentityRecord::from_key(nested_key.clone()).unwrap();
        let nested_binding = binding(origin, package, nested_name, &nested_key);

        let mut foundation = hir::CanonicalHirFoundation::empty();
        foundation
            .set_types(vec![outer.clone(), nested.clone()])
            .unwrap();
        foundation
            .set_export_bindings(vec![outer_binding.clone(), nested_binding.clone()])
            .unwrap();
        Self {
            coordinate,
            foundation,
            interface: interface(
                vec![
                    hir::PublicExportBindingRecordV1::new(
                        outer_binding.id(),
                        hir::ExportBindingSourceV1::DeclaredCurrent {
                            declaration: scoop_identity::BindableEntity::Type(outer.id()),
                        },
                    ),
                    hir::PublicExportBindingRecordV1::new(
                        nested_binding.id(),
                        hir::ExportBindingSourceV1::DeclaredCurrent {
                            declaration: scoop_identity::BindableEntity::Type(nested.id()),
                        },
                    ),
                ],
                vec![
                    nominal_record(outer.id(), vec![nested_binding.id()]),
                    nominal_record(nested.id(), Vec::new()),
                ],
            ),
        }
    }

    fn identity(&self) -> ConeIdentity {
        self.coordinate.identity().unwrap()
    }
}

fn import_foundation(
    session: &mut SemanticIdentitySession,
    fixture: &ProviderFixture,
    fingerprint: u8,
) -> hir::ImportedHirFoundation {
    let decoded: hir::DecodedHirFoundation = decode_canonical(
        &encode(&fixture.foundation).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap();
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(fixture.identity()).unwrap();
    decoded.register_identities(&mut pending).unwrap();
    decoded.resolve_identities(&mut pending).unwrap();
    let identities = pending.finish().unwrap();
    let imported = session
        .import(
            fixture.identity(),
            SemanticOriginFingerprint::new(
                [fingerprint; 32],
                [fingerprint.wrapping_add(1); 32],
                [fingerprint.wrapping_add(2); 32],
            ),
            &identities,
        )
        .unwrap();
    let (hir, _, _) = imported.into_parts();
    hir::ImportedHirFoundation::from_odr_free(
        hir::OdrFreeHirFoundation::try_new(fixture.foundation.clone()).unwrap(),
        hir,
    )
}

fn nominal_key(
    origin: ConeIdentity,
    package: PackagePath,
    owners: DefinitionOwnerChain,
    name: &str,
) -> SourceDeclarationKey {
    SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(origin, package, owners, DeclarationScope::ConeWide).unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
        SourceNominalKind::Class,
        0,
    )
}

fn binding(
    origin: ConeIdentity,
    package: PackagePath,
    name: &str,
    target: &SourceDeclarationKey,
) -> CborIdentityRecord<PersistentExportBindingId, ExportBindingKey> {
    CborIdentityRecord::from_key(ExportBindingKey::new(
        origin,
        package,
        CanonicalIdentifier::new(name).unwrap(),
        BindingTarget::type_name(target).unwrap(),
    ))
    .unwrap()
}

fn nominal_record(
    declaration: PersistentTypeId,
    nested_bindings: Vec<PersistentExportBindingId>,
) -> hir::NominalInterfaceRecordV1 {
    hir::NominalInterfaceRecordV1::try_new(
        hir::SourceNominalId::Concrete(declaration),
        hir::PublicNominalKindV1::Class,
        hir::CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
        hir::CanonicalSignatureTypesV1::try_new(Vec::new()).unwrap(),
        hir::CanonicalPersistentIdsV1::try_new(Vec::new()).unwrap(),
        hir::CanonicalPublicMemberRefsV1::try_new(Vec::new()).unwrap(),
        hir::CanonicalPersistentIdsV1::try_new(nested_bindings).unwrap(),
        hir::NominalSourceShapeV1::Class,
    )
    .unwrap()
}

fn interface(
    bindings: Vec<hir::PublicExportBindingRecordV1>,
    nominals: Vec<hir::NominalInterfaceRecordV1>,
) -> hir::CrossConeHirInterfaceSectionV1 {
    hir::CrossConeHirInterfaceSectionV1::new(
        hir::CanonicalPublicExportBindingsV1::try_new(bindings).unwrap(),
        hir::CanonicalNominalInterfacesV1::try_new(nominals).unwrap(),
        hir::CanonicalCallableInterfacesV1::try_new(Vec::new()).unwrap(),
        hir::CanonicalPropertyInterfacesV1::try_new(Vec::new()).unwrap(),
        hir::CanonicalTypeAliasInterfacesV1::try_new(Vec::new()).unwrap(),
        hir::CanonicalCallableSourceInterfacesV1::try_new(Vec::new()).unwrap(),
        hir::CanonicalExportDefaultTemplatesV1::try_new(Vec::new()).unwrap(),
        hir::CanonicalExportConstValuesV1::try_new(Vec::new()).unwrap(),
        hir::CanonicalExportDefinitionSourcesV1::try_new(Vec::new()).unwrap(),
        hir::CanonicalExternalHirReferencesV1::try_new(Vec::new()).unwrap(),
    )
}

fn empty_alias_expansions() -> hir::CanonicalTypeAliasExpansionsV1 {
    hir::CanonicalTypeAliasInterfacesV1::try_new(Vec::new())
        .unwrap()
        .expand_alias_closure(
            &EmptyAliasAuthority,
            &mut BudgetMeter::new(DecodeLimits::default()),
            &WirePath::root(),
        )
        .unwrap()
}

struct EmptyAliasAuthority;

impl hir::TypeAliasClosureAuthority for EmptyAliasAuthority {
    fn external_type_alias(
        &self,
        _alias: PersistentTypeAliasId,
    ) -> Option<&hir::TypeAliasInterfaceRecordV1> {
        None
    }

    fn is_type_alias_edge_authorized(
        &self,
        _source: PersistentTypeAliasId,
        _target: PersistentTypeAliasId,
    ) -> bool {
        false
    }
}

fn certificate(coordinate: &ConeCoordinate, fingerprint: u8) -> hir::ImportedProviderCertificate {
    hir::ImportedProviderCertificate::from_validated(
        coordinate.clone(),
        coordinate.identity().unwrap(),
        SemanticOriginFingerprint::new(
            [fingerprint; 32],
            [fingerprint.wrapping_add(1); 32],
            [fingerprint.wrapping_add(2); 32],
        ),
    )
}

fn package_path(segments: &[&str]) -> PackagePath {
    PackagePath::from_segments(
        segments
            .iter()
            .map(|segment| CanonicalIdentifier::new(segment).unwrap())
            .collect(),
    )
}
