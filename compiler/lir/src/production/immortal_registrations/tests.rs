use la_arena::Arena;
use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope, DefinitionAtomRole,
    DefinitionAtomSubkey, DefinitionOwnerChain, ExactTypeKey, ImmortalObjectKey,
    ImmortalObjectOwner, LinkageClass, ObjectDefinitionAtomKey, ObjectDefinitionPlanKey,
    PackagePath, PersistentImmortalObjectId, PersistentPropertyId, PersistentSymbolKey,
    PersistentSymbolRequest, PersistentSymbolRequestTable, PersistentTypeId, PropertyOwner,
    SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind, StrongDefinitionEntity,
    StrongDefinitionRole, StructuralDefinitionPath, StructuralDefinitionSiteRole,
    StructuralPathSegment,
};

use super::*;
use crate::{
    CanonicalLirFoundation, Global, GlobalInit, ImmortalObjectIdentity, LirTargetProfile,
    MaterializationRoot, PointerKind, RefScan,
};

#[test]
fn semantic_plans_are_sorted_and_bind_utf8_object_extent() {
    let string_type = exact_type("String");
    let mut globals = Arena::new();
    let first = string_global(2, "plain");
    let first_id = match &first.init {
        GlobalInit::StringConst { identity, .. } => identity.identity_record().id(),
        _ => unreachable!(),
    };
    globals.alloc(first);
    let second = string_global(1, "é");
    let second_id = match &second.init {
        GlobalInit::StringConst { identity, .. } => identity.identity_record().id(),
        _ => unreachable!(),
    };
    globals.alloc(second);

    let plans = StrongImmortalObjectSemanticPlanSetV1::from_globals(
        ConeIdentity::SINGLE_FILE,
        LirTargetProfile::DARWIN_AARCH64,
        &globals,
        ImmortalObjectTypeRegistrationRefV1::DependencyExternal {
            provider: ConeIdentity::CORE,
            exact: string_type,
        },
    )
    .unwrap();

    assert_eq!(plans.producer(), ConeIdentity::SINGLE_FILE);
    assert_eq!(plans.objects().len(), 2);
    assert!(
        plans
            .objects()
            .windows(2)
            .all(|pair| pair[0].object() < pair[1].object())
    );
    for plan in plans.objects() {
        assert_eq!(plan.type_registration(), string_type);
        assert_eq!(plan.required_alignment(), 8);
        assert_eq!(plan.object_size(), 32);
        assert!(plan.object() == first_id || plan.object() == second_id);
        assert_eq!(
            plan.symbol().key(),
            PersistentSymbolKey::ImmortalObject(plan.object())
        );
    }
}

#[test]
fn semantic_plans_reject_duplicate_identity_and_invalid_global_shape() {
    let string_type = exact_type("String");
    let duplicate = string_global(1, "same");
    let mut globals = Arena::new();
    globals.alloc(duplicate_global(&duplicate));
    globals.alloc(duplicate);
    assert!(matches!(
        StrongImmortalObjectSemanticPlanSetV1::from_globals(
            ConeIdentity::SINGLE_FILE,
            LirTargetProfile::DARWIN_AARCH64,
            &globals,
            ImmortalObjectTypeRegistrationRefV1::DependencyExternal {
                provider: ConeIdentity::CORE,
                exact: string_type
            },
        ),
        Err(StrongImmortalObjectSemanticPlanBuildError::DuplicateObject(
            _
        ))
    ));

    let mut wrong_address = Arena::new();
    let mut global = string_global(1, "value");
    global.address_kind = PointerKind::Raw;
    wrong_address.alloc(global);
    assert!(matches!(
        StrongImmortalObjectSemanticPlanSetV1::from_globals(
            ConeIdentity::SINGLE_FILE,
            LirTargetProfile::DARWIN_AARCH64,
            &wrong_address,
            ImmortalObjectTypeRegistrationRefV1::DependencyExternal {
                provider: ConeIdentity::CORE,
                exact: string_type
            },
        ),
        Err(StrongImmortalObjectSemanticPlanBuildError::AddressKind { .. })
    ));

    let mut wrong_scan = Arena::new();
    let mut global = string_global(1, "value");
    global.scan = RefScan::References(vec![0]);
    wrong_scan.alloc(global);
    assert!(matches!(
        StrongImmortalObjectSemanticPlanSetV1::from_globals(
            ConeIdentity::SINGLE_FILE,
            LirTargetProfile::DARWIN_AARCH64,
            &wrong_scan,
            ImmortalObjectTypeRegistrationRefV1::DependencyExternal {
                provider: ConeIdentity::CORE,
                exact: string_type
            },
        ),
        Err(StrongImmortalObjectSemanticPlanBuildError::Scan { .. })
    ));
}

#[test]
fn joins_every_immortal_object_to_its_definition_and_registration() {
    let fixture = Fixture::new(Options::default());

    let plans = fixture.build().unwrap();

    assert_eq!(plans.producer(), ConeIdentity::SINGLE_FILE);
    assert_eq!(plans.registrations().len(), 2);
    assert!(
        plans
            .registrations()
            .windows(2)
            .all(|pair| pair[0].object() < pair[1].object())
    );
    for plan in plans.registrations() {
        assert_eq!(
            plan.registration_symbol().key(),
            PersistentSymbolKey::ImmortalRegistration(plan.object())
        );
        assert_eq!(
            plan.object_symbol().key(),
            PersistentSymbolKey::ImmortalObject(plan.object())
        );
        assert_eq!(
            plan.type_registration_symbol().key(),
            PersistentSymbolKey::TypeRegistration(plan.type_registration())
        );
    }
}

#[test]
fn registration_plans_require_complete_semantics_and_symbols() {
    assert!(matches!(
        Fixture::new(Options {
            omit_last_registration: true,
            ..Options::default()
        })
        .build(),
        Err(StrongImmortalObjectRegistrationPlanBuildError::ObjectSet { expected, actual })
            if expected.len() == 2 && actual.len() == 1
    ));
    for options in [
        Options {
            omit_object_symbol: true,
            ..Options::default()
        },
        Options {
            omit_registration_symbol: true,
            ..Options::default()
        },
    ] {
        assert!(matches!(
            Fixture::new(options).build(),
            Err(StrongImmortalObjectRegistrationPlanBuildError::MissingSymbol(_))
        ));
    }

    assert!(matches!(
        Fixture::new(Options {
            claim_local_string_type: true,
            ..Options::default()
        })
        .build(),
        Err(
            StrongImmortalObjectRegistrationPlanBuildError::LocalTypeRegistrationSet {
                actual: 0,
                ..
            }
        )
    ));
}

#[derive(Clone, Copy, Default)]
struct Options {
    omit_last_registration: bool,
    omit_object_symbol: bool,
    omit_registration_symbol: bool,

    claim_local_string_type: bool,
}

struct ObjectArtifacts {
    object: CborIdentityRecord<PersistentImmortalObjectId, ImmortalObjectKey>,
    object_definition:
        CborIdentityRecord<scoop_identity::ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    object_primary:
        CborIdentityRecord<scoop_identity::ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
    registration_definition:
        CborIdentityRecord<scoop_identity::ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    registration_primary:
        CborIdentityRecord<scoop_identity::ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
}

struct Fixture {
    foundation: ConeLirFoundation,
    identities: RegistrationIdentitySurfaceV1,
    semantics: StrongImmortalObjectSemanticPlanSetV1,
}

impl Fixture {
    fn new(options: Options) -> Self {
        let string_type = exact_type("String");
        let artifacts = [object_artifacts(1), object_artifacts(2)];
        let mut canonical = CanonicalLirFoundation::empty();
        canonical
            .set_immortal_objects(
                artifacts
                    .iter()
                    .map(|artifacts| artifacts.object.clone())
                    .collect(),
            )
            .unwrap();
        canonical
            .set_definition_plans(
                artifacts
                    .iter()
                    .flat_map(|artifacts| {
                        [
                            artifacts.object_definition.clone(),
                            artifacts.registration_definition.clone(),
                        ]
                    })
                    .filter(|plan| {
                        !(options.omit_last_registration
                            && plan.id() == artifacts[1].registration_definition.id())
                    })
                    .collect(),
            )
            .unwrap();
        canonical
            .set_definition_atoms(
                artifacts
                    .iter()
                    .flat_map(|artifacts| {
                        [
                            artifacts.object_primary.clone(),
                            artifacts.registration_primary.clone(),
                        ]
                    })
                    .filter(|atom| {
                        !(options.omit_last_registration
                            && atom.key().plan() == artifacts[1].registration_definition.id())
                    })
                    .collect(),
            )
            .unwrap();
        canonical.set_symbol_requests(
            PersistentSymbolRequestTable::new(
                artifacts
                    .iter()
                    .flat_map(|artifacts| {
                        [
                            symbol(PersistentSymbolKey::ImmortalObject(artifacts.object.id())),
                            symbol(PersistentSymbolKey::ImmortalRegistration(
                                artifacts.object.id(),
                            )),
                        ]
                    })
                    .filter(|request| {
                        !(options.omit_object_symbol
                            && request.key()
                                == PersistentSymbolKey::ImmortalObject(artifacts[0].object.id()))
                            && !(options.omit_registration_symbol
                                && request.key()
                                    == PersistentSymbolKey::ImmortalRegistration(
                                        artifacts[0].object.id(),
                                    ))
                            && !(options.omit_last_registration
                                && request.key()
                                    == PersistentSymbolKey::ImmortalRegistration(
                                        artifacts[1].object.id(),
                                    ))
                    })
                    .collect(),
            )
            .unwrap(),
        );
        let foundation = ConeLirFoundation::try_new(ConeIdentity::SINGLE_FILE, canonical).unwrap();

        let identities = RegistrationIdentitySurfaceV1::from_foundation(&foundation).unwrap();
        let mut objects = artifacts
            .iter()
            .map(|artifacts| {
                StrongImmortalObjectSemanticPlanV1::from_artifact(
                    artifacts.object.id(),
                    symbol(PersistentSymbolKey::ImmortalObject(artifacts.object.id())),
                    32,
                    8,
                    if options.claim_local_string_type {
                        ImmortalObjectTypeRegistrationRefV1::Local(string_type)
                    } else {
                        ImmortalObjectTypeRegistrationRefV1::DependencyExternal {
                            provider: ConeIdentity::CORE,
                            exact: string_type,
                        }
                    },
                )
            })
            .collect::<Vec<_>>();
        objects.sort_unstable_by_key(|object| object.object());
        let semantics = StrongImmortalObjectSemanticPlanSetV1::from_artifact(
            ConeIdentity::SINGLE_FILE,
            objects,
        );
        Self {
            foundation,
            identities,
            semantics,
        }
    }

    fn build(
        &self,
    ) -> Result<
        StrongImmortalObjectRegistrationPlanSetV1,
        StrongImmortalObjectRegistrationPlanBuildError,
    > {
        StrongImmortalObjectRegistrationPlanSetV1::new(
            &self.foundation,
            &self.identities,
            &self.semantics,
        )
    }
}

fn object_artifacts(seed: u32) -> ObjectArtifacts {
    let object = CborIdentityRecord::from_key(immortal_key(seed)).unwrap();
    let object_definition = definition(
        StrongDefinitionEntity::immortal_object(object.id()),
        StrongDefinitionRole::ImmortalObject,
    );
    let object_primary = primary(&object_definition);
    let registration_definition = definition(
        StrongDefinitionEntity::immortal_object(object.id()),
        StrongDefinitionRole::ImmortalRegistration,
    );
    let registration_primary = primary(&registration_definition);
    ObjectArtifacts {
        object,
        object_definition,
        object_primary,
        registration_definition,
        registration_primary,
    }
}

fn definition(
    entity: StrongDefinitionEntity,
    role: StrongDefinitionRole,
) -> CborIdentityRecord<scoop_identity::ObjectDefinitionPlanId, ObjectDefinitionPlanKey> {
    CborIdentityRecord::from_key(
        ObjectDefinitionPlanKey::strong(ConeIdentity::SINGLE_FILE, entity, role).unwrap(),
    )
    .unwrap()
}

fn primary(
    definition: &CborIdentityRecord<
        scoop_identity::ObjectDefinitionPlanId,
        ObjectDefinitionPlanKey,
    >,
) -> CborIdentityRecord<scoop_identity::ObjectDefinitionAtomId, ObjectDefinitionAtomKey> {
    CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
        definition.id(),
        DefinitionAtomRole::Primary,
        DefinitionAtomSubkey::Singleton,
    ))
    .unwrap()
}

fn symbol(key: PersistentSymbolKey) -> PersistentSymbolRequest {
    PersistentSymbolRequest::new(key, LinkageClass::ConeStrong).unwrap()
}

fn string_global(path_index: u32, value: &str) -> Global {
    Global {
        address_kind: PointerKind::Managed,
        scan: RefScan::None,
        init: GlobalInit::StringConst {
            identity: ImmortalObjectIdentity::from_key(
                immortal_key(path_index),
                MaterializationRoot::cone_owned(),
            )
            .unwrap(),
            value: value.to_string(),
        },
    }
}

fn duplicate_global(global: &Global) -> Global {
    let GlobalInit::StringConst { identity, value } = &global.init else {
        unreachable!()
    };
    Global {
        address_kind: global.address_kind,
        scan: global.scan.clone(),
        init: GlobalInit::StringConst {
            identity: identity.clone(),
            value: value.clone(),
        },
    }
}

fn immortal_key(path_index: u32) -> ImmortalObjectKey {
    let property = PersistentPropertyId::from_source_declaration(&SourceDeclarationKey::property(
        source_site(),
        CanonicalIdentifier::new("text").unwrap(),
    ))
    .unwrap();
    ImmortalObjectKey::string_constant(
        ImmortalObjectOwner::Property(PropertyOwner::Property(property)),
        StructuralDefinitionPath::from_first(
            StructuralPathSegment::new(StructuralDefinitionSiteRole::StringConstant, path_index),
            [],
        ),
    )
}

fn exact_type(name: &str) -> PersistentExactTypeId {
    let source = SourceDeclarationKey::nominal(
        source_site(),
        CanonicalIdentifier::new(name).unwrap(),
        SourceNominalKind::Class,
        0,
    );
    let nominal = PersistentTypeId::from_source_declaration(&source).unwrap();
    PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(nominal)).unwrap()
}

fn source_site() -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}

#[test]
fn external_type_registrations_accept_ordinary_providers_and_reject_self_imports() {
    let ordinary = scoop_identity::ConeCoordinate::new("test", "string-provider", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    for provider in [ordinary, ConeIdentity::SINGLE_FILE] {
        let mut fixture = Fixture::new(Options::default());
        let objects = fixture
            .semantics
            .objects()
            .iter()
            .map(|object| {
                StrongImmortalObjectSemanticPlanV1::from_artifact(
                    object.object(),
                    object.symbol(),
                    object.object_size(),
                    object.required_alignment(),
                    ImmortalObjectTypeRegistrationRefV1::DependencyExternal {
                        provider,
                        exact: object.type_registration(),
                    },
                )
            })
            .collect();
        fixture.semantics = StrongImmortalObjectSemanticPlanSetV1::from_artifact(
            ConeIdentity::SINGLE_FILE,
            objects,
        );
        if provider == ConeIdentity::SINGLE_FILE {
            assert!(
                matches!(fixture.build(), Err(StrongImmortalObjectRegistrationPlanBuildError::SelfImport { provider: actual, .. }) if actual == provider)
            );
        } else {
            let plan = fixture.build().unwrap();
            assert!(plan.registrations().iter().all(|object| matches!(object.semantic().type_registration_ref(), ImmortalObjectTypeRegistrationRefV1::DependencyExternal { provider: actual, .. } if actual == provider)));
        }
    }
}
