use scoop_hir::{
    CanonicalCallableInterfacesV1, CanonicalCallableSourceInterfacesV1,
    CanonicalDependencyBindingWitnessesV1, CanonicalExportConstValuesV1,
    CanonicalExportDefaultTemplatesV1, CanonicalExportDefinitionSourcesV1,
    CanonicalExternalHirReferenceRolesV1, CanonicalExternalHirReferencesV1,
    CanonicalNominalInterfacesV1, CanonicalPropertyInterfacesV1, CanonicalPublicExportBindingsV1,
    CanonicalTypeAliasInterfacesV1, CrossConeHirInterfaceSectionV1, ExportBindingSourceV1,
    ExportDefinitionSourceV1, ExternalHirReferenceRoleV1, ExternalHirReferenceV1,
    ExternalHirTargetV1, PublicExportBindingRecordV1, PublicLookupAccessV1,
    TypeAliasInterfaceRecordV1, TypeAliasTargetV1,
};
use scoop_identity::{
    BindableEntity, BindingTarget, CanonicalIdentifier, CborIdentityRecord, ConeCoordinate,
    ConeIdentity, CoreBuiltinNominal, DeclarationScope, DefinitionOrigin, DefinitionOwnerChain,
    ExportBindingKey, NormalizedSourcePath, PackagePath, PendingIdentityValidation,
    PersistentExportBindingId, PersistentTypeAliasId, SourceContextKey, SourceDeclarationKey,
    SourceDeclarationSite, SourceIdentity, SourceSpan,
};
use scoop_wire::WirePath;

use super::*;
use crate::cross_cone_closure::route_validation::{
    CanonicalCrossConeRouteAuthority, RouteProviderView,
};

#[test]
fn expands_a_foreign_alias_from_its_resolved_typed_target() {
    let fixture = foreign_alias_fixture();
    let providers = [RouteProviderView {
        identity: fixture.provider,
        bindings: fixture.provider_interface.public_bindings(),
    }];
    let direct = [fixture.provider];
    let mut route_authority = CanonicalCrossConeRouteAuthority::try_new(
        fixture.current,
        &fixture.identities,
        &fixture.current_interface,
        &direct,
        &providers,
        &scoop_wire::WirePath::root(),
    )
    .unwrap();

    let path = WirePath::root();
    fixture
        .current_interface
        .validate_external_reference_closure(&mut route_authority, &path)
        .unwrap();
    validate_alias_targets(
        fixture.current,
        &fixture.current_interface,
        &fixture.identities,
    )
    .unwrap();
    let external = fixture
        .provider_interface
        .type_aliases()
        .expand_alias_closure(&[], &path)
        .unwrap();
    let expansions = fixture
        .current_interface
        .type_aliases()
        .expand_alias_closure(&[&external], &path.field(5))
        .unwrap();

    assert_eq!(
        expansions.get(fixture.current_alias).unwrap().target(),
        &scoop_identity::SignatureTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id())
    );
}

#[test]
fn a_resolved_alias_target_can_come_from_a_support_provider() {
    let fixture = foreign_alias_fixture();
    let providers = [RouteProviderView {
        identity: fixture.provider,
        bindings: fixture.provider_interface.public_bindings(),
    }];
    let mut route_authority = CanonicalCrossConeRouteAuthority::try_new(
        fixture.current,
        &fixture.identities,
        &fixture.current_interface,
        &[],
        &providers,
        &scoop_wire::WirePath::root(),
    )
    .unwrap();

    fixture
        .current_interface
        .validate_external_reference_closure(&mut route_authority, &WirePath::root())
        .unwrap();
    validate_alias_targets(
        fixture.current,
        &fixture.current_interface,
        &fixture.identities,
    )
    .unwrap();
}

#[test]
fn rejects_a_same_cone_alias_target_absent_from_the_public_alias_table() {
    let current = cone("local");
    let source = alias_identity(current, "Source");
    let hidden = alias_identity(current, "Hidden");
    let hidden_id = hidden.id();
    let source_binding = binding(current, "Source", source.key());
    let interface = interface(
        vec![PublicExportBindingRecordV1::new(
            source_binding.id(),
            ExportBindingSourceV1::DeclaredCurrent {
                declaration: BindableEntity::TypeAlias(source.id()),
            },
        )],
        vec![alias_record(
            source.id(),
            TypeAliasTargetV1::Alias(hidden.id()),
            current,
            "Source.scoop",
        )],
        Vec::new(),
    );
    let identities = identity_graph([source, hidden], [source_binding]);
    assert!(matches!(
        validate_alias_targets(current, &interface, &identities),
        Err(CrossConeHirAliasReferenceError::MissingCurrentPublicTarget {
            source: actual_source,
            target,
        }) if actual_source == source_id(current, "Source") && target == hidden_id
    ));
}

struct ForeignAliasFixture {
    current: ConeIdentity,
    provider: ConeIdentity,
    current_alias: PersistentTypeAliasId,
    identities: scoop_identity::ValidatedIdentityGraph,
    current_interface: CrossConeHirInterfaceSectionV1,
    provider_interface: CrossConeHirInterfaceSectionV1,
}

fn foreign_alias_fixture() -> ForeignAliasFixture {
    let current = cone("consumer");
    let provider = cone("provider");
    let current_alias = alias_identity(current, "CurrentAlias");
    let foreign_alias = alias_identity(provider, "ForeignAlias");
    let current_binding = binding(current, "CurrentAlias", current_alias.key());
    let foreign_binding = binding(provider, "ForeignAlias", foreign_alias.key());
    let current_interface = interface(
        vec![PublicExportBindingRecordV1::new(
            current_binding.id(),
            ExportBindingSourceV1::DeclaredCurrent {
                declaration: BindableEntity::TypeAlias(current_alias.id()),
            },
        )],
        vec![alias_record(
            current_alias.id(),
            TypeAliasTargetV1::Alias(foreign_alias.id()),
            current,
            "CurrentAlias.scoop",
        )],
        vec![
            ExternalHirReferenceV1::try_new(
                provider,
                ExternalHirTargetV1::TypeAlias(foreign_alias.id()),
                CanonicalExternalHirReferenceRolesV1::try_new(vec![
                    ExternalHirReferenceRoleV1::AliasTarget,
                ])
                .unwrap(),
                CanonicalDependencyBindingWitnessesV1::try_new(Vec::new()).unwrap(),
                Default::default(),
                Default::default(),
            )
            .unwrap(),
        ],
    );
    let provider_interface = interface(
        vec![PublicExportBindingRecordV1::new(
            foreign_binding.id(),
            ExportBindingSourceV1::DeclaredCurrent {
                declaration: BindableEntity::TypeAlias(foreign_alias.id()),
            },
        )],
        vec![alias_record(
            foreign_alias.id(),
            TypeAliasTargetV1::Signature(scoop_identity::SignatureTypeKey::Nominal(
                CoreBuiltinNominal::Unit.identity_record().id(),
            )),
            provider,
            "ForeignAlias.scoop",
        )],
        Vec::new(),
    );
    let identities = identity_graph(
        [current_alias.clone(), foreign_alias.clone()],
        [current_binding, foreign_binding],
    );

    ForeignAliasFixture {
        current,
        provider,
        current_alias: current_alias.id(),
        identities,
        current_interface,
        provider_interface,
    }
}

fn identity_graph(
    aliases: impl IntoIterator<Item = CborIdentityRecord<PersistentTypeAliasId, SourceDeclarationKey>>,
    bindings: impl IntoIterator<Item = CborIdentityRecord<PersistentExportBindingId, ExportBindingKey>>,
) -> scoop_identity::ValidatedIdentityGraph {
    let mut pending = PendingIdentityValidation::new();
    for alias in aliases {
        pending
            .register_external_canonical_authority(alias)
            .unwrap();
    }
    for binding in bindings {
        pending
            .register_external_canonical_authority(binding)
            .unwrap();
    }
    pending.finish().unwrap()
}

fn alias_identity(
    cone: ConeIdentity,
    name: &str,
) -> CborIdentityRecord<PersistentTypeAliasId, SourceDeclarationKey> {
    CborIdentityRecord::from_key(SourceDeclarationKey::type_alias(
        SourceDeclarationSite::new(
            cone,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
    ))
    .unwrap()
}

fn source_id(cone: ConeIdentity, name: &str) -> PersistentTypeAliasId {
    alias_identity(cone, name).id()
}

fn binding(
    exporter: ConeIdentity,
    name: &str,
    alias: &SourceDeclarationKey,
) -> CborIdentityRecord<PersistentExportBindingId, ExportBindingKey> {
    CborIdentityRecord::from_key(ExportBindingKey::new(
        exporter,
        PackagePath::root(),
        CanonicalIdentifier::new(name).unwrap(),
        BindingTarget::type_alias(alias).unwrap(),
    ))
    .unwrap()
}

fn alias_record(
    alias: PersistentTypeAliasId,
    target: TypeAliasTargetV1,
    cone: ConeIdentity,
    path: &str,
) -> TypeAliasInterfaceRecordV1 {
    TypeAliasInterfaceRecordV1::try_new(
        alias,
        target,
        PublicLookupAccessV1::DirectOnly,
        definition_source(cone, path),
    )
    .unwrap()
}

fn definition_source(cone: ConeIdentity, path: &str) -> ExportDefinitionSourceV1 {
    let source = SourceIdentity::new(cone, NormalizedSourcePath::new(path).unwrap()).unwrap();
    let context = SourceContextKey::File {
        source: source.clone(),
    };
    ExportDefinitionSourceV1::new(
        DefinitionOrigin::new(source, SourceSpan::new(0, 1).unwrap(), &context).unwrap(),
    )
}

fn interface(
    bindings: Vec<PublicExportBindingRecordV1>,
    aliases: Vec<TypeAliasInterfaceRecordV1>,
    references: Vec<ExternalHirReferenceV1>,
) -> CrossConeHirInterfaceSectionV1 {
    let definitions = aliases
        .iter()
        .map(|alias| alias.definition_origin().clone())
        .collect();
    CrossConeHirInterfaceSectionV1::new(
        CanonicalPublicExportBindingsV1::try_new(bindings).unwrap(),
        CanonicalNominalInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalCallableInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalPropertyInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalTypeAliasInterfacesV1::try_new(aliases).unwrap(),
        CanonicalCallableSourceInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalExportDefaultTemplatesV1::try_new(Vec::new()).unwrap(),
        CanonicalExportConstValuesV1::try_new(Vec::new()).unwrap(),
        CanonicalExportDefinitionSourcesV1::try_new(definitions).unwrap(),
        CanonicalExternalHirReferencesV1::try_new(references).unwrap(),
        Default::default(),
        Default::default(),
        Default::default(),
    )
}

fn cone(name: &str) -> ConeIdentity {
    ConeCoordinate::new("test", name, "1.0.0")
        .unwrap()
        .identity()
        .unwrap()
}
