use scoop_hir::{
    CanonicalCallableInterfacesV1, CanonicalCallableSourceInterfacesV1,
    CanonicalExportConstValuesV1, CanonicalExportDefaultTemplatesV1,
    CanonicalExportDefinitionSourcesV1, CanonicalExternalHirReferencesV1,
    CanonicalNominalInterfacesV1, CanonicalPropertyInterfacesV1, CanonicalPublicExportBindingsV1,
    CanonicalReexportRoutesV1, CanonicalTypeAliasInterfacesV1, CrossConeHirInterfaceSectionV1,
    ExportBindingSourceV1, PublicExportBindingRecordV1, ReexportRouteHopV1, ReexportRouteV1,
};
use scoop_identity::{
    BindableEntity, BindingTarget, CanonicalIdentifier, CborIdentityRecord, ConeCoordinate,
    ConeIdentity, DeclarationScope, DefinitionOwnerChain, ExportBindingKey, PackagePath,
    PendingIdentityValidation, PersistentExportBindingId, PersistentFunctionId,
    SourceDeclarationKey, SourceDeclarationSite,
};

use super::*;

#[test]
fn adapter_validates_a_route_through_the_exact_provider_closure() {
    let fixture = route_fixture();
    let providers = [
        RouteProviderView {
            identity: fixture.direct,
            bindings: fixture.direct_interface.public_bindings(),
        },
        RouteProviderView {
            identity: fixture.terminal,
            bindings: fixture.terminal_interface.public_bindings(),
        },
    ];
    let direct = [fixture.direct];
    let authority = CanonicalCrossConeRouteAuthority::try_new(
        &fixture.identities,
        fixture.current_interface.public_bindings(),
        &direct,
        &providers,
        3,
    )
    .unwrap();

    fixture
        .current_interface
        .public_bindings()
        .validate_route_closure(fixture.current, &authority)
        .unwrap();
}

#[test]
fn adapter_does_not_treat_a_loaded_non_dependency_as_route_authority() {
    let fixture = route_fixture();
    let providers = [RouteProviderView {
        identity: fixture.direct,
        bindings: fixture.direct_interface.public_bindings(),
    }];
    let direct = [fixture.direct];
    let authority = CanonicalCrossConeRouteAuthority::try_new(
        &fixture.identities,
        fixture.current_interface.public_bindings(),
        &direct,
        &providers,
        2,
    )
    .unwrap();

    assert!(matches!(
        fixture
            .current_interface
            .public_bindings()
            .validate_route_closure(fixture.current, &authority),
        Err(PublicExportBindingClosureValidationError::MissingProviderSurface {
            hop: 1,
            provider,
            ..
        }) if provider == fixture.terminal
    ));
}

#[test]
fn adapter_uses_the_artifact_local_direct_dependency_set() {
    let fixture = route_fixture();
    let providers = [
        RouteProviderView {
            identity: fixture.direct,
            bindings: fixture.direct_interface.public_bindings(),
        },
        RouteProviderView {
            identity: fixture.terminal,
            bindings: fixture.terminal_interface.public_bindings(),
        },
    ];
    let authority = CanonicalCrossConeRouteAuthority::try_new(
        &fixture.identities,
        fixture.current_interface.public_bindings(),
        &[],
        &providers,
        3,
    )
    .unwrap();

    assert!(matches!(
        fixture
            .current_interface
            .public_bindings()
            .validate_route_closure(fixture.current, &authority),
        Err(PublicExportBindingClosureValidationError::ImmediateProviderNotDirect {
            provider,
            ..
        }) if provider == fixture.direct
    ));
}

struct RouteFixture {
    current: ConeIdentity,
    direct: ConeIdentity,
    terminal: ConeIdentity,
    identities: ValidatedIdentityGraph,
    current_interface: CrossConeHirInterfaceSectionV1,
    direct_interface: CrossConeHirInterfaceSectionV1,
    terminal_interface: CrossConeHirInterfaceSectionV1,
}

fn route_fixture() -> RouteFixture {
    let current = cone("current");
    let direct = cone("direct");
    let terminal = cone("terminal");
    let function =
        CborIdentityRecord::<PersistentFunctionId, _>::from_key(SourceDeclarationKey::function(
            SourceDeclarationSite::new(
                terminal,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new("target").unwrap(),
            0,
            None,
            Vec::new(),
        ))
        .unwrap();
    let target = BindableEntity::Function(function.id());
    let terminal_binding = binding(terminal, "target", function.key());
    let direct_binding = binding(direct, "middle", function.key());
    let current_binding = binding(current, "facade", function.key());

    let terminal_interface = interface(vec![PublicExportBindingRecordV1::new(
        terminal_binding.id(),
        ExportBindingSourceV1::DeclaredCurrent {
            declaration: target,
        },
    )]);
    let direct_route = ReexportRouteV1::try_new(
        terminal,
        vec![ReexportRouteHopV1::new(terminal, terminal_binding.id())],
    )
    .unwrap();
    let direct_interface = interface(vec![PublicExportBindingRecordV1::new(
        direct_binding.id(),
        ExportBindingSourceV1::Reexport {
            routes: CanonicalReexportRoutesV1::try_new(vec![direct_route]).unwrap(),
        },
    )]);
    let current_route = ReexportRouteV1::try_new(
        direct,
        vec![
            ReexportRouteHopV1::new(direct, direct_binding.id()),
            ReexportRouteHopV1::new(terminal, terminal_binding.id()),
        ],
    )
    .unwrap();
    let current_interface = interface(vec![PublicExportBindingRecordV1::new(
        current_binding.id(),
        ExportBindingSourceV1::Reexport {
            routes: CanonicalReexportRoutesV1::try_new(vec![current_route]).unwrap(),
        },
    )]);

    let mut pending = PendingIdentityValidation::new();
    for record in [terminal_binding, direct_binding, current_binding] {
        pending
            .register_external_canonical_authority(record)
            .unwrap();
    }

    RouteFixture {
        current,
        direct,
        terminal,
        identities: pending.finish().unwrap(),
        current_interface,
        direct_interface,
        terminal_interface,
    }
}

fn binding(
    exporter: ConeIdentity,
    name: &str,
    target: &SourceDeclarationKey,
) -> CborIdentityRecord<PersistentExportBindingId, ExportBindingKey> {
    CborIdentityRecord::from_key(ExportBindingKey::new(
        exporter,
        PackagePath::root(),
        CanonicalIdentifier::new(name).unwrap(),
        BindingTarget::function(target).unwrap(),
    ))
    .unwrap()
}

fn interface(bindings: Vec<PublicExportBindingRecordV1>) -> CrossConeHirInterfaceSectionV1 {
    CrossConeHirInterfaceSectionV1::new(
        CanonicalPublicExportBindingsV1::try_new(bindings).unwrap(),
        CanonicalNominalInterfacesV1::try_new(Vec::new()).unwrap(),
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

fn cone(name: &str) -> ConeIdentity {
    ConeCoordinate::new("test", name, "1.0.0")
        .unwrap()
        .identity()
        .unwrap()
}
