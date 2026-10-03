use std::collections::BTreeMap;

mod resources;

use scoop_identity::{
    BindableEntity, BindingTarget, CanonicalIdentifier, CborIdentityRecord, ConeCoordinate,
    ConeIdentity, DeclarationScope, DefinitionOwnerChain, ExportBindingKey, PackagePath,
    PersistentExportBindingId, PersistentFunctionId, SourceDeclarationKey, SourceDeclarationSite,
};

use super::*;
use crate::{PublicExportBindingRecordV1, ReexportRouteHopV1, ReexportRouteV1};

#[test]
fn validates_a_complete_reexport_chain_and_supports_binding_lookup() {
    let fixture = chain();

    fixture
        .current_surface
        .validate_route_closure(fixture.current, &fixture.authority)
        .unwrap();
    assert_eq!(
        fixture
            .current_surface
            .get(fixture.current_binding)
            .unwrap()
            .binding(),
        fixture.current_binding
    );
    let absent = binding(
        fixture.current,
        "absent",
        &function(fixture.current, "absent"),
    );
    assert!(fixture.current_surface.get(absent.id()).is_none());
}

#[test]
fn rejects_routes_longer_than_the_closure() {
    let mut fixture = chain();
    fixture.authority.closure_nodes = 1;
    assert!(matches!(
        fixture
            .current_surface
            .validate_route_closure(fixture.current, &fixture.authority),
        Err(PublicExportBindingClosureValidationError::RouteExceedsClosure {
            binding,
            route: 0,
            hops: 2,
            closure_nodes: 1,
        }) if binding == fixture.current_binding
    ));
}

#[test]
fn rejects_missing_or_inconsistent_hop_authority() {
    let mut fixture = chain();
    fixture.authority.keys.remove(&fixture.terminal_binding);

    assert!(matches!(
        fixture
            .current_surface
            .validate_route_closure(fixture.current, &fixture.authority),
        Err(PublicExportBindingClosureValidationError::MissingHopBindingKey {
            binding,
            route: 0,
            hop: 1,
            hop_binding,
        }) if binding == fixture.current_binding && hop_binding == fixture.terminal_binding
    ));

    let mut fixture = chain();
    fixture.authority.surfaces.remove(&fixture.terminal);
    assert!(matches!(
        fixture
            .current_surface
            .validate_route_closure(fixture.current, &fixture.authority),
        Err(PublicExportBindingClosureValidationError::MissingProviderSurface {
            binding,
            route: 0,
            hop: 1,
            provider,
        }) if binding == fixture.current_binding && provider == fixture.terminal
    ));

    let mut fixture = chain();
    let other = function(fixture.terminal, "other");
    let wrong = binding(fixture.terminal, "terminal", &other);
    fixture
        .authority
        .keys
        .insert(fixture.terminal_binding, wrong.key().clone());
    assert!(matches!(
        fixture
            .current_surface
            .validate_route_closure(fixture.current, &fixture.authority),
        Err(PublicExportBindingClosureValidationError::HopTargetMismatch {
            binding,
            route: 0,
            hop: 1,
            hop_binding,
            ..
        }) if binding == fixture.current_binding && hop_binding == fixture.terminal_binding
    ));
}

#[test]
fn requires_intermediate_reexports_and_a_declared_terminal() {
    let mut fixture = chain();
    fixture.authority.surfaces.insert(
        fixture.direct,
        surface(vec![PublicExportBindingRecordV1::new(
            fixture.direct_binding,
            ExportBindingSourceV1::DeclaredCurrent {
                declaration: fixture.target,
            },
        )]),
    );

    assert!(matches!(
        fixture
            .current_surface
            .validate_route_closure(fixture.current, &fixture.authority),
        Err(PublicExportBindingClosureValidationError::IntermediateIsDeclared {
            binding,
            route: 0,
            hop: 0,
            hop_binding,
        }) if binding == fixture.current_binding && hop_binding == fixture.direct_binding
    ));

    let mut fixture = chain();
    let invalid_terminal_route = route(fixture.direct, &[fixture.direct_binding]);
    fixture.authority.surfaces.insert(
        fixture.terminal,
        surface(vec![PublicExportBindingRecordV1::new(
            fixture.terminal_binding,
            ExportBindingSourceV1::Reexport {
                routes: CanonicalReexportRoutesV1::try_new(vec![invalid_terminal_route]).unwrap(),
            },
        )]),
    );

    assert!(matches!(
        fixture
            .current_surface
            .validate_route_closure(fixture.current, &fixture.authority),
        Err(PublicExportBindingClosureValidationError::TerminalIsReexport {
            binding,
            route: 0,
            hop: 1,
            hop_binding,
        }) if binding == fixture.current_binding && hop_binding == fixture.terminal_binding
    ));
}

#[test]
fn requires_the_intermediate_surface_to_publish_the_exact_suffix() {
    let mut fixture = chain();
    let alternate = binding(fixture.terminal, "alternate", &fixture.function);
    fixture
        .authority
        .keys
        .insert(alternate.id(), alternate.key().clone());
    fixture.authority.surfaces.insert(
        fixture.terminal,
        surface(vec![
            declared(fixture.terminal_binding, fixture.target),
            declared(alternate.id(), fixture.target),
        ]),
    );
    fixture.authority.surfaces.insert(
        fixture.direct,
        surface(vec![PublicExportBindingRecordV1::new(
            fixture.direct_binding,
            ExportBindingSourceV1::Reexport {
                routes: CanonicalReexportRoutesV1::try_new(vec![route(
                    fixture.terminal,
                    &[alternate.id()],
                )])
                .unwrap(),
            },
        )]),
    );

    assert!(matches!(
        fixture
            .current_surface
            .validate_route_closure(fixture.current, &fixture.authority),
        Err(PublicExportBindingClosureValidationError::MissingRouteSuffix {
            binding,
            route: 0,
            hop: 0,
            hop_binding,
        }) if binding == fixture.current_binding && hop_binding == fixture.direct_binding
    ));
}

#[test]
fn validates_current_binding_exporter_and_declared_target() {
    let fixture = chain();
    let wrong_current = cone("example:wrong-current:1.0.0");
    assert!(matches!(
        fixture
            .current_surface
            .validate_route_closure(wrong_current, &fixture.authority),
        Err(PublicExportBindingClosureValidationError::CurrentBindingExporterMismatch {
            binding,
            expected,
            actual,
        }) if binding == fixture.current_binding
            && expected == wrong_current
            && actual == fixture.current
    ));

    let other = function(fixture.terminal, "other_declaration");
    let direct = surface(vec![PublicExportBindingRecordV1::new(
        fixture.terminal_binding,
        ExportBindingSourceV1::DeclaredCurrent {
            declaration: BindableEntity::Function(other.id()),
        },
    )]);
    assert!(matches!(
        direct.validate_route_closure(fixture.terminal, &fixture.authority),
        Err(PublicExportBindingClosureValidationError::DeclaredTargetMismatch {
            exporter,
            binding,
            ..
        }) if exporter == fixture.terminal && binding == fixture.terminal_binding
    ));
}

struct ChainFixture {
    current: ConeIdentity,
    direct: ConeIdentity,
    terminal: ConeIdentity,
    function: CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey>,
    target: BindableEntity,
    current_binding: PersistentExportBindingId,
    direct_binding: PersistentExportBindingId,
    terminal_binding: PersistentExportBindingId,
    current_surface: CanonicalPublicExportBindingsV1,
    authority: TestAuthority,
}

fn chain() -> ChainFixture {
    let current = cone("example:current:1.0.0");
    let direct = cone("example:direct:1.0.0");
    let terminal = cone("example:terminal:1.0.0");
    let function = function(terminal, "target");
    let target = BindableEntity::Function(function.id());
    let terminal_binding = binding(terminal, "target", &function);
    let direct_binding = binding(direct, "middle", &function);
    let current_binding = binding(current, "facade", &function);

    let terminal_surface = surface(vec![declared(terminal_binding.id(), target)]);
    let direct_surface = surface(vec![PublicExportBindingRecordV1::new(
        direct_binding.id(),
        ExportBindingSourceV1::Reexport {
            routes: CanonicalReexportRoutesV1::try_new(vec![route(
                terminal,
                &[terminal_binding.id()],
            )])
            .unwrap(),
        },
    )]);
    let current_surface = surface(vec![PublicExportBindingRecordV1::new(
        current_binding.id(),
        ExportBindingSourceV1::Reexport {
            routes: CanonicalReexportRoutesV1::try_new(vec![
                ReexportRouteV1::try_new(
                    direct,
                    vec![
                        ReexportRouteHopV1::new(direct, direct_binding.id()),
                        ReexportRouteHopV1::new(terminal, terminal_binding.id()),
                    ],
                )
                .unwrap(),
            ])
            .unwrap(),
        },
    )]);

    let authority = TestAuthority {
        closure_nodes: 3,
        keys: BTreeMap::from([
            (current_binding.id(), current_binding.key().clone()),
            (direct_binding.id(), direct_binding.key().clone()),
            (terminal_binding.id(), terminal_binding.key().clone()),
        ]),
        surfaces: BTreeMap::from([(direct, direct_surface), (terminal, terminal_surface)]),
    };

    ChainFixture {
        current,
        direct,
        terminal,
        function,
        target,
        current_binding: current_binding.id(),
        direct_binding: direct_binding.id(),
        terminal_binding: terminal_binding.id(),
        current_surface,
        authority,
    }
}

fn declared(
    binding: PersistentExportBindingId,
    declaration: BindableEntity,
) -> PublicExportBindingRecordV1 {
    PublicExportBindingRecordV1::new(
        binding,
        ExportBindingSourceV1::DeclaredCurrent { declaration },
    )
}

fn surface(records: Vec<PublicExportBindingRecordV1>) -> CanonicalPublicExportBindingsV1 {
    CanonicalPublicExportBindingsV1::try_new(records).unwrap()
}

fn route(
    immediate_provider: ConeIdentity,
    bindings: &[PersistentExportBindingId],
) -> ReexportRouteV1 {
    ReexportRouteV1::try_new(
        immediate_provider,
        bindings
            .iter()
            .map(|binding| ReexportRouteHopV1::new(immediate_provider, *binding))
            .collect(),
    )
    .unwrap()
}

#[derive(Default)]
struct TestAuthority {
    closure_nodes: usize,
    keys: BTreeMap<PersistentExportBindingId, ExportBindingKey>,
    surfaces: BTreeMap<ConeIdentity, CanonicalPublicExportBindingsV1>,
}

impl PublicExportBindingClosureAuthority for TestAuthority {
    fn closure_node_count(&self) -> usize {
        self.closure_nodes
    }

    fn binding_key(&self, binding: PersistentExportBindingId) -> Option<&ExportBindingKey> {
        self.keys.get(&binding)
    }

    fn public_bindings(&self, exporter: ConeIdentity) -> Option<&CanonicalPublicExportBindingsV1> {
        self.surfaces.get(&exporter)
    }
}

fn function(
    cone: ConeIdentity,
    name: &str,
) -> CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey> {
    CborIdentityRecord::from_key(SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            cone,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
        0,
        None,
        Vec::new(),
    ))
    .unwrap()
}

fn binding(
    exporter: ConeIdentity,
    name: &str,
    function: &CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey>,
) -> CborIdentityRecord<PersistentExportBindingId, ExportBindingKey> {
    CborIdentityRecord::from_key(ExportBindingKey::new(
        exporter,
        PackagePath::root(),
        CanonicalIdentifier::new(name).unwrap(),
        BindingTarget::function(function.key()).unwrap(),
    ))
    .unwrap()
}

fn cone(value: &str) -> ConeIdentity {
    let mut parts = value.split(':');
    let group = parts.next().unwrap();
    let name = parts.next().unwrap();
    let version = parts.next().unwrap();
    assert!(parts.next().is_none());
    ConeCoordinate::new(group, name, version)
        .unwrap()
        .identity()
        .unwrap()
}
