use std::{collections::BTreeMap, fmt};

use scoop_identity::{
    BindingTarget, CanonicalIdentifier, CborIdentityRecord, ConeCoordinate, ConeIdentity,
    DeclarationScope, DefinitionOrigin, DefinitionOwnerChain, ExportBindingKey,
    NominalDeclarationOwner, NonEmptyVec, NormalizedSourcePath, PackagePath,
    PersistentExportBindingId, PersistentTypeAliasId, PersistentTypeId, SignatureTypeKey,
    SourceContextKey, SourceDeclarationKey, SourceDeclarationSite, SourceIdentity,
    SourceNominalKind, SourceSpan,
};
use scoop_wire::{BudgetMeter, DecodeLimits, WirePath};

use super::*;
use crate::{
    CanonicalCallableInterfacesV1, CanonicalCallableSourceInterfacesV1,
    CanonicalDependencyBindingWitnessesV1, CanonicalExportConstValuesV1,
    CanonicalExportDefaultTemplatesV1, CanonicalExportDefinitionSourcesV1,
    CanonicalExternalHirReferenceRolesV1, CanonicalExternalHirReferencesV1,
    CanonicalNominalInterfacesV1, CanonicalPropertyInterfacesV1, CanonicalPublicExportBindingsV1,
    CanonicalTypeAliasInterfacesV1, DependencyBindingWitnessV1, ExportDefinitionSourceV1,
    ExternalHirReferenceRoleV1, ExternalHirReferenceSemanticAuthority, ExternalHirReferenceV1,
    ExternalHirTargetV1, PublicExportBindingClosureAuthority, PublicLookupAccessV1,
    ReexportRouteHopV1, ReexportRouteV1, TypeAliasInterfaceRecordV1, TypeAliasTargetV1,
};

#[test]
fn accepts_exact_foreign_alias_closure_and_ignores_current_cone_targets() {
    let fixture = Fixture::new();

    assert_eq!(fixture.validate(&fixture.section), Ok(()));
    assert_eq!(fixture.section.external_references().records().len(), 2);
}

#[test]
fn direct_aliases_and_expanded_signatures_each_require_their_reference() {
    let fixture = Fixture::new();

    for case in &fixture.cases {
        let records = fixture
            .section
            .external_references()
            .records()
            .iter()
            .filter(|record| record.target() != case.target)
            .cloned()
            .collect();
        let section = fixture.with_references(records);

        assert_eq!(
            fixture.validate(&section),
            Err(ExternalHirAliasClosureValidationError::MissingReference {
                site: case.site,
                target: case.target,
            })
        );
    }
}

#[test]
fn rejects_missing_role_and_wrong_origin_at_the_exact_alias_use() {
    let fixture = Fixture::new();
    let case = fixture
        .cases
        .iter()
        .find(|case| {
            matches!(
                case.site,
                ExternalHirAliasUseSiteV1::ExpandedSignature { .. }
            )
        })
        .unwrap();

    let missing_role = fixture.replace_reference(
        case.target,
        fixture.provider,
        ExternalHirReferenceRoleV1::SignatureDependency,
        None,
    );
    let record_index = reference_index(&missing_role, case.target);
    assert_eq!(
        fixture.validate(&missing_role),
        Err(ExternalHirAliasClosureValidationError::MissingRole {
            site: case.site,
            record_index,
            target: case.target,
        })
    );

    let wrong_origin = fixture.replace_reference(
        case.target,
        fixture.alternate,
        ExternalHirReferenceRoleV1::AliasTarget,
        Some(case.route.clone()),
    );
    let record_index = reference_index(&wrong_origin, case.target);
    assert_eq!(
        fixture.validate(&wrong_origin),
        Err(ExternalHirAliasClosureValidationError::OriginMismatch {
            site: case.site,
            record_index,
            target: case.target,
            expected: fixture.provider,
            actual: fixture.alternate,
        })
    );
}

#[test]
fn rejects_unobserved_alias_roles_and_missing_origin_authority() {
    let fixture = Fixture::new();
    let (extra_alias, extra_binding) = type_alias_identity(fixture.provider, "Extra");
    let extra_target = ExternalHirTargetV1::TypeAlias(extra_alias);
    let extra = section(
        CanonicalTypeAliasInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalExternalHirReferencesV1::try_new(vec![reference(
            fixture.provider,
            extra_target,
            ExternalHirReferenceRoleV1::AliasTarget,
            Some(route(fixture.provider, extra_binding)),
        )])
        .unwrap(),
    );
    assert_eq!(
        fixture.validate(&extra),
        Err(ExternalHirAliasClosureValidationError::ExtraRole {
            record_index: 0,
            target: extra_target,
        })
    );

    let case = &fixture.cases[0];
    let mut authority = fixture.authority.clone();
    authority.origins.remove(&case.target);
    assert_eq!(
        fixture.validate_with(&fixture.section, &mut authority),
        Err(ExternalHirAliasClosureValidationError::TargetOrigin {
            site: case.site,
            target: case.target,
            error: AuthorityError,
        })
    );
}

struct Case {
    target: ExternalHirTargetV1,
    site: ExternalHirAliasUseSiteV1,
    route: ReexportRouteV1,
}

struct Fixture {
    provider: ConeIdentity,
    alternate: ConeIdentity,
    section: CrossConeHirInterfaceSectionV1,
    cases: Vec<Case>,
    authority: Authority,
}

impl Fixture {
    fn new() -> Self {
        let current = cone("current");
        let provider = cone("provider");
        let alternate = cone("alternate");
        let (foreign_alias, foreign_alias_binding) = type_alias_identity(provider, "ForeignAlias");
        let (foreign_nominal, foreign_nominal_binding) = nominal_identity(provider, "ForeignType");
        let (local_alias, _) = type_alias_identity(current, "LocalAlias");
        let (local_nominal, _) = nominal_identity(current, "LocalType");

        let (direct_declaration, direct_record) = alias_record(
            current,
            "DirectUse",
            TypeAliasTargetV1::Alias(foreign_alias),
            1,
        );
        let nested_signature = SignatureTypeKey::Tuple(
            NonEmptyVec::new(vec![
                SignatureTypeKey::RawPointer(Box::new(signature(foreign_nominal))),
                signature(local_nominal),
                signature(foreign_nominal),
            ])
            .unwrap(),
        );
        let (signature_declaration, signature_record) = alias_record(
            current,
            "SignatureUse",
            TypeAliasTargetV1::Signature(nested_signature),
            2,
        );
        let (_, local_record) = alias_record(
            current,
            "LocalForward",
            TypeAliasTargetV1::Alias(local_alias),
            3,
        );
        let aliases = CanonicalTypeAliasInterfacesV1::try_new(vec![
            direct_record,
            signature_record,
            local_record,
        ])
        .unwrap();
        let direct_index = alias_index(&aliases, direct_declaration);
        let signature_index = alias_index(&aliases, signature_declaration);

        let foreign_alias_target = ExternalHirTargetV1::TypeAlias(foreign_alias);
        let foreign_nominal_target = ExternalHirTargetV1::from(foreign_nominal);
        let alias_route = route(provider, foreign_alias_binding);
        let nominal_route = route(provider, foreign_nominal_binding);
        let references = CanonicalExternalHirReferencesV1::try_new(vec![
            reference(
                provider,
                foreign_alias_target,
                ExternalHirReferenceRoleV1::AliasTarget,
                Some(alias_route.clone()),
            ),
            reference(
                provider,
                foreign_nominal_target,
                ExternalHirReferenceRoleV1::AliasTarget,
                Some(nominal_route.clone()),
            ),
        ])
        .unwrap();
        let section = section(aliases, references);
        let authority = Authority {
            current,
            origins: BTreeMap::from([
                (foreign_alias_target, provider),
                (foreign_nominal_target, provider),
                (ExternalHirTargetV1::TypeAlias(local_alias), current),
                (ExternalHirTargetV1::from(local_nominal), current),
            ]),
        };
        Self {
            provider,
            alternate,
            section,
            cases: vec![
                Case {
                    target: foreign_alias_target,
                    site: ExternalHirAliasUseSiteV1::DirectAlias {
                        record_index: direct_index,
                    },
                    route: alias_route,
                },
                Case {
                    target: foreign_nominal_target,
                    site: ExternalHirAliasUseSiteV1::ExpandedSignature {
                        record_index: signature_index,
                    },
                    route: nominal_route,
                },
            ],
            authority,
        }
    }

    fn with_references(
        &self,
        records: Vec<ExternalHirReferenceV1>,
    ) -> CrossConeHirInterfaceSectionV1 {
        section(
            self.section.type_aliases().clone(),
            CanonicalExternalHirReferencesV1::try_new(records).unwrap(),
        )
    }

    fn replace_reference(
        &self,
        target: ExternalHirTargetV1,
        origin: ConeIdentity,
        role: ExternalHirReferenceRoleV1,
        route: Option<ReexportRouteV1>,
    ) -> CrossConeHirInterfaceSectionV1 {
        let mut records: Vec<_> = self
            .section
            .external_references()
            .records()
            .iter()
            .filter(|record| record.target() != target)
            .cloned()
            .collect();
        records.push(reference(origin, target, role, route));
        self.with_references(records)
    }

    fn validate(
        &self,
        section: &CrossConeHirInterfaceSectionV1,
    ) -> Result<(), ExternalHirAliasClosureValidationError<AuthorityError>> {
        self.validate_with(section, &mut self.authority.clone())
    }

    fn validate_with(
        &self,
        section: &CrossConeHirInterfaceSectionV1,
        authority: &mut Authority,
    ) -> Result<(), ExternalHirAliasClosureValidationError<AuthorityError>> {
        section.validate_alias_reference_closure(
            authority,
            &mut BudgetMeter::new(DecodeLimits::default()),
            &WirePath::root(),
        )
    }
}

fn section(
    aliases: CanonicalTypeAliasInterfacesV1,
    references: CanonicalExternalHirReferencesV1,
) -> CrossConeHirInterfaceSectionV1 {
    CrossConeHirInterfaceSectionV1::new(
        CanonicalPublicExportBindingsV1::try_new(Vec::new()).unwrap(),
        CanonicalNominalInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalCallableInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalPropertyInterfacesV1::try_new(Vec::new()).unwrap(),
        aliases,
        CanonicalCallableSourceInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalExportDefaultTemplatesV1::try_new(Vec::new()).unwrap(),
        CanonicalExportConstValuesV1::try_new(Vec::new()).unwrap(),
        CanonicalExportDefinitionSourcesV1::try_new(Vec::new()).unwrap(),
        references,
    )
}

fn alias_record(
    current: ConeIdentity,
    name: &str,
    target: TypeAliasTargetV1,
    point: u64,
) -> (PersistentTypeAliasId, TypeAliasInterfaceRecordV1) {
    let declaration = SourceDeclarationKey::type_alias(site(current), identifier(name));
    let alias = PersistentTypeAliasId::from_source_declaration(&declaration).unwrap();
    let record = TypeAliasInterfaceRecordV1::try_new(
        alias,
        target,
        PublicLookupAccessV1::DirectOnly,
        definition_origin(current, point),
    )
    .unwrap();
    (alias, record)
}

fn alias_index(aliases: &CanonicalTypeAliasInterfacesV1, alias: PersistentTypeAliasId) -> usize {
    aliases
        .records()
        .iter()
        .position(|record| record.alias() == alias)
        .unwrap()
}

fn reference_index(section: &CrossConeHirInterfaceSectionV1, target: ExternalHirTargetV1) -> usize {
    section
        .external_references()
        .records()
        .iter()
        .position(|record| record.target() == target)
        .unwrap()
}

fn reference(
    origin: ConeIdentity,
    target: ExternalHirTargetV1,
    role: ExternalHirReferenceRoleV1,
    route: Option<ReexportRouteV1>,
) -> ExternalHirReferenceV1 {
    let witnesses = route
        .into_iter()
        .map(DependencyBindingWitnessV1::new)
        .collect();
    ExternalHirReferenceV1::try_new(
        origin,
        target,
        CanonicalExternalHirReferenceRolesV1::try_new(vec![role]).unwrap(),
        CanonicalDependencyBindingWitnessesV1::try_new(witnesses).unwrap(),
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

fn type_alias_identity(
    origin: ConeIdentity,
    name: &str,
) -> (PersistentTypeAliasId, PersistentExportBindingId) {
    let declaration = SourceDeclarationKey::type_alias(site(origin), identifier(name));
    let alias = PersistentTypeAliasId::from_source_declaration(&declaration).unwrap();
    let target = BindingTarget::type_alias(&declaration).unwrap();
    (alias, binding(origin, name, target))
}

fn nominal_identity(
    origin: ConeIdentity,
    name: &str,
) -> (NominalDeclarationOwner, PersistentExportBindingId) {
    let declaration =
        SourceDeclarationKey::nominal(site(origin), identifier(name), SourceNominalKind::Class, 0);
    let nominal = NominalDeclarationOwner::Concrete(
        PersistentTypeId::from_source_declaration(&declaration).unwrap(),
    );
    let target = BindingTarget::type_name(&declaration).unwrap();
    (nominal, binding(origin, name, target))
}

fn binding(origin: ConeIdentity, name: &str, target: BindingTarget) -> PersistentExportBindingId {
    CborIdentityRecord::<PersistentExportBindingId, _>::from_key(ExportBindingKey::new(
        origin,
        PackagePath::root(),
        identifier(name),
        target,
    ))
    .unwrap()
    .id()
}

fn route(provider: ConeIdentity, binding: PersistentExportBindingId) -> ReexportRouteV1 {
    ReexportRouteV1::try_new(provider, vec![ReexportRouteHopV1::new(provider, binding)]).unwrap()
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

    fn binding_key(&self, _binding: PersistentExportBindingId) -> Option<&ExportBindingKey> {
        None
    }

    fn public_bindings(&self, _exporter: ConeIdentity) -> Option<&CanonicalPublicExportBindingsV1> {
        None
    }
}
