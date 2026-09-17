use scoop_identity::{
    BindingTarget, CanonicalIdentifier, CborIdentityRecord, ConeCoordinate, ConeIdentity,
    DeclarationScope, DefinitionOwnerChain, ExportBindingKey, PackagePath,
    PendingIdentityValidation, PersistentExportBindingId, PersistentTypeAliasId,
    SourceDeclarationKey, SourceDeclarationSite, ValidatedIdentityGraph,
};

use super::{
    CanonicalDependencyBindingWitnessesV1, CanonicalExternalHirReferenceRolesV1,
    DependencyBindingWitnessV1, ExternalHirReferenceRoleV1, ExternalHirReferenceV1,
    ExternalHirTargetV1,
};
use crate::{ReexportRouteHopV1, ReexportRouteV1};

pub(super) struct Fixture {
    pub(super) provider: ConeIdentity,
    pub(super) first_alias: PersistentTypeAliasId,
    pub(super) second_alias: PersistentTypeAliasId,
    pub(super) first_route: ReexportRouteV1,
    pub(super) second_route: ReexportRouteV1,
}

impl Fixture {
    pub(super) fn new() -> Self {
        let provider = ConeCoordinate::new("example", "provider", "1.0.0")
            .unwrap()
            .identity()
            .unwrap();
        let (first_alias, first_binding) = alias_binding(provider, "First");
        let (second_alias, second_binding) = alias_binding(provider, "Second");
        Self {
            provider,
            first_alias,
            second_alias,
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
            roles(&[ExternalHirReferenceRoleV1::AliasTarget]),
            witnesses(vec![route]),
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

fn alias_binding(
    provider: ConeIdentity,
    name: &str,
) -> (PersistentTypeAliasId, PersistentExportBindingId) {
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
    let binding =
        CborIdentityRecord::<PersistentExportBindingId, _>::from_key(ExportBindingKey::new(
            provider,
            PackagePath::root(),
            CanonicalIdentifier::new(name).unwrap(),
            BindingTarget::type_alias(&declaration).unwrap(),
        ))
        .unwrap()
        .id();
    (alias, binding)
}

fn route(provider: ConeIdentity, binding: PersistentExportBindingId) -> ReexportRouteV1 {
    ReexportRouteV1::try_new(provider, vec![ReexportRouteHopV1::new(provider, binding)]).unwrap()
}
