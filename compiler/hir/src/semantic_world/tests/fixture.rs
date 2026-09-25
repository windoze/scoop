use scoop_identity::{
    BindingTarget, CanonicalIdentifier, CborIdentityRecord, ConeCoordinate, ConeIdentity,
    DeclarationScope, DefinitionOwnerAtom, DefinitionOwnerChain, EnumVariantIdentityKey,
    ExportBindingKey, PackagePath, PendingIdentityValidation, PersistentEnumVariantId,
    PersistentExportBindingId, PersistentObjectValueId, PersistentTypeAliasId, PersistentTypeId,
    SemanticIdentitySession, SemanticOriginFingerprint, SourceDeclarationKey,
    SourceDeclarationSite, SourceNominalKind,
};
use scoop_wire::{WirePath, decode_canonical, encode};

use crate::ImportedProviderCertificate;
use crate::{
    CanonicalBinderListV1, CanonicalCallableInterfacesV1, CanonicalCallableSourceInterfacesV1,
    CanonicalExportConstValuesV1, CanonicalExportDefaultTemplatesV1,
    CanonicalExportDefinitionSourcesV1, CanonicalExternalHirReferencesV1, CanonicalHirFoundation,
    CanonicalNominalInterfacesV1, CanonicalPersistentIdsV1, CanonicalPropertyInterfacesV1,
    CanonicalPublicExportBindingsV1, CanonicalPublicMemberRefsV1, CanonicalSignatureTypesV1,
    CanonicalTypeAliasExpansionsV1, CanonicalTypeAliasInterfacesV1, CrossConeHirInterfaceSectionV1,
    DecodedHirFoundation, EnumSourceShapeV1, EnumSourceVariantStyleV1, EnumSourceVariantV1,
    ExportBindingSourceV1, ImportedHirFoundation, NominalInterfaceRecordV1, NominalSourceShapeV1,
    ObjectSourceShapeV1, OdrFreeHirFoundation, PublicExportBindingRecordV1, PublicNominalKindV1,
    ReexportRouteHopV1, ReexportRouteV1, SourceNominalId, TypeAliasClosureAuthority,
    TypeAliasInterfaceRecordV1,
};

mod callable;
pub(crate) use callable::CallableProviderFixture;

pub(super) struct ProviderFixture {
    pub(super) coordinate: ConeCoordinate,
    pub(super) foundation: CanonicalHirFoundation,
    pub(super) interface: CrossConeHirInterfaceSectionV1,
    pub(super) outer: Option<SourceNominalId>,
    pub(super) outer_key: Option<SourceDeclarationKey>,
    pub(super) outer_binding: Option<PersistentExportBindingId>,
    pub(super) nested: Option<PersistentTypeId>,
    pub(super) object_value: Option<PersistentObjectValueId>,
    pub(super) enum_variant: Option<PersistentEnumVariantId>,
    external_types: Vec<CborIdentityRecord<PersistentTypeId, SourceDeclarationKey>>,
}

impl ProviderFixture {
    pub(super) fn empty(coordinate: ConeCoordinate) -> Self {
        Self {
            coordinate,
            foundation: CanonicalHirFoundation::empty(),
            interface: interface(Vec::new(), Vec::new()),
            outer: None,
            outer_key: None,
            outer_binding: None,
            nested: None,
            object_value: None,
            enum_variant: None,
            external_types: Vec::new(),
        }
    }

    pub(super) fn with_nominals(
        coordinate: ConeCoordinate,
        package: PackagePath,
        outer_name: &str,
        nested_name: Option<&str>,
    ) -> Self {
        let origin = coordinate.identity().unwrap();
        let outer_key = nominal_key(
            origin,
            package.clone(),
            DefinitionOwnerChain::top_level(),
            outer_name,
        );
        let outer = CborIdentityRecord::from_key(outer_key.clone()).unwrap();
        let outer_id = SourceNominalId::Concrete(outer.id());
        let outer_binding = binding(origin, package.clone(), outer_name, &outer_key);
        let mut types = vec![outer.clone()];
        let mut binding_records = vec![outer_binding.clone()];
        let mut public_bindings = vec![PublicExportBindingRecordV1::new(
            outer_binding.id(),
            ExportBindingSourceV1::DeclaredCurrent {
                declaration: scoop_identity::BindableEntity::Type(outer.id()),
            },
        )];
        let mut nested_ids = Vec::new();
        let nested = nested_name.map(|nested_name| {
            let key = nominal_key(
                origin,
                package.clone(),
                DefinitionOwnerChain::from_outer_to_inner(vec![DefinitionOwnerAtom::Type(
                    outer.id(),
                )]),
                nested_name,
            );
            let record = CborIdentityRecord::from_key(key.clone()).unwrap();
            let nested_binding = binding(origin, package.clone(), nested_name, &key);
            types.push(record.clone());
            binding_records.push(nested_binding.clone());
            public_bindings.push(PublicExportBindingRecordV1::new(
                nested_binding.id(),
                ExportBindingSourceV1::DeclaredCurrent {
                    declaration: scoop_identity::BindableEntity::Type(record.id()),
                },
            ));
            nested_ids.push(nested_binding.id());
            record.id()
        });

        let mut nominals = vec![nominal_record(outer.id(), nested_ids)];
        if let Some(nested) = nested {
            nominals.push(nominal_record(nested, Vec::new()));
        }
        let mut foundation = CanonicalHirFoundation::empty();
        foundation.set_types(types).unwrap();
        foundation.set_export_bindings(binding_records).unwrap();
        Self {
            coordinate,
            foundation,
            interface: interface(public_bindings, nominals),
            outer: Some(outer_id),
            outer_key: Some(outer_key),
            outer_binding: Some(outer_binding.id()),
            nested,
            object_value: None,
            enum_variant: None,
            external_types: Vec::new(),
        }
    }

    pub(super) fn reexporting_type(
        coordinate: ConeCoordinate,
        package: PackagePath,
        name: &str,
        target_key: SourceDeclarationKey,
        terminal_provider: ConeIdentity,
        terminal_binding: PersistentExportBindingId,
    ) -> Self {
        let origin = coordinate.identity().unwrap();
        let exported = binding(origin, package, name, &target_key);
        let route = ReexportRouteV1::try_new(
            terminal_provider,
            vec![ReexportRouteHopV1::new(terminal_provider, terminal_binding)],
        )
        .unwrap();
        let mut foundation = CanonicalHirFoundation::empty();
        foundation
            .set_export_bindings(vec![exported.clone()])
            .unwrap();
        Self {
            coordinate,
            foundation,
            interface: interface(
                vec![PublicExportBindingRecordV1::new(
                    exported.id(),
                    ExportBindingSourceV1::Reexport {
                        routes: crate::CanonicalReexportRoutesV1::try_new(vec![route]).unwrap(),
                    },
                )],
                Vec::new(),
            ),
            outer: None,
            outer_key: None,
            outer_binding: Some(exported.id()),
            nested: None,
            object_value: None,
            enum_variant: None,
            external_types: vec![CborIdentityRecord::from_key(target_key).unwrap()],
        }
    }

    pub(super) fn with_value_shapes(coordinate: ConeCoordinate) -> Self {
        let origin = coordinate.identity().unwrap();
        let object_key = nominal_key_with_kind(origin, "Registry", SourceNominalKind::Object);
        let object_type: CborIdentityRecord<PersistentTypeId, SourceDeclarationKey> =
            CborIdentityRecord::from_key(object_key.clone()).unwrap();
        let object_value: CborIdentityRecord<PersistentObjectValueId, SourceDeclarationKey> =
            CborIdentityRecord::from_key(object_key).unwrap();

        let enum_key = nominal_key_with_kind(origin, "Choice", SourceNominalKind::Enum);
        let enum_type: CborIdentityRecord<PersistentTypeId, SourceDeclarationKey> =
            CborIdentityRecord::from_key(enum_key.clone()).unwrap();
        let variant = CborIdentityRecord::from_key(
            EnumVariantIdentityKey::source(&enum_key, CanonicalIdentifier::new("Only").unwrap())
                .unwrap(),
        )
        .unwrap();

        let nominals = vec![
            nominal_record_with_shape(
                object_type.id(),
                PublicNominalKindV1::Object,
                NominalSourceShapeV1::Object(ObjectSourceShapeV1::new(
                    object_value.id(),
                    Default::default(),
                )),
            ),
            nominal_record_with_shape(
                enum_type.id(),
                PublicNominalKindV1::Enum,
                NominalSourceShapeV1::Enum(
                    EnumSourceShapeV1::try_new(vec![
                        EnumSourceVariantV1::try_new(
                            variant.id(),
                            EnumSourceVariantStyleV1::Unit,
                            Vec::new(),
                        )
                        .unwrap(),
                    ])
                    .unwrap(),
                ),
            ),
        ];
        let mut foundation = CanonicalHirFoundation::empty();
        foundation.set_types(vec![object_type, enum_type]).unwrap();
        foundation
            .set_object_values(vec![object_value.clone()])
            .unwrap();
        foundation.set_enum_variants(vec![variant.clone()]).unwrap();
        Self {
            coordinate,
            foundation,
            interface: interface(Vec::new(), nominals),
            outer: None,
            outer_key: None,
            outer_binding: None,
            nested: None,
            object_value: Some(object_value.id()),
            enum_variant: Some(variant.id()),
            external_types: Vec::new(),
        }
    }

    pub(super) fn identity(&self) -> ConeIdentity {
        self.coordinate.identity().unwrap()
    }
}

pub(super) fn import_foundation(
    session: &mut SemanticIdentitySession,
    fixture: &ProviderFixture,
    fingerprint: u8,
) -> ImportedHirFoundation {
    let decoded: DecodedHirFoundation =
        decode_canonical(&encode(&fixture.foundation).unwrap()).unwrap();
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(fixture.identity()).unwrap();
    for record in &fixture.external_types {
        pending
            .register_external_canonical_authority(record.clone())
            .unwrap();
    }
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
    ImportedHirFoundation::from_odr_free(
        OdrFreeHirFoundation::try_new(fixture.foundation.clone()).unwrap(),
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

fn nominal_key_with_kind(
    origin: ConeIdentity,
    name: &str,
    kind: SourceNominalKind,
) -> SourceDeclarationKey {
    SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            origin,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
        kind,
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
) -> NominalInterfaceRecordV1 {
    crate::nominal_interface_fixture::public_record(
        SourceNominalId::Concrete(declaration),
        PublicNominalKindV1::Class,
        CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
        CanonicalSignatureTypesV1::try_new(Vec::new()).unwrap(),
        CanonicalPersistentIdsV1::try_new(Vec::new()).unwrap(),
        CanonicalPublicMemberRefsV1::try_new(Vec::new()).unwrap(),
        CanonicalPersistentIdsV1::try_new(nested_bindings).unwrap(),
        NominalSourceShapeV1::Class(Default::default()),
    )
    .unwrap()
}

fn nominal_record_with_shape(
    declaration: PersistentTypeId,
    kind: PublicNominalKindV1,
    shape: NominalSourceShapeV1,
) -> NominalInterfaceRecordV1 {
    crate::nominal_interface_fixture::public_record(
        SourceNominalId::Concrete(declaration),
        kind,
        CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
        CanonicalSignatureTypesV1::try_new(Vec::new()).unwrap(),
        CanonicalPersistentIdsV1::try_new(Vec::new()).unwrap(),
        CanonicalPublicMemberRefsV1::try_new(Vec::new()).unwrap(),
        CanonicalPersistentIdsV1::try_new(Vec::new()).unwrap(),
        shape,
    )
    .unwrap()
}

fn interface(
    bindings: Vec<PublicExportBindingRecordV1>,
    nominals: Vec<NominalInterfaceRecordV1>,
) -> CrossConeHirInterfaceSectionV1 {
    CrossConeHirInterfaceSectionV1::new(
        CanonicalPublicExportBindingsV1::try_new(bindings).unwrap(),
        CanonicalNominalInterfacesV1::try_new(nominals).unwrap(),
        CanonicalCallableInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalPropertyInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalTypeAliasInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalCallableSourceInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalExportDefaultTemplatesV1::try_new(Vec::new()).unwrap(),
        CanonicalExportConstValuesV1::try_new(Vec::new()).unwrap(),
        CanonicalExportDefinitionSourcesV1::try_new(Vec::new()).unwrap(),
        CanonicalExternalHirReferencesV1::try_new(Vec::new()).unwrap(),
    )
}

pub(super) fn empty_alias_expansions() -> CanonicalTypeAliasExpansionsV1 {
    CanonicalTypeAliasInterfacesV1::try_new(Vec::new())
        .unwrap()
        .expand_alias_closure(&EmptyAliasAuthority, &WirePath::root())
        .unwrap()
}

struct EmptyAliasAuthority;

impl TypeAliasClosureAuthority for EmptyAliasAuthority {
    fn external_type_alias(
        &self,
        _alias: PersistentTypeAliasId,
    ) -> Option<&TypeAliasInterfaceRecordV1> {
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

pub(super) fn certificate(
    coordinate: &ConeCoordinate,
    fingerprint: u8,
) -> ImportedProviderCertificate {
    ImportedProviderCertificate::from_validated(
        coordinate.clone(),
        coordinate.identity().unwrap(),
        SemanticOriginFingerprint::new(
            [fingerprint; 32],
            [fingerprint.wrapping_add(1); 32],
            [fingerprint.wrapping_add(2); 32],
        ),
    )
}

pub(super) fn coordinate(name: &str) -> ConeCoordinate {
    ConeCoordinate::new("test", name, "1.0.0").unwrap()
}

pub(super) fn package(segments: &[&str]) -> PackagePath {
    PackagePath::from_segments(identifiers(segments))
}

pub(super) fn identifiers(segments: &[&str]) -> Vec<CanonicalIdentifier> {
    segments
        .iter()
        .map(|segment| CanonicalIdentifier::new(segment).unwrap())
        .collect()
}
