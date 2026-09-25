use std::{collections::BTreeMap, fmt};

use scoop_identity::{
    BindingTarget, CanonicalIdentifier, CborIdentityRecord, ConeCoordinate, ConeIdentity,
    DeclarationScope, DefinitionOwnerChain, ExportBindingKey, PackagePath,
    PersistentExportBindingId, PersistentTypeAliasId, SourceDeclarationKey, SourceDeclarationSite,
};
use scoop_wire::WirePath;

use super::*;
use crate::{
    CanonicalCallableInterfacesV1, CanonicalCallableSourceInterfacesV1,
    CanonicalDependencyBindingWitnessesV1, CanonicalExportConstValuesV1,
    CanonicalExportDefaultTemplatesV1, CanonicalExportDefinitionSourcesV1,
    CanonicalExternalHirReferenceRolesV1, CanonicalExternalHirReferencesV1,
    CanonicalNominalInterfacesV1, CanonicalPropertyInterfacesV1, CanonicalPublicExportBindingsV1,
    CanonicalTypeAliasInterfacesV1, DependencyBindingWitnessV1, ExportBindingSourceV1,
    ExternalHirReferenceRoleV1, ExternalHirReferenceSemanticAuthority,
    ExternalHirReferenceSemanticValidationError, ExternalHirReferenceV1, ExternalHirTargetV1,
    PublicExportBindingClosureAuthority, PublicExportBindingRecordV1, ReexportRouteHopV1,
    ReexportRouteV1,
};

#[test]
fn accepts_empty_export_closure_and_explicit_concrete_selected_edges() {
    let fixture = Fixture::new();

    assert_eq!(
        fixture.validate(&empty_section(
            CanonicalExternalHirReferencesV1::try_new(Vec::new()).unwrap()
        )),
        Ok(())
    );
    assert_eq!(fixture.validate(&fixture.selected_section()), Ok(()));
}

#[test]
fn validates_record_semantics_before_reconstructing_roles() {
    let fixture = Fixture::new();
    let section = fixture.section_with_role(ExternalHirReferenceRoleV1::SignatureDependency);
    let mut authority = fixture.authority.clone();
    authority.current = fixture.provider;

    assert_eq!(
        validate(&section, &mut authority),
        Err(CrossConeHirExternalReferenceValidationError::Records(
            crate::ExternalHirReferenceSetSemanticValidationError::Record {
                index: 0,
                error: Box::new(
                    ExternalHirReferenceSemanticValidationError::CurrentConeTarget {
                        target: fixture.target,
                        current: fixture.provider,
                    }
                ),
            }
        ))
    );
}

#[test]
fn reports_the_exact_reconstructible_role_stage() {
    let fixture = Fixture::new();
    let signature = fixture.section_with_role(ExternalHirReferenceRoleV1::SignatureDependency);
    assert!(matches!(
        fixture.validate(&signature),
        Err(CrossConeHirExternalReferenceValidationError::Signatures(error))
            if matches!(
                error.as_ref(),
                crate::ExternalHirSignatureClosureValidationError::ExtraRole {
                    record_index: 0,
                    target,
                } if *target == fixture.target
            )
    ));

    let alias = fixture.section_with_role(ExternalHirReferenceRoleV1::AliasTarget);
    assert!(matches!(
        fixture.validate(&alias),
        Err(CrossConeHirExternalReferenceValidationError::Aliases(error))
            if matches!(
                error.as_ref(),
                crate::ExternalHirAliasClosureValidationError::ExtraRole {
                    record_index: 0,
                    target,
                } if *target == fixture.target
            )
    ));

    let default = fixture.section_with_role(ExternalHirReferenceRoleV1::DefaultDependency);
    assert!(matches!(
        fixture.validate(&default),
        Err(CrossConeHirExternalReferenceValidationError::Defaults(error))
            if matches!(
                error.as_ref(),
                crate::ExternalHirDefaultClosureValidationError::ExtraRole {
                    record_index: 0,
                    target,
                } if *target == fixture.target
            )
    ));

    let const_type = fixture.section_with_role(ExternalHirReferenceRoleV1::ConstType);
    assert!(matches!(
        fixture.validate(&const_type),
        Err(CrossConeHirExternalReferenceValidationError::ConstTypes(error))
            if matches!(
                error.as_ref(),
                crate::ExternalHirConstTypeClosureValidationError::ExtraRole {
                    record_index: 0,
                    target,
                } if *target == fixture.target
            )
    ));
}

struct Fixture {
    provider: ConeIdentity,
    target: ExternalHirTargetV1,
    route: ReexportRouteV1,
    authority: Authority,
}

impl Fixture {
    fn new() -> Self {
        let current = cone("current");
        let provider = cone("provider");
        let declaration = SourceDeclarationKey::type_alias(site(provider), identifier("Target"));
        let alias = PersistentTypeAliasId::from_source_declaration(&declaration).unwrap();
        let target = ExternalHirTargetV1::TypeAlias(alias);
        let root = BindingTarget::type_alias(&declaration).unwrap();
        let binding = CborIdentityRecord::<PersistentExportBindingId, _>::from_key(
            ExportBindingKey::new(provider, PackagePath::root(), identifier("Target"), root),
        )
        .unwrap();
        let route = ReexportRouteV1::try_new(
            provider,
            vec![ReexportRouteHopV1::new(provider, binding.id())],
        )
        .unwrap();
        let surface =
            CanonicalPublicExportBindingsV1::try_new(vec![PublicExportBindingRecordV1::new(
                binding.id(),
                ExportBindingSourceV1::DeclaredCurrent {
                    declaration: root.target(),
                },
            )])
            .unwrap();
        let authority = Authority {
            current,
            target,
            provider,
            root,
            keys: BTreeMap::from([(binding.id(), binding.key().clone())]),
            surfaces: BTreeMap::from([(provider, surface)]),
        };
        Self {
            provider,
            target,
            route,
            authority,
        }
    }

    fn selected_section(&self) -> CrossConeHirInterfaceSectionV1 {
        self.section_with_role(ExternalHirReferenceRoleV1::ConcreteSelectedUse)
    }

    fn section_with_role(
        &self,
        role: ExternalHirReferenceRoleV1,
    ) -> CrossConeHirInterfaceSectionV1 {
        let witnesses = if matches!(
            role,
            ExternalHirReferenceRoleV1::ReexportTarget
                | ExternalHirReferenceRoleV1::AliasTarget
                | ExternalHirReferenceRoleV1::DefaultDependency
                | ExternalHirReferenceRoleV1::ConcreteSelectedUse
        ) {
            vec![DependencyBindingWitnessV1::new(self.route.clone())]
        } else {
            Vec::new()
        };
        let reference = ExternalHirReferenceV1::try_new(
            self.provider,
            self.target,
            CanonicalExternalHirReferenceRolesV1::try_new(vec![role]).unwrap(),
            CanonicalDependencyBindingWitnessesV1::try_new(witnesses).unwrap(),
            Default::default(),
            Default::default(),
        )
        .unwrap();
        empty_section(CanonicalExternalHirReferencesV1::try_new(vec![reference]).unwrap())
    }

    fn validate(
        &self,
        section: &CrossConeHirInterfaceSectionV1,
    ) -> Result<(), CrossConeHirExternalReferenceValidationError<AuthorityError>> {
        validate(section, &mut self.authority.clone())
    }
}

fn validate(
    section: &CrossConeHirInterfaceSectionV1,
    authority: &mut Authority,
) -> Result<(), CrossConeHirExternalReferenceValidationError<AuthorityError>> {
    section.validate_external_reference_closure(authority, &WirePath::root())
}

fn empty_section(references: CanonicalExternalHirReferencesV1) -> CrossConeHirInterfaceSectionV1 {
    CrossConeHirInterfaceSectionV1::new(
        CanonicalPublicExportBindingsV1::try_new(Vec::new()).unwrap(),
        CanonicalNominalInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalCallableInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalPropertyInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalTypeAliasInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalCallableSourceInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalExportDefaultTemplatesV1::try_new(Vec::new()).unwrap(),
        CanonicalExportConstValuesV1::try_new(Vec::new()).unwrap(),
        CanonicalExportDefinitionSourcesV1::try_new(Vec::new()).unwrap(),
        references,
    )
}

#[derive(Clone)]
struct Authority {
    current: ConeIdentity,
    target: ExternalHirTargetV1,
    provider: ConeIdentity,
    root: BindingTarget,
    keys: BTreeMap<PersistentExportBindingId, ExportBindingKey>,
    surfaces: BTreeMap<ConeIdentity, CanonicalPublicExportBindingsV1>,
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
            .then_some(self.provider)
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
        self.surfaces.len()
    }

    fn is_direct_dependency(&self, provider: ConeIdentity) -> bool {
        provider == self.provider
    }

    fn binding_key(&self, binding: PersistentExportBindingId) -> Option<&ExportBindingKey> {
        self.keys.get(&binding)
    }

    fn public_bindings(&self, exporter: ConeIdentity) -> Option<&CanonicalPublicExportBindingsV1> {
        self.surfaces.get(&exporter)
    }
}

fn cone(name: &str) -> ConeIdentity {
    ConeCoordinate::new("example", name, "1.0.0")
        .unwrap()
        .identity()
        .unwrap()
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

fn identifier(value: &str) -> CanonicalIdentifier {
    CanonicalIdentifier::new(value).unwrap()
}
