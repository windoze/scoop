use std::collections::{BTreeMap, BTreeSet};

use scoop_identity::{
    BindingTarget, CanonicalIdentifier, CborIdentityRecord, ConeCoordinate, ConeIdentity,
    DeclarationScope, DefinitionOwnerChain, ExportBindingKey, PackagePath,
    PersistentExportBindingId, PersistentFunctionId, SourceDeclarationKey, SourceDeclarationSite,
};
use scoop_wire::WirePath;

use super::*;
use crate::{
    CanonicalDependencyBindingWitnessesV1, CanonicalExternalHirReferenceRolesV1,
    CanonicalReexportRoutesV1, DependencyBindingWitnessV1, ExportBindingSourceV1,
    ExternalHirReferenceV1, PublicExportBindingClosureAuthority, PublicExportBindingRecordV1,
    ReexportRouteHopV1,
};

#[test]
fn accepts_the_exact_reexport_target_and_route_union() {
    let fixture = Fixture::new();
    let bindings = fixture.bindings(vec![fixture.direct_route.clone()]);
    let references = fixture.references(
        &[ExternalHirReferenceRoleV1::ReexportTarget],
        vec![fixture.direct_route.clone()],
        fixture.provider,
    );

    assert_eq!(fixture.validate(&references, &bindings), Ok(()));
}

#[test]
fn rejects_missing_target_role_and_origin() {
    let fixture = Fixture::new();
    let bindings = fixture.bindings(vec![fixture.direct_route.clone()]);

    assert!(matches!(
        fixture.validate(
            &CanonicalExternalHirReferencesV1::try_new(Vec::new()).unwrap(),
            &bindings,
        ),
        Err(ExternalHirReexportClosureValidationError::MissingReference {
            binding_index: 0,
            target,
        }) if target == fixture.target
    ));

    let missing_role = fixture.references(
        &[ExternalHirReferenceRoleV1::AliasTarget],
        vec![fixture.direct_route.clone()],
        fixture.provider,
    );
    assert!(matches!(
        fixture.validate(&missing_role, &bindings),
        Err(ExternalHirReexportClosureValidationError::MissingRole {
            binding_index: 0,
            record_index: 0,
            target,
        }) if target == fixture.target
    ));

    let wrong_origin = fixture.references(
        &[ExternalHirReferenceRoleV1::ReexportTarget],
        vec![fixture.direct_route.clone()],
        fixture.alternate,
    );
    assert!(matches!(
        fixture.validate(&wrong_origin, &bindings),
        Err(ExternalHirReexportClosureValidationError::OriginMismatch {
            binding_index: 0,
            record_index: 0,
            target,
            expected,
            actual,
        }) if target == fixture.target
            && expected == fixture.provider
            && actual == fixture.alternate
    ));
}

#[test]
fn rejects_missing_and_unattributed_witnesses() {
    let fixture = Fixture::new();
    let bindings = fixture.bindings(vec![fixture.direct_route.clone()]);
    let wrong_witness = fixture.references(
        &[ExternalHirReferenceRoleV1::ReexportTarget],
        vec![fixture.alternate_route.clone()],
        fixture.provider,
    );
    assert!(matches!(
        fixture.validate(&wrong_witness, &bindings),
        Err(ExternalHirReexportClosureValidationError::MissingWitness {
            binding_index: 0,
            route_index: 0,
            record_index: 0,
            target,
        }) if target == fixture.target
    ));

    let extra_witness = fixture.references(
        &[ExternalHirReferenceRoleV1::ReexportTarget],
        vec![
            fixture.direct_route.clone(),
            fixture.alternate_route.clone(),
        ],
        fixture.provider,
    );
    let witness_index = extra_witness.records()[0]
        .witnesses()
        .witnesses()
        .binary_search_by(|witness| witness.route().cmp(&fixture.alternate_route))
        .unwrap();
    assert!(matches!(
        fixture.validate(&extra_witness, &bindings),
        Err(ExternalHirReexportClosureValidationError::ExtraWitness {
            record_index: 0,
            witness_index: actual,
            target,
        }) if actual == witness_index && target == fixture.target
    ));
}

#[test]
fn permits_a_valid_witness_attributed_to_another_source_name_role() {
    let fixture = Fixture::new();
    let bindings = fixture.bindings(vec![fixture.direct_route.clone()]);
    let references = fixture.references(
        &[
            ExternalHirReferenceRoleV1::ReexportTarget,
            ExternalHirReferenceRoleV1::DefaultDependency,
        ],
        vec![
            fixture.direct_route.clone(),
            fixture.alternate_route.clone(),
        ],
        fixture.provider,
    );

    assert_eq!(fixture.validate(&references, &bindings), Ok(()));
}

#[test]
fn rejects_an_unobserved_reexport_role() {
    let fixture = Fixture::new();
    let references = fixture.references(
        &[ExternalHirReferenceRoleV1::ReexportTarget],
        vec![fixture.direct_route.clone()],
        fixture.provider,
    );

    assert!(matches!(
        fixture.validate(
            &references,
            &CanonicalPublicExportBindingsV1::try_new(Vec::new()).unwrap(),
        ),
        Err(ExternalHirReexportClosureValidationError::ExtraRole {
            record_index: 0,
            target,
        }) if target == fixture.target
    ));
}

#[test]
fn rejects_missing_binding_authority_and_current_cone_targets() {
    let mut fixture = Fixture::new();
    let bindings = fixture.bindings(vec![fixture.direct_route.clone()]);
    let references = fixture.references(
        &[ExternalHirReferenceRoleV1::ReexportTarget],
        vec![fixture.direct_route.clone()],
        fixture.provider,
    );
    fixture.authority.keys.remove(&fixture.current_binding);
    assert!(matches!(
        fixture.validate(&references, &bindings),
        Err(ExternalHirReexportClosureValidationError::MissingBindingKey {
            binding_index: 0,
            binding,
        }) if binding == fixture.current_binding
    ));

    let mut fixture = Fixture::new();
    fixture.authority.target_origin = fixture.current;
    let bindings = fixture.bindings(vec![fixture.direct_route.clone()]);
    let references = fixture.references(
        &[ExternalHirReferenceRoleV1::ReexportTarget],
        vec![fixture.direct_route.clone()],
        fixture.provider,
    );
    assert!(matches!(
        fixture.validate(&references, &bindings),
        Err(ExternalHirReexportClosureValidationError::CurrentConeTarget {
            binding_index: 0,
            target,
            current,
        }) if target == fixture.target && current == fixture.current
    ));
}

struct Fixture {
    current: ConeIdentity,
    provider: ConeIdentity,
    alternate: ConeIdentity,
    target: ExternalHirTargetV1,
    current_binding: PersistentExportBindingId,
    direct_route: ReexportRouteV1,
    alternate_route: ReexportRouteV1,
    authority: Authority,
}

impl Fixture {
    fn new() -> Self {
        let current = cone("example", "current");
        let provider = cone("example", "provider");
        let alternate = cone("example", "alternate");
        let function = function(provider, "target");
        let root = BindingTarget::function(function.key()).unwrap();
        let target = ExternalHirTargetV1::from(root.target());
        let provider_binding = binding(provider, "target", root);
        let alternate_binding = binding(alternate, "target", root);
        let current_binding = binding(current, "facade", root);
        let direct_route = route(provider, &[(provider, provider_binding.id())]);
        let alternate_route = route(
            alternate,
            &[
                (alternate, alternate_binding.id()),
                (provider, provider_binding.id()),
            ],
        );
        let authority = Authority {
            current,
            target,
            target_origin: provider,
            root,
            direct: BTreeSet::from([provider, alternate]),
            keys: BTreeMap::from([
                (provider_binding.id(), provider_binding.key().clone()),
                (alternate_binding.id(), alternate_binding.key().clone()),
                (current_binding.id(), current_binding.key().clone()),
            ]),
        };
        Self {
            current,
            provider,
            alternate,
            target,
            current_binding: current_binding.id(),
            direct_route,
            alternate_route,
            authority,
        }
    }

    fn bindings(&self, routes: Vec<ReexportRouteV1>) -> CanonicalPublicExportBindingsV1 {
        CanonicalPublicExportBindingsV1::try_new(vec![PublicExportBindingRecordV1::new(
            self.current_binding,
            ExportBindingSourceV1::Reexport {
                routes: CanonicalReexportRoutesV1::try_new(routes).unwrap(),
            },
        )])
        .unwrap()
    }

    fn references(
        &self,
        roles: &[ExternalHirReferenceRoleV1],
        routes: Vec<ReexportRouteV1>,
        origin: ConeIdentity,
    ) -> CanonicalExternalHirReferencesV1 {
        let roles = CanonicalExternalHirReferenceRolesV1::try_new(roles.to_vec()).unwrap();
        let witnesses = CanonicalDependencyBindingWitnessesV1::try_new(
            routes
                .into_iter()
                .map(DependencyBindingWitnessV1::new)
                .collect(),
        )
        .unwrap();
        CanonicalExternalHirReferencesV1::try_new(vec![
            ExternalHirReferenceV1::try_new(
                origin,
                self.target,
                roles,
                witnesses,
                Default::default(),
                Default::default(),
            )
            .unwrap(),
        ])
        .unwrap()
    }

    fn validate(
        &self,
        references: &CanonicalExternalHirReferencesV1,
        bindings: &CanonicalPublicExportBindingsV1,
    ) -> Result<(), ExternalHirReexportClosureValidationError<AuthorityError>> {
        let mut authority = self.authority.clone();
        references.validate_reexport_closure(bindings, &mut authority, &WirePath::root().field(10))
    }
}

#[derive(Clone)]
struct Authority {
    current: ConeIdentity,
    target: ExternalHirTargetV1,
    target_origin: ConeIdentity,
    root: BindingTarget,
    direct: BTreeSet<ConeIdentity>,
    keys: BTreeMap<PersistentExportBindingId, ExportBindingKey>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AuthorityError;

impl fmt::Display for AuthorityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("missing test authority")
    }
}

impl std::error::Error for AuthorityError {}

impl ExternalHirReferenceSemanticAuthority<AuthorityError> for Authority {
    fn current_cone(&self) -> ConeIdentity {
        self.current
    }

    fn external_hir_target_origin(
        &mut self,
        target: ExternalHirTargetV1,
    ) -> Result<ConeIdentity, AuthorityError> {
        (target == self.target)
            .then_some(self.target_origin)
            .ok_or(AuthorityError)
    }

    fn external_hir_target_binding_root(
        &mut self,
        target: ExternalHirTargetV1,
    ) -> Result<BindingTarget, AuthorityError> {
        (target == self.target)
            .then_some(self.root)
            .ok_or(AuthorityError)
    }
}

impl PublicExportBindingClosureAuthority for Authority {
    fn closure_node_count(&self) -> usize {
        3
    }

    fn is_direct_dependency(&self, provider: ConeIdentity) -> bool {
        self.direct.contains(&provider)
    }

    fn binding_key(&self, binding: PersistentExportBindingId) -> Option<&ExportBindingKey> {
        self.keys.get(&binding)
    }

    fn public_bindings(&self, _exporter: ConeIdentity) -> Option<&CanonicalPublicExportBindingsV1> {
        None
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

fn cone(group: &str, name: &str) -> ConeIdentity {
    ConeCoordinate::new(group, name, "1.0.0")
        .unwrap()
        .identity()
        .unwrap()
}
