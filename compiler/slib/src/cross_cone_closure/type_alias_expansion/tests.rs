use scoop_hir::{
    CanonicalCallableInterfacesV1, CanonicalCallableSourceInterfacesV1,
    CanonicalDependencyBindingWitnessesV1, CanonicalExportConstValuesV1,
    CanonicalExportDefaultTemplatesV1, CanonicalExportDefinitionSourcesV1,
    CanonicalExternalHirReferenceRolesV1, CanonicalExternalHirReferencesV1,
    CanonicalNominalInterfacesV1, CanonicalPropertyInterfacesV1, CanonicalPublicExportBindingsV1,
    CanonicalTypeAliasInterfacesV1, CrossConeHirExternalReferenceValidationError,
    CrossConeHirInterfaceSectionV1, DependencyBindingWitnessSemanticValidationError,
    DependencyBindingWitnessV1, ExportBindingSourceV1, ExportDefinitionSourceV1,
    ExternalHirReferenceRoleV1, ExternalHirReferenceSemanticValidationError,
    ExternalHirReferenceSetSemanticValidationError, ExternalHirReferenceV1, ExternalHirTargetV1,
    PublicExportBindingRecordV1, PublicLookupAccessV1, ReexportRouteHopV1, ReexportRouteV1,
    TypeAliasInterfaceRecordV1, TypeAliasTargetV1,
};
use scoop_identity::{
    BindableEntity, BindingTarget, CanonicalIdentifier, CborIdentityRecord, ConeCoordinate,
    ConeIdentity, CoreBuiltinNominal, DeclarationScope, DefinitionOrigin, DefinitionOwnerChain,
    ExportBindingKey, NormalizedSourcePath, PackagePath, PendingIdentityValidation,
    PersistentExportBindingId, PersistentTypeAliasId, SourceContextKey, SourceDeclarationKey,
    SourceDeclarationSite, SourceIdentity, SourceSpan,
};
use scoop_wire::{BudgetMeter, DecodeLimits, WirePath};

use super::*;
use crate::cross_cone_closure::route_validation::RouteProviderView;

#[test]
fn authorizes_and_expands_a_foreign_alias_through_a_direct_witness() {
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
        2,
    )
    .unwrap();
    let mut meter = BudgetMeter::new(DecodeLimits::default());
    let path = WirePath::root();
    fixture
        .current_interface
        .validate_external_reference_closure(&mut route_authority, &mut meter, &path)
        .unwrap();
    let authorized =
        validate_alias_authority(&fixture.current_interface, &mut route_authority).unwrap();
    assert_eq!(
        authorized,
        vec![(fixture.current_alias, fixture.foreign_alias)]
    );

    let external_authorized = [];
    let alias_providers = [AliasProviderView {
        interface: &fixture.provider_interface,
        authorized: &external_authorized,
    }];
    let authority =
        CanonicalTypeAliasClosureAuthority::try_new(&authorized, &alias_providers).unwrap();
    let expansions = fixture
        .current_interface
        .type_aliases()
        .expand_alias_closure(&authority, &mut meter, &path.field(5))
        .unwrap();

    assert_eq!(
        expansions.get(fixture.current_alias).unwrap().target(),
        &scoop_identity::SignatureTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id())
    );
}

#[test]
fn rejects_a_foreign_alias_witness_that_does_not_start_at_a_direct_provider() {
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
        2,
    )
    .unwrap();
    let mut meter = BudgetMeter::new(DecodeLimits::default());

    assert!(matches!(
        fixture.current_interface.validate_external_reference_closure(
            &mut route_authority,
            &mut meter,
            &WirePath::root(),
        ),
        Err(CrossConeHirExternalReferenceValidationError::Records(
            ExternalHirReferenceSetSemanticValidationError::Record { error, .. }
        )) if matches!(
            error.as_ref(),
            ExternalHirReferenceSemanticValidationError::Witness {
                error: DependencyBindingWitnessSemanticValidationError::ImmediateProviderNotDirect {
                    provider,
                },
                ..
            } if *provider == fixture.provider
        )
    ));
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
    let mut authority =
        CanonicalCrossConeRouteAuthority::try_new(current, &identities, &interface, &[], &[], 1)
            .unwrap();
    assert!(matches!(
        validate_alias_authority(&interface, &mut authority),
        Err(CrossConeHirAliasAuthorityValidationError::MissingCurrentPublicTarget {
            source: actual_source,
            target,
        }) if actual_source == source_id(current, "Source") && target == hidden_id
    ));
}

struct ForeignAliasFixture {
    current: ConeIdentity,
    provider: ConeIdentity,
    current_alias: PersistentTypeAliasId,
    foreign_alias: PersistentTypeAliasId,
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
    let witness_route = ReexportRouteV1::try_new(
        provider,
        vec![ReexportRouteHopV1::new(provider, foreign_binding.id())],
    )
    .unwrap();

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
                CanonicalDependencyBindingWitnessesV1::try_new(vec![
                    DependencyBindingWitnessV1::new(witness_route),
                ])
                .unwrap(),
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
        foreign_alias: foreign_alias.id(),
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
    )
}

fn cone(name: &str) -> ConeIdentity {
    ConeCoordinate::new("test", name, "1.0.0")
        .unwrap()
        .identity()
        .unwrap()
}
