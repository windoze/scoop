use std::collections::BTreeMap;

use scoop_identity::{
    BindingTarget, CanonicalIdentifier, CborIdentityRecord, ConeCoordinate, ConeIdentity,
    DeclarationScope, DefinitionOwnerChain, ExportBindingKey, NominalDeclarationOwner, PackagePath,
    PersistentExportBindingId, PersistentTypeAliasId, PersistentTypeId, SourceDeclarationKey,
    SourceDeclarationSite, SourceNominalKind,
};
use scoop_wire::{BudgetMeter, DecodeLimits, WirePath};

use super::*;
use crate::{
    CanonicalCallableInterfacesV1, CanonicalCallableSourceInterfacesV1,
    CanonicalExportConstValuesV1, CanonicalExportDefaultTemplatesV1, CanonicalNominalInterfacesV1,
    CanonicalPropertyInterfacesV1, CanonicalPublicExportBindingsV1, CanonicalReexportRoutesV1,
    CanonicalTypeAliasInterfacesV1, DependencyBindingWitnessV1, ExportBindingSourceV1,
    ExternalHirReferenceRoleV1, ExternalHirReferenceSemanticAuthority, ExternalHirTargetV1,
    PublicExportBindingClosureAuthority, PublicExportBindingRecordV1, ReexportRouteHopV1,
    ReexportRouteV1,
};

mod builtin_defaults;
mod dispatch;

#[test]
fn producer_unions_reexport_and_selected_roles_with_one_canonical_witness() {
    let current = cone("consumer");
    let provider = cone("provider");
    let declaration = alias_declaration(provider, "Shared");
    let alias = PersistentTypeAliasId::from_source_declaration(&declaration).unwrap();
    let target = ExternalHirTargetV1::TypeAlias(alias);
    let root = BindingTarget::type_alias(&declaration).unwrap();
    let provider_binding = binding(provider, "Shared", root);
    let route = direct_route(provider, provider_binding.id());
    let reexport_binding = binding(current, "Shared", root);
    let public_bindings =
        CanonicalPublicExportBindingsV1::try_new(vec![PublicExportBindingRecordV1::new(
            reexport_binding.id(),
            ExportBindingSourceV1::Reexport {
                routes: CanonicalReexportRoutesV1::try_new(vec![route.clone()]).unwrap(),
            },
        )])
        .unwrap();
    let parts = EmptyParts::new(public_bindings);
    let selected = ExternalHirBindingWitnessUse::new(
        target,
        ExternalHirBindingWitnessRole::ConcreteSelectedUse,
        DependencyBindingWitnessV1::new(route.clone()),
    );
    let mut authority = Authority::new(current)
        .with_binding(reexport_binding)
        .with_origin(target, provider)
        .with_root(target, root);

    let references = CanonicalExternalHirReferencesV1::from_interface_parts(
        parts.input(),
        &[selected],
        &mut authority,
    )
    .unwrap();

    let record = references.get(target).unwrap();
    assert_eq!(record.origin(), provider);
    assert_eq!(
        record.roles().roles(),
        &[
            ExternalHirReferenceRoleV1::ReexportTarget,
            ExternalHirReferenceRoleV1::ConcreteSelectedUse,
        ]
    );
    assert_eq!(record.witnesses().witnesses().len(), 1);
    assert_eq!(record.witnesses().witnesses()[0].route(), &route);
}

#[test]
fn signature_collection_deduplicates_foreign_nominal_leaves() {
    let current = cone("consumer");
    let provider = cone("provider");
    let nominal = nominal(provider, "Payload");
    let target = ExternalHirTargetV1::from(nominal);
    let NominalDeclarationOwner::Concrete(nominal_id) = nominal else {
        unreachable!()
    };
    let signature =
        scoop_identity::SignatureTypeKey::Tuple(scoop_identity::NonEmptyVec::from_first(
            scoop_identity::SignatureTypeKey::Nominal(nominal_id),
            vec![scoop_identity::SignatureTypeKey::Nominal(nominal_id)],
        ));
    let mut authority = Authority::new(current).with_origin(target, provider);
    let mut accumulator = accumulator::ExternalReferenceAccumulator::new(&mut authority);
    accumulator
        .observe_signature(
            &signature,
            ExternalHirReferenceRoleV1::SignatureDependency,
            &mut BudgetMeter::new(DecodeLimits::default()),
            &WirePath::root(),
        )
        .unwrap();

    let references = accumulator.finish::<AuthorityError>().unwrap();
    assert_eq!(references.records().len(), 1);
    assert_eq!(references.records()[0].target(), target);
    assert_eq!(
        references.records()[0].roles().roles(),
        &[ExternalHirReferenceRoleV1::SignatureDependency]
    );
    assert!(references.records()[0].witnesses().is_empty());
}

#[test]
fn source_name_roles_require_an_actual_selected_witness() {
    let current = cone("consumer");
    let provider = cone("provider");
    let alias =
        PersistentTypeAliasId::from_source_declaration(&alias_declaration(provider, "Shared"))
            .unwrap();
    let target = ExternalHirTargetV1::TypeAlias(alias);
    let mut authority = Authority::new(current).with_origin(target, provider);
    let mut accumulator = accumulator::ExternalReferenceAccumulator::new(&mut authority);
    accumulator
        .observe(target, ExternalHirReferenceRoleV1::AliasTarget)
        .unwrap();

    assert!(matches!(
        accumulator.finish::<AuthorityError>(),
        Err(ExternalHirReferenceProductionError::MissingWitnessUse {
            target: actual,
            role: ExternalHirReferenceRoleV1::AliasTarget,
        }) if actual == target
    ));
}

struct EmptyParts {
    public_bindings: CanonicalPublicExportBindingsV1,
    nominal_interfaces: CanonicalNominalInterfacesV1,
    callable_interfaces: CanonicalCallableInterfacesV1,
    property_interfaces: CanonicalPropertyInterfacesV1,
    type_aliases: CanonicalTypeAliasInterfacesV1,
    source_interfaces: CanonicalCallableSourceInterfacesV1,
    default_templates: CanonicalExportDefaultTemplatesV1,
    constants: CanonicalExportConstValuesV1,
}

impl EmptyParts {
    fn new(public_bindings: CanonicalPublicExportBindingsV1) -> Self {
        Self {
            public_bindings,
            nominal_interfaces: CanonicalNominalInterfacesV1::try_new(Vec::new()).unwrap(),
            callable_interfaces: CanonicalCallableInterfacesV1::try_new(Vec::new()).unwrap(),
            property_interfaces: CanonicalPropertyInterfacesV1::try_new(Vec::new()).unwrap(),
            type_aliases: CanonicalTypeAliasInterfacesV1::try_new(Vec::new()).unwrap(),
            source_interfaces: CanonicalCallableSourceInterfacesV1::try_new(Vec::new()).unwrap(),
            default_templates: CanonicalExportDefaultTemplatesV1::try_new(Vec::new()).unwrap(),
            constants: CanonicalExportConstValuesV1::try_new(Vec::new()).unwrap(),
        }
    }

    fn input(&self) -> ExternalHirReferenceProductionInput<'_> {
        ExternalHirReferenceProductionInput::new(
            &self.public_bindings,
            &self.nominal_interfaces,
            &self.callable_interfaces,
            &self.property_interfaces,
            &self.type_aliases,
            &self.source_interfaces,
            &self.default_templates,
            &self.constants,
        )
    }
}

#[derive(Debug)]
struct Authority {
    current: ConeIdentity,
    bindings: BTreeMap<PersistentExportBindingId, ExportBindingKey>,
    origins: BTreeMap<ExternalHirTargetV1, ConeIdentity>,
    roots: BTreeMap<ExternalHirTargetV1, BindingTarget>,
}

impl Authority {
    fn new(current: ConeIdentity) -> Self {
        Self {
            current,
            bindings: BTreeMap::new(),
            origins: BTreeMap::new(),
            roots: BTreeMap::new(),
        }
    }

    fn with_binding(
        mut self,
        binding: CborIdentityRecord<PersistentExportBindingId, ExportBindingKey>,
    ) -> Self {
        self.bindings.insert(binding.id(), binding.into_key());
        self
    }

    fn with_origin(mut self, target: ExternalHirTargetV1, origin: ConeIdentity) -> Self {
        self.origins.insert(target, origin);
        self
    }

    fn with_root(mut self, target: ExternalHirTargetV1, root: BindingTarget) -> Self {
        self.roots.insert(target, root);
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AuthorityError;

impl std::fmt::Display for AuthorityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("missing test authority")
    }
}

impl std::error::Error for AuthorityError {}

impl PublicExportBindingClosureAuthority for Authority {
    fn closure_node_count(&self) -> usize {
        2
    }

    fn is_direct_dependency(&self, provider: ConeIdentity) -> bool {
        provider != self.current
    }

    fn binding_key(&self, binding: PersistentExportBindingId) -> Option<&ExportBindingKey> {
        self.bindings.get(&binding)
    }

    fn public_bindings(&self, _exporter: ConeIdentity) -> Option<&CanonicalPublicExportBindingsV1> {
        None
    }
}

impl ExternalHirReferenceSemanticAuthority<AuthorityError> for Authority {
    fn current_cone(&self) -> ConeIdentity {
        self.current
    }

    fn external_hir_target_origin(
        &mut self,
        target: ExternalHirTargetV1,
    ) -> Result<ConeIdentity, AuthorityError> {
        self.origins.get(&target).copied().ok_or(AuthorityError)
    }

    fn external_hir_target_binding_root(
        &mut self,
        target: ExternalHirTargetV1,
    ) -> Result<BindingTarget, AuthorityError> {
        self.roots.get(&target).copied().ok_or(AuthorityError)
    }
}

fn alias_declaration(origin: ConeIdentity, name: &str) -> SourceDeclarationKey {
    SourceDeclarationKey::type_alias(site(origin), CanonicalIdentifier::new(name).unwrap())
}

fn nominal(origin: ConeIdentity, name: &str) -> NominalDeclarationOwner {
    let declaration = SourceDeclarationKey::nominal(
        site(origin),
        CanonicalIdentifier::new(name).unwrap(),
        SourceNominalKind::Class,
        0,
    );
    NominalDeclarationOwner::Concrete(
        PersistentTypeId::from_source_declaration(&declaration).unwrap(),
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

fn direct_route(provider: ConeIdentity, binding: PersistentExportBindingId) -> ReexportRouteV1 {
    ReexportRouteV1::try_new(provider, vec![ReexportRouteHopV1::new(provider, binding)]).unwrap()
}

fn site(origin: ConeIdentity) -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        origin,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}

fn cone(name: &str) -> ConeIdentity {
    ConeCoordinate::new("example", name, "1.0.0")
        .unwrap()
        .identity()
        .unwrap()
}
