use std::collections::{BTreeMap, BTreeSet};

mod resources;

use scoop_identity::{
    BindableEntity, BindingTarget, CanonicalIdentifier, CborIdentityRecord, ConeCoordinate,
    ConeIdentity, DeclarationScope, DefinitionOwnerChain, ExportBindingKey, PackagePath,
    PersistentExportBindingId, PersistentTypeAliasId, SourceDeclarationKey, SourceDeclarationSite,
};

use super::*;
use crate::{
    CanonicalPublicExportBindingsV1, CanonicalReexportRoutesV1, ExportBindingSourceV1,
    PublicExportBindingRecordV1, ReexportRouteV1,
};

#[test]
fn validates_a_complete_dependency_binding_chain() {
    let fixture = chain();

    assert!(
        fixture
            .witness
            .validate_semantics(
                fixture.root,
                &fixture.authority,
                &mut route_meter(),
                &scoop_wire::WirePath::root()
            )
            .is_ok()
    );
}

#[test]
fn rejects_a_non_direct_start_and_an_overlong_route() {
    let mut fixture = chain();
    fixture.authority.direct.clear();
    assert_eq!(
        fixture.witness.validate_semantics(
            fixture.root,
            &fixture.authority,
            &mut route_meter(),
            &scoop_wire::WirePath::root()
        ),
        Err(
            DependencyBindingWitnessSemanticValidationError::ImmediateProviderNotDirect {
                provider: fixture.direct,
            }
        )
    );

    let mut fixture = chain();
    fixture.authority.closure_nodes = 1;
    assert_eq!(
        fixture.witness.validate_semantics(
            fixture.root,
            &fixture.authority,
            &mut route_meter(),
            &scoop_wire::WirePath::root()
        ),
        Err(
            DependencyBindingWitnessSemanticValidationError::RouteExceedsClosure {
                hops: 2,
                closure_nodes: 1,
            }
        )
    );
}

#[test]
fn every_hop_must_have_the_expected_canonical_binding_key() {
    let mut fixture = chain();
    fixture.authority.keys.remove(&fixture.terminal_binding);
    assert_eq!(
        fixture.witness.validate_semantics(
            fixture.root,
            &fixture.authority,
            &mut route_meter(),
            &scoop_wire::WirePath::root()
        ),
        Err(
            DependencyBindingWitnessSemanticValidationError::MissingHopBindingKey {
                hop: 1,
                binding: fixture.terminal_binding,
            }
        )
    );

    let fixture = chain();
    let (_, other_root) = alias(fixture.terminal, "Other");
    assert!(matches!(
        fixture
            .witness
            .validate_semantics(other_root, &fixture.authority, &mut route_meter(), &scoop_wire::WirePath::root()),
        Err(DependencyBindingWitnessSemanticValidationError::HopTargetMismatch {
            hop: 0,
            binding,
            ..
        }) if binding == fixture.direct_binding
    ));

    let mut fixture = chain();
    let wrong_exporter = binding(fixture.terminal, "middle", fixture.root);
    fixture
        .authority
        .keys
        .insert(fixture.direct_binding, wrong_exporter.key().clone());
    assert!(matches!(
        fixture
            .witness
            .validate_semantics(fixture.root, &fixture.authority, &mut route_meter(), &scoop_wire::WirePath::root()),
        Err(DependencyBindingWitnessSemanticValidationError::HopBindingExporterMismatch {
            hop: 0,
            binding,
            expected,
            actual,
        }) if binding == fixture.direct_binding
            && expected == fixture.direct
            && actual == fixture.terminal
    ));
}

#[test]
fn every_hop_must_exist_in_its_provider_surface() {
    let mut fixture = chain();
    fixture.authority.surfaces.remove(&fixture.terminal);
    assert_eq!(
        fixture.witness.validate_semantics(
            fixture.root,
            &fixture.authority,
            &mut route_meter(),
            &scoop_wire::WirePath::root()
        ),
        Err(
            DependencyBindingWitnessSemanticValidationError::MissingProviderSurface {
                hop: 1,
                provider: fixture.terminal,
            }
        )
    );

    let mut fixture = chain();
    fixture
        .authority
        .surfaces
        .insert(fixture.terminal, surface(Vec::new()));
    assert_eq!(
        fixture.witness.validate_semantics(
            fixture.root,
            &fixture.authority,
            &mut route_meter(),
            &scoop_wire::WirePath::root()
        ),
        Err(
            DependencyBindingWitnessSemanticValidationError::MissingProviderBinding {
                hop: 1,
                provider: fixture.terminal,
                binding: fixture.terminal_binding,
            }
        )
    );
}

#[test]
fn route_requires_reexport_intermediates_and_a_declared_terminal() {
    let mut fixture = chain();
    fixture.authority.surfaces.insert(
        fixture.direct,
        surface(vec![declared(fixture.direct_binding, fixture.target)]),
    );
    assert_eq!(
        fixture.witness.validate_semantics(
            fixture.root,
            &fixture.authority,
            &mut route_meter(),
            &scoop_wire::WirePath::root()
        ),
        Err(
            DependencyBindingWitnessSemanticValidationError::IntermediateIsDeclared {
                hop: 0,
                binding: fixture.direct_binding,
            }
        )
    );

    let mut fixture = chain();
    fixture.authority.surfaces.insert(
        fixture.terminal,
        surface(vec![reexport(
            fixture.terminal_binding,
            route(
                fixture.terminal,
                &[(fixture.terminal, fixture.terminal_binding)],
            ),
        )]),
    );
    assert_eq!(
        fixture.witness.validate_semantics(
            fixture.root,
            &fixture.authority,
            &mut route_meter(),
            &scoop_wire::WirePath::root()
        ),
        Err(
            DependencyBindingWitnessSemanticValidationError::TerminalIsReexport {
                hop: 1,
                binding: fixture.terminal_binding,
            }
        )
    );
}

#[test]
fn intermediate_reexport_must_publish_the_exact_remaining_suffix() {
    let mut fixture = chain();
    let alternate = binding(fixture.terminal, "alternate", fixture.root);
    fixture
        .authority
        .keys
        .insert(alternate.id(), alternate.key().clone());
    fixture.authority.surfaces.insert(
        fixture.direct,
        surface(vec![reexport(
            fixture.direct_binding,
            route(fixture.terminal, &[(fixture.terminal, alternate.id())]),
        )]),
    );

    assert_eq!(
        fixture.witness.validate_semantics(
            fixture.root,
            &fixture.authority,
            &mut route_meter(),
            &scoop_wire::WirePath::root()
        ),
        Err(
            DependencyBindingWitnessSemanticValidationError::MissingRouteSuffix {
                hop: 0,
                binding: fixture.direct_binding,
            }
        )
    );
}

#[test]
fn terminal_declared_source_must_match_the_binding_root() {
    let mut fixture = chain();
    let (other_alias, _) = alias(fixture.terminal, "Other");
    fixture.authority.surfaces.insert(
        fixture.terminal,
        surface(vec![declared(
            fixture.terminal_binding,
            BindableEntity::TypeAlias(other_alias),
        )]),
    );

    assert!(matches!(
        fixture
            .witness
            .validate_semantics(fixture.root, &fixture.authority, &mut route_meter(), &scoop_wire::WirePath::root()),
        Err(DependencyBindingWitnessSemanticValidationError::DeclaredTargetMismatch {
            hop: 1,
            binding,
            ..
        }) if binding == fixture.terminal_binding
    ));
}

struct ChainFixture {
    direct: ConeIdentity,
    terminal: ConeIdentity,
    root: BindingTarget,
    target: BindableEntity,
    direct_binding: PersistentExportBindingId,
    terminal_binding: PersistentExportBindingId,
    witness: DependencyBindingWitnessV1,
    authority: TestAuthority,
}

fn chain() -> ChainFixture {
    let direct = cone("direct");
    let terminal = cone("terminal");
    let (_, root) = alias(terminal, "Target");
    let target = root.target();
    let terminal_binding = binding(terminal, "target", root);
    let direct_binding = binding(direct, "middle", root);
    let suffix = route(terminal, &[(terminal, terminal_binding.id())]);
    let witness = DependencyBindingWitnessV1::new(route(
        direct,
        &[
            (direct, direct_binding.id()),
            (terminal, terminal_binding.id()),
        ],
    ));
    let authority = TestAuthority {
        closure_nodes: 3,
        direct: BTreeSet::from([direct]),
        keys: BTreeMap::from([
            (direct_binding.id(), direct_binding.key().clone()),
            (terminal_binding.id(), terminal_binding.key().clone()),
        ]),
        surfaces: BTreeMap::from([
            (direct, surface(vec![reexport(direct_binding.id(), suffix)])),
            (
                terminal,
                surface(vec![declared(terminal_binding.id(), target)]),
            ),
        ]),
    };
    ChainFixture {
        direct,
        terminal,
        root,
        target,
        direct_binding: direct_binding.id(),
        terminal_binding: terminal_binding.id(),
        witness,
        authority,
    }
}

#[derive(Default)]
struct TestAuthority {
    closure_nodes: usize,
    direct: BTreeSet<ConeIdentity>,
    keys: BTreeMap<PersistentExportBindingId, ExportBindingKey>,
    surfaces: BTreeMap<ConeIdentity, CanonicalPublicExportBindingsV1>,
}

impl PublicExportBindingClosureAuthority for TestAuthority {
    fn closure_node_count(&self) -> usize {
        self.closure_nodes
    }

    fn is_direct_dependency(
        &self,
        provider: ConeIdentity,
        meter: &mut scoop_wire::BudgetMeter,
        path: &scoop_wire::WirePath,
    ) -> Result<bool, scoop_wire::WireError> {
        meter.charge_work(1, path)?;
        Ok(self.direct.contains(&provider))
    }

    fn binding_key(
        &self,
        binding: PersistentExportBindingId,
        meter: &mut scoop_wire::BudgetMeter,
        path: &scoop_wire::WirePath,
    ) -> Result<Option<&ExportBindingKey>, scoop_wire::WireError> {
        meter.charge_work(1, path)?;
        Ok(self.keys.get(&binding))
    }

    fn public_bindings(
        &self,
        exporter: ConeIdentity,
        meter: &mut scoop_wire::BudgetMeter,
        path: &scoop_wire::WirePath,
    ) -> Result<Option<&CanonicalPublicExportBindingsV1>, scoop_wire::WireError> {
        meter.charge_work(1, path)?;
        Ok(self.surfaces.get(&exporter))
    }
}

fn alias(origin: ConeIdentity, name: &str) -> (PersistentTypeAliasId, BindingTarget) {
    let declaration = SourceDeclarationKey::type_alias(
        SourceDeclarationSite::new(
            origin,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
    );
    (
        PersistentTypeAliasId::from_source_declaration(&declaration).unwrap(),
        BindingTarget::type_alias(&declaration).unwrap(),
    )
}

fn binding(
    exporter: ConeIdentity,
    name: &str,
    target: BindingTarget,
) -> CborIdentityRecord<PersistentExportBindingId, ExportBindingKey> {
    CborIdentityRecord::from_key(ExportBindingKey::new(
        exporter,
        PackagePath::root(),
        CanonicalIdentifier::new(name).unwrap(),
        target,
    ))
    .unwrap()
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

fn reexport(
    binding: PersistentExportBindingId,
    route: ReexportRouteV1,
) -> PublicExportBindingRecordV1 {
    PublicExportBindingRecordV1::new(
        binding,
        ExportBindingSourceV1::Reexport {
            routes: CanonicalReexportRoutesV1::try_new(vec![route]).unwrap(),
        },
    )
}

fn surface(records: Vec<PublicExportBindingRecordV1>) -> CanonicalPublicExportBindingsV1 {
    CanonicalPublicExportBindingsV1::try_new(records).unwrap()
}

fn route(
    immediate_provider: ConeIdentity,
    hops: &[(ConeIdentity, PersistentExportBindingId)],
) -> ReexportRouteV1 {
    ReexportRouteV1::try_new(
        immediate_provider,
        hops.iter()
            .map(|(exporter, binding)| ReexportRouteHopV1::new(*exporter, *binding))
            .collect(),
    )
    .unwrap()
}

fn cone(name: &str) -> ConeIdentity {
    ConeCoordinate::new("example", name, "1.0.0")
        .unwrap()
        .identity()
        .unwrap()
}

fn route_meter() -> scoop_wire::BudgetMeter {
    scoop_wire::BudgetMeter::new(scoop_wire::DecodeLimits::default())
}
