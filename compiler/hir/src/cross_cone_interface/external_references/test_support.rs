use scoop_identity::{
    BindingTarget, CanonicalIdentifier, CborIdentityRecord, ConeCoordinate, ConeIdentity,
    DeclarationScope, DefinitionOwnerChain, ExportBindingKey, PackagePath,
    PendingIdentityValidation, PersistentExportBindingId, PersistentTypeAliasId,
    SourceDeclarationKey, SourceDeclarationSite, ValidatedIdentityGraph,
};

use super::{
    CanonicalDependencyBindingWitnessesV1, CanonicalExternalHirReferenceRolesV1,
    DependencyBindingWitnessV1, ExternalHirReferenceRoleV1, ExternalHirReferenceSemanticAuthority,
    ExternalHirReferenceV1, ExternalHirTargetV1,
};
use crate::{CanonicalPublicExportBindingsV1, PublicExportBindingClosureAuthority};
use crate::{ReexportRouteHopV1, ReexportRouteV1};

pub(super) struct Fixture {
    pub(super) provider: ConeIdentity,
    pub(super) first_alias: PersistentTypeAliasId,
    pub(super) second_alias: PersistentTypeAliasId,
    pub(super) first_root: BindingTarget,
    pub(super) first_route: ReexportRouteV1,
    pub(super) second_route: ReexportRouteV1,
}

impl Fixture {
    pub(super) fn new() -> Self {
        let provider = ConeCoordinate::new("example", "provider", "1.0.0")
            .unwrap()
            .identity()
            .unwrap();
        let (first_alias, first_binding, first_root) = alias_binding(provider, "First");
        let (second_alias, second_binding, _) = alias_binding(provider, "Second");
        Self {
            provider,
            first_alias,
            second_alias,
            first_root,
            first_route: route(provider, first_binding),
            second_route: route(provider, second_binding),
        }
    }

    pub(super) fn signature_reference(
        &self,
        alias: PersistentTypeAliasId,
    ) -> ExternalHirReferenceV1 {
        ExternalHirReferenceV1::try_new(
            self.provider,
            ExternalHirTargetV1::TypeAlias(alias),
            roles(&[ExternalHirReferenceRoleV1::SignatureDependency]),
            CanonicalDependencyBindingWitnessesV1::try_new(Vec::new()).unwrap(),
            Default::default(),
            Default::default(),
        )
        .unwrap()
    }

    pub(super) fn alias_reference(
        &self,
        alias: PersistentTypeAliasId,
        route: ReexportRouteV1,
    ) -> ExternalHirReferenceV1 {
        ExternalHirReferenceV1::try_new(
            self.provider,
            ExternalHirTargetV1::TypeAlias(alias),
            roles(&[ExternalHirReferenceRoleV1::ReexportTarget]),
            witnesses(vec![route]),
            Default::default(),
            Default::default(),
        )
        .unwrap()
    }

    pub(super) fn authority(&self) -> ValidatedIdentityGraph {
        let mut pending = PendingIdentityValidation::new();
        pending.register_authority(self.provider).unwrap();
        pending.register_authority(self.first_alias).unwrap();
        pending.register_authority(self.second_alias).unwrap();
        pending
            .register_authority(self.first_route.terminal().binding())
            .unwrap();
        pending
            .register_authority(self.second_route.terminal().binding())
            .unwrap();
        pending.finish().unwrap()
    }
}

pub(super) fn roles(values: &[ExternalHirReferenceRoleV1]) -> CanonicalExternalHirReferenceRolesV1 {
    CanonicalExternalHirReferenceRolesV1::try_new(values.to_vec()).unwrap()
}

pub(super) fn witnesses(routes: Vec<ReexportRouteV1>) -> CanonicalDependencyBindingWitnessesV1 {
    CanonicalDependencyBindingWitnessesV1::try_new(
        routes
            .into_iter()
            .map(DependencyBindingWitnessV1::new)
            .collect(),
    )
    .unwrap()
}

pub(super) struct TargetOriginAuthority {
    current: ConeIdentity,
    target_origin: ConeIdentity,
    binding_root: Option<BindingTarget>,
    failing_target: Option<ExternalHirTargetV1>,
    origin_queries: usize,
}

impl TargetOriginAuthority {
    pub(super) fn new(current: ConeIdentity, target_origin: ConeIdentity) -> Self {
        Self {
            current,
            target_origin,
            binding_root: None,
            failing_target: None,
            origin_queries: 0,
        }
    }

    pub(super) fn set_target_origin(&mut self, target_origin: ConeIdentity) {
        self.target_origin = target_origin;
    }

    pub(super) fn fail_on(&mut self, target: ExternalHirTargetV1) {
        self.failing_target = Some(target);
    }

    pub(super) fn set_binding_root(&mut self, binding_root: BindingTarget) {
        self.binding_root = Some(binding_root);
    }

    pub(super) const fn target_origin(&self) -> ConeIdentity {
        self.target_origin
    }

    pub(super) const fn origin_queries(&self) -> usize {
        self.origin_queries
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct TargetOriginAuthorityError;

impl std::fmt::Display for TargetOriginAuthorityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("missing external target origin")
    }
}

impl std::error::Error for TargetOriginAuthorityError {}

impl ExternalHirReferenceSemanticAuthority<TargetOriginAuthorityError> for TargetOriginAuthority {
    fn current_cone(&self) -> ConeIdentity {
        self.current
    }

    fn external_hir_target_origin(
        &mut self,
        target: ExternalHirTargetV1,
    ) -> Result<ConeIdentity, TargetOriginAuthorityError> {
        self.origin_queries += 1;
        if self.failing_target == Some(target) {
            Err(TargetOriginAuthorityError)
        } else {
            Ok(self.target_origin)
        }
    }

    fn external_hir_target_binding_root(
        &mut self,
        _target: ExternalHirTargetV1,
    ) -> Result<BindingTarget, TargetOriginAuthorityError> {
        self.binding_root.ok_or(TargetOriginAuthorityError)
    }
}

impl PublicExportBindingClosureAuthority for TargetOriginAuthority {
    fn closure_node_count(&self) -> usize {
        0
    }

    fn binding_key(&self, _binding: PersistentExportBindingId) -> Option<&ExportBindingKey> {
        None
    }

    fn public_bindings(&self, _exporter: ConeIdentity) -> Option<&CanonicalPublicExportBindingsV1> {
        None
    }
}

fn alias_binding(
    provider: ConeIdentity,
    name: &str,
) -> (
    PersistentTypeAliasId,
    PersistentExportBindingId,
    BindingTarget,
) {
    let declaration = SourceDeclarationKey::type_alias(
        SourceDeclarationSite::new(
            provider,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
    );
    let alias = PersistentTypeAliasId::from_source_declaration(&declaration).unwrap();
    let target = BindingTarget::type_alias(&declaration).unwrap();
    let binding =
        CborIdentityRecord::<PersistentExportBindingId, _>::from_key(ExportBindingKey::new(
            provider,
            PackagePath::root(),
            CanonicalIdentifier::new(name).unwrap(),
            target,
        ))
        .unwrap()
        .id();
    (alias, binding, target)
}

fn route(provider: ConeIdentity, binding: PersistentExportBindingId) -> ReexportRouteV1 {
    ReexportRouteV1::try_new(provider, vec![ReexportRouteHopV1::new(provider, binding)]).unwrap()
}
