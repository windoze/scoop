use std::{collections::BTreeMap, fmt};

use scoop_identity::{
    BindingTarget, CanonicalIdentifier, ConeCoordinate, ConeIdentity, DeclarationScope,
    DefinitionOrigin, DefinitionOwnerChain, NominalDeclarationOwner, NormalizedSourcePath,
    PackagePath, PersistentExportBindingId, PersistentPropertyId, PersistentTypeAliasId,
    PersistentTypeId, SignatureTypeKey, SourceContextKey, SourceDeclarationKey,
    SourceDeclarationSite, SourceIdentity, SourceNominalKind, SourceSpan,
};
use scoop_wire::WirePath;

use super::*;
use crate::{
    CanonicalBooleanV1, CanonicalCallableInterfacesV1, CanonicalCallableSourceInterfacesV1,
    CanonicalConstValueV1, CanonicalDependencyBindingWitnessesV1, CanonicalExportConstValuesV1,
    CanonicalExportDefaultTemplatesV1, CanonicalExportDefinitionSourcesV1,
    CanonicalExternalHirReferenceRolesV1, CanonicalExternalHirReferencesV1,
    CanonicalNominalInterfacesV1, CanonicalPropertyInterfacesV1, CanonicalPublicExportBindingsV1,
    CanonicalTypeAliasInterfacesV1, ExportConstValueV1, ExportDefinitionSourceV1,
    ExternalHirReferenceRoleV1, ExternalHirReferenceSemanticAuthority, ExternalHirReferenceV1,
    ExternalHirTargetV1, PublicExportBindingClosureAuthority,
};

#[test]
fn accepts_the_deduplicated_foreign_const_type_closure_and_ignores_local_types() {
    let fixture = Fixture::new();

    assert_eq!(fixture.validate(&fixture.section), Ok(()));
    assert_eq!(fixture.section.external_references().records().len(), 1);
}

#[test]
fn rejects_missing_reference_role_and_wrong_origin() {
    let fixture = Fixture::new();
    let missing = fixture.with_references(Vec::new());
    assert_eq!(
        fixture.validate(&missing),
        Err(
            ExternalHirConstTypeClosureValidationError::MissingReference {
                constant_index: fixture.first_foreign_index,
                target: fixture.foreign_target,
            }
        )
    );

    let missing_role = fixture.with_references(vec![reference(
        fixture.provider,
        fixture.foreign_target,
        ExternalHirReferenceRoleV1::SignatureDependency,
    )]);
    assert_eq!(
        fixture.validate(&missing_role),
        Err(ExternalHirConstTypeClosureValidationError::MissingRole {
            constant_index: fixture.first_foreign_index,
            record_index: 0,
            target: fixture.foreign_target,
        })
    );

    let wrong_origin = fixture.with_references(vec![reference(
        fixture.alternate,
        fixture.foreign_target,
        ExternalHirReferenceRoleV1::ConstType,
    )]);
    assert_eq!(
        fixture.validate(&wrong_origin),
        Err(ExternalHirConstTypeClosureValidationError::OriginMismatch {
            constant_index: fixture.first_foreign_index,
            record_index: 0,
            target: fixture.foreign_target,
            expected: fixture.provider,
            actual: fixture.alternate,
        })
    );
}

#[test]
fn rejects_unobserved_const_type_roles_and_missing_origin_authority() {
    let fixture = Fixture::new();
    let extra_target = ExternalHirTargetV1::TypeAlias(type_alias(fixture.provider, "Extra"));
    let extra = section(
        CanonicalExportConstValuesV1::try_new(Vec::new()).unwrap(),
        CanonicalExternalHirReferencesV1::try_new(vec![reference(
            fixture.provider,
            extra_target,
            ExternalHirReferenceRoleV1::ConstType,
        )])
        .unwrap(),
    );
    assert_eq!(
        fixture.validate(&extra),
        Err(ExternalHirConstTypeClosureValidationError::ExtraRole {
            record_index: 0,
            target: extra_target,
        })
    );

    let mut authority = fixture.authority.clone();
    authority.origins.remove(&fixture.foreign_target);
    assert_eq!(
        fixture.validate_with(&fixture.section, &mut authority),
        Err(ExternalHirConstTypeClosureValidationError::TargetOrigin {
            constant_index: fixture.first_foreign_index,
            target: fixture.foreign_target,
            error: AuthorityError,
        })
    );
}

struct Fixture {
    provider: ConeIdentity,
    alternate: ConeIdentity,
    foreign_target: ExternalHirTargetV1,
    first_foreign_index: usize,
    section: CrossConeHirInterfaceSectionV1,
    authority: Authority,
}

impl Fixture {
    fn new() -> Self {
        let current = cone("current");
        let provider = cone("provider");
        let alternate = cone("alternate");
        let foreign_type = nominal(provider, "ForeignConstType");
        let local_type = nominal(current, "LocalConstType");
        let foreign_target = ExternalHirTargetV1::from(foreign_type);
        let local_target = ExternalHirTargetV1::from(local_type);
        let constants = CanonicalExportConstValuesV1::try_new(vec![
            constant(current, "first", foreign_type, 1),
            constant(current, "local", local_type, 2),
            constant(current, "second", foreign_type, 3),
        ])
        .unwrap();
        let first_foreign_index = constants
            .records()
            .iter()
            .position(|constant| constant.value_type() == &signature(foreign_type))
            .unwrap();
        let references = CanonicalExternalHirReferencesV1::try_new(vec![reference(
            provider,
            foreign_target,
            ExternalHirReferenceRoleV1::ConstType,
        )])
        .unwrap();
        let section = section(constants, references);
        let authority = Authority {
            current,
            origins: BTreeMap::from([(foreign_target, provider), (local_target, current)]),
        };
        Self {
            provider,
            alternate,
            foreign_target,
            first_foreign_index,
            section,
            authority,
        }
    }

    fn with_references(
        &self,
        records: Vec<ExternalHirReferenceV1>,
    ) -> CrossConeHirInterfaceSectionV1 {
        section(
            self.section.constants().clone(),
            CanonicalExternalHirReferencesV1::try_new(records).unwrap(),
        )
    }

    fn validate(
        &self,
        section: &CrossConeHirInterfaceSectionV1,
    ) -> Result<(), ExternalHirConstTypeClosureValidationError<AuthorityError>> {
        self.validate_with(section, &mut self.authority.clone())
    }

    fn validate_with(
        &self,
        section: &CrossConeHirInterfaceSectionV1,
        authority: &mut Authority,
    ) -> Result<(), ExternalHirConstTypeClosureValidationError<AuthorityError>> {
        section.validate_const_type_reference_closure(authority, &WirePath::root())
    }
}

fn section(
    constants: CanonicalExportConstValuesV1,
    references: CanonicalExternalHirReferencesV1,
) -> CrossConeHirInterfaceSectionV1 {
    CrossConeHirInterfaceSectionV1::new(
        CanonicalPublicExportBindingsV1::try_new(Vec::new()).unwrap(),
        CanonicalNominalInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalCallableInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalPropertyInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalTypeAliasInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalCallableSourceInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalExportDefaultTemplatesV1::try_new(Vec::new()).unwrap(),
        constants,
        CanonicalExportDefinitionSourcesV1::try_new(Vec::new()).unwrap(),
        references,
        Default::default(),
        Default::default(),
        Default::default(),
    )
}

fn constant(
    current: ConeIdentity,
    name: &str,
    value_type: NominalDeclarationOwner,
    point: u64,
) -> ExportConstValueV1 {
    let declaration = SourceDeclarationKey::property(site(current), identifier(name));
    let property = PersistentPropertyId::from_source_declaration(&declaration).unwrap();
    ExportConstValueV1::new(
        property,
        signature(value_type),
        CanonicalConstValueV1::Boolean(CanonicalBooleanV1::True),
        definition_origin(current, point),
    )
}

fn reference(
    origin: ConeIdentity,
    target: ExternalHirTargetV1,
    role: ExternalHirReferenceRoleV1,
) -> ExternalHirReferenceV1 {
    ExternalHirReferenceV1::try_new(
        origin,
        target,
        CanonicalExternalHirReferenceRolesV1::try_new(vec![role]).unwrap(),
        CanonicalDependencyBindingWitnessesV1::try_new(Vec::new()).unwrap(),
        Default::default(),
        Default::default(),
    )
    .unwrap()
}

fn signature(target: NominalDeclarationOwner) -> SignatureTypeKey {
    match target {
        NominalDeclarationOwner::Concrete(declaration) => SignatureTypeKey::Nominal(declaration),
        NominalDeclarationOwner::GenericTemplate(_) => {
            panic!("test fixture uses only concrete nominal types")
        }
    }
}

fn nominal(origin: ConeIdentity, name: &str) -> NominalDeclarationOwner {
    let key =
        SourceDeclarationKey::nominal(site(origin), identifier(name), SourceNominalKind::Class, 0);
    NominalDeclarationOwner::Concrete(PersistentTypeId::from_source_declaration(&key).unwrap())
}

fn type_alias(origin: ConeIdentity, name: &str) -> PersistentTypeAliasId {
    PersistentTypeAliasId::from_source_declaration(&SourceDeclarationKey::type_alias(
        site(origin),
        identifier(name),
    ))
    .unwrap()
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

fn definition_origin(cone: ConeIdentity, point: u64) -> ExportDefinitionSourceV1 {
    let source =
        SourceIdentity::new(cone, NormalizedSourcePath::new("main.scoop").unwrap()).unwrap();
    let context = SourceContextKey::File {
        source: source.clone(),
    };
    ExportDefinitionSourceV1::new(
        DefinitionOrigin::new(source, SourceSpan::new(point, point + 1).unwrap(), &context)
            .unwrap(),
    )
}

#[derive(Clone)]
struct Authority {
    current: ConeIdentity,
    origins: BTreeMap<ExternalHirTargetV1, ConeIdentity>,
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
        self.origins.get(&target).copied().ok_or(AuthorityError)
    }

    fn external_hir_target_binding_root(
        &mut self,
        _target: ExternalHirTargetV1,
    ) -> Result<BindingTarget, AuthorityError> {
        Err(AuthorityError)
    }
}

impl PublicExportBindingClosureAuthority for Authority {
    fn closure_node_count(&self) -> usize {
        0
    }

    fn is_direct_dependency(&self, _provider: ConeIdentity) -> bool {
        false
    }

    fn binding_key(
        &self,
        _binding: PersistentExportBindingId,
    ) -> Option<&scoop_identity::ExportBindingKey> {
        None
    }

    fn public_bindings(&self, _exporter: ConeIdentity) -> Option<&CanonicalPublicExportBindingsV1> {
        None
    }
}
