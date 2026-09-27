use scoop_identity::{
    CallableBodyKey, CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope,
    DefinitionAtomRole, DefinitionAtomSubkey, DefinitionOwnerChain, DigestNodeId, DigestNodeKey,
    DigestPatchIntentKey, DigestSemanticFieldRole, LinkageClass, ObjectDefinitionAtomKey,
    ObjectDefinitionPlanKey, PackagePath, PersistentFunctionId, PersistentSymbolKey,
    PersistentSymbolRequest, PersistentSymbolRequestTable, RuntimeIdentityRecord,
    SourceDeclarationKey, SourceDeclarationSite, StrongCallableDefinitionOwner,
    StrongDefinitionEntity, StrongDefinitionRole,
};

use super::*;
use crate::{CanonicalLirFoundation, DigestNodeV1};

#[test]
fn joins_every_callable_to_its_entry_definition_and_digest_writers() {
    let fixture = Fixture::new(Options::default());

    let plans = fixture.build().unwrap();

    assert_eq!(plans.producer(), ConeIdentity::SINGLE_FILE);
    assert_eq!(plans.registrations().len(), 2);
    assert!(
        plans
            .registrations()
            .windows(2)
            .all(|pair| pair[0].body() < pair[1].body())
    );
    for plan in plans.registrations() {
        assert_eq!(
            plan.symbol().key(),
            PersistentSymbolKey::CallableRegistration(plan.body())
        );
        assert_eq!(
            plan.entry_symbol().key(),
            PersistentSymbolKey::CallableBody(plan.body())
        );
        let registration = fixture
            .digests
            .nodes()
            .iter()
            .find(|node| node.id() == plan.registration_fingerprint_node())
            .unwrap();
        assert_eq!(registration.direct_inputs().len(), 2);
        assert_eq!(
            registration.patch_intents()[0].id(),
            plan.registration_definition_patch()
        );
        let body = fixture
            .digests
            .nodes()
            .iter()
            .find(|node| node.id() == plan.body_definition_node())
            .unwrap();
        assert!(
            body.patch_intents()
                .iter()
                .any(|patch| patch.id() == plan.body_definition_patch())
        );
    }
}

#[test]
fn requires_complete_callable_registration_coverage() {
    let fixture = Fixture::new(Options {
        omit_last_registration: true,
        ..Options::default()
    });

    assert!(matches!(
        fixture.build(),
        Err(StrongCallableRegistrationPlanBuildError::CallableSet { expected, actual })
            if expected.len() == 2 && actual.len() == 1
    ));
}

#[test]
fn requires_both_strong_symbols_and_primary_atoms() {
    for options in [
        Options {
            omit_entry_symbol: true,
            ..Options::default()
        },
        Options {
            omit_registration_symbol: true,
            ..Options::default()
        },
    ] {
        assert!(matches!(
            Fixture::new(options).build(),
            Err(StrongCallableRegistrationPlanBuildError::MissingSymbol(_))
        ));
    }
    assert!(matches!(
        Fixture::new(Options {
            omit_registration_primary: true,
            ..Options::default()
        })
        .build(),
        Err(StrongCallableRegistrationPlanBuildError::PrimaryAtomSet { actual, .. })
            if actual.is_empty()
    ));
}

#[test]
fn requires_registration_object_and_both_definition_nodes() {
    for (options, expected_registration_object) in [
        (
            Options {
                omit_registration_object: true,
                ..Options::default()
            },
            true,
        ),
        (
            Options {
                omit_body_object: true,
                ..Options::default()
            },
            false,
        ),
    ] {
        let fixture = Fixture::new(options);
        let expected_atom = if expected_registration_object {
            fixture.registrations[0].registration_primary.id()
        } else {
            fixture.registrations[0].body_primary.id()
        };
        let error = fixture.build().unwrap_err();
        assert!(matches!(
            error,
            StrongCallableRegistrationPlanBuildError::MissingDigestNode(key)
                if key.owner_and_role()
                    == scoop_identity::DigestOwnerAndRoleKey::ObjectDefinition(expected_atom)
        ));
    }
}

#[test]
fn requires_a_leaf_registration_object_definition() {
    assert!(matches!(
        Fixture::new(Options {
            registration_object_input: true,
            ..Options::default()
        })
        .build(),
        Err(StrongCallableRegistrationPlanBuildError::RegistrationObjectInputs { .. })
    ));
    assert!(matches!(
        Fixture::new(Options {
            registration_object_patch: true,
            ..Options::default()
        })
        .build(),
        Err(StrongCallableRegistrationPlanBuildError::RegistrationObjectPatches { .. })
    ));
}

#[test]
fn requires_exact_registration_inputs_and_digest_writers() {
    assert!(matches!(
        Fixture::new(Options {
            omit_body_input: true,
            ..Options::default()
        })
        .build(),
        Err(StrongCallableRegistrationPlanBuildError::DirectInputs { .. })
    ));
    assert!(matches!(
        Fixture::new(Options {
            omit_registration_patch: true,
            ..Options::default()
        })
        .build(),
        Err(StrongCallableRegistrationPlanBuildError::PatchSet { .. })
    ));
    assert!(matches!(
        Fixture::new(Options {
            omit_body_patch: true,
            ..Options::default()
        })
        .build(),
        Err(StrongCallableRegistrationPlanBuildError::MissingPatch { .. })
    ));
}

#[derive(Clone, Copy, Default)]
struct Options {
    omit_last_registration: bool,
    omit_entry_symbol: bool,
    omit_registration_symbol: bool,
    omit_registration_primary: bool,
    omit_registration_object: bool,
    omit_body_object: bool,
    registration_object_input: bool,
    registration_object_patch: bool,
    omit_body_input: bool,
    omit_registration_patch: bool,
    omit_body_patch: bool,
}

struct RegistrationArtifacts {
    body: RuntimeIdentityRecord<scoop_identity::PersistentCallableBodyId>,
    body_definition:
        CborIdentityRecord<scoop_identity::ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    body_primary:
        CborIdentityRecord<scoop_identity::ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
    registration_definition:
        CborIdentityRecord<scoop_identity::ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    registration_primary:
        CborIdentityRecord<scoop_identity::ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
}

struct Fixture {
    foundation: ConeLirFoundation,
    identities: StrongRegistrationIdentitySurfaceV1,
    digests: StrongDigestFinalizationPlanV1,
    registrations: Vec<RegistrationArtifacts>,
}

impl Fixture {
    fn new(options: Options) -> Self {
        let registrations = ["alpha", "beta"]
            .into_iter()
            .map(registration_artifacts)
            .collect::<Vec<_>>();
        let mut canonical = CanonicalLirFoundation::empty();
        canonical
            .set_callable_bodies(
                registrations
                    .iter()
                    .map(|registration| registration.body.clone())
                    .collect(),
            )
            .unwrap();
        canonical
            .set_definition_plans(
                registrations
                    .iter()
                    .flat_map(|registration| {
                        [
                            registration.body_definition.clone(),
                            registration.registration_definition.clone(),
                        ]
                    })
                    .filter(|plan| {
                        !(options.omit_last_registration
                            && plan.id() == registrations[1].registration_definition.id())
                    })
                    .collect(),
            )
            .unwrap();
        canonical
            .set_definition_atoms(
                registrations
                    .iter()
                    .flat_map(|registration| {
                        [
                            registration.body_primary.clone(),
                            registration.registration_primary.clone(),
                        ]
                    })
                    .filter(|atom| {
                        !(options.omit_last_registration
                            && atom.key().plan() == registrations[1].registration_definition.id())
                            && !(options.omit_registration_primary
                                && atom.id() == registrations[0].registration_primary.id())
                    })
                    .collect(),
            )
            .unwrap();
        let symbols = registrations
            .iter()
            .flat_map(|registration| {
                [
                    PersistentSymbolRequest::new(
                        PersistentSymbolKey::CallableBody(registration.body.id()),
                        LinkageClass::ConeStrong,
                    )
                    .unwrap(),
                    PersistentSymbolRequest::new(
                        PersistentSymbolKey::CallableRegistration(registration.body.id()),
                        LinkageClass::ConeStrong,
                    )
                    .unwrap(),
                ]
            })
            .filter(|symbol| {
                !(options.omit_entry_symbol
                    && symbol.key()
                        == PersistentSymbolKey::CallableBody(registrations[0].body.id()))
                    && !(options.omit_registration_symbol
                        && symbol.key()
                            == PersistentSymbolKey::CallableRegistration(
                                registrations[0].body.id(),
                            ))
                    && !(options.omit_last_registration
                        && symbol.key()
                            == PersistentSymbolKey::CallableRegistration(
                                registrations[1].body.id(),
                            ))
            })
            .collect();
        canonical.set_symbol_requests(PersistentSymbolRequestTable::new(symbols).unwrap());
        let foundation = ConeLirFoundation::try_new(ConeIdentity::SINGLE_FILE, canonical).unwrap();
        let digests = digest_plan(&foundation, &registrations, options);
        let identities =
            StrongRegistrationIdentitySurfaceV1::from_foundation(&foundation, &digests).unwrap();
        Self {
            foundation,
            identities,
            digests,
            registrations,
        }
    }

    fn build(
        &self,
    ) -> Result<StrongCallableRegistrationPlanSetV1, StrongCallableRegistrationPlanBuildError> {
        StrongCallableRegistrationPlanSetV1::new(
            &self.foundation,
            &self.identities,
            StrongCallableRuntimeScanPlanSetV1::from_foundation_without_scans(&self.foundation)
                .unwrap(),
            &self.digests,
        )
    }
}

fn registration_artifacts(name: &str) -> RegistrationArtifacts {
    let declaration =
        PersistentFunctionId::from_source_declaration(&SourceDeclarationKey::function(
            SourceDeclarationSite::new(
                ConeIdentity::SINGLE_FILE,
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
        .unwrap();
    let body = RuntimeIdentityRecord::from_key(&CallableBodyKey::strong(
        StrongCallableDefinitionOwner::Function(declaration),
    ))
    .unwrap();
    let body_definition = definition(
        StrongDefinitionEntity::callable_body(body.id()),
        StrongDefinitionRole::CallableBody,
    );
    let body_primary = primary(&body_definition);
    let registration_definition = definition(
        StrongDefinitionEntity::callable_body(body.id()),
        StrongDefinitionRole::CallableRegistration,
    );
    let registration_primary = primary(&registration_definition);
    RegistrationArtifacts {
        body,
        body_definition,
        body_primary,
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

fn digest_plan(
    foundation: &ConeLirFoundation,
    registrations: &[RegistrationArtifacts],
    options: Options,
) -> StrongDigestFinalizationPlanV1 {
    let mut nodes = Vec::new();
    let mut image_inputs = Vec::new();
    let registration_object_input = options.registration_object_input.then(|| {
        DigestNodeV1::new(
            DigestNodeKey::source_signature(registrations[0].body.id()),
            Vec::new(),
            Vec::new(),
        )
        .unwrap()
    });
    for (index, registration) in registrations.iter().enumerate() {
        let body_key = DigestNodeKey::object_definition(registration.body_primary.id());
        let body_source = DigestNodeId::from_key(&body_key).unwrap();
        let body_patch = DigestPatchIntentKey::new(
            body_source,
            registration.registration_definition.id(),
            DefinitionAtomRole::Primary,
            DigestSemanticFieldRole::CallableBodyDefinition,
        );
        let body = (!options.omit_body_object || index != 0).then(|| {
            DigestNodeV1::new(
                body_key,
                Vec::new(),
                if (options.omit_body_patch && index == 0)
                    || (options.omit_last_registration && index == 1)
                    || (options.omit_registration_primary && index == 0)
                {
                    Vec::new()
                } else {
                    vec![body_patch]
                },
            )
            .unwrap()
        });

        if options.omit_last_registration && index == 1 {
            nodes.push(body.unwrap());
            continue;
        }
        let object_key = DigestNodeKey::object_definition(registration.registration_primary.id());
        let object_source = DigestNodeId::from_key(&object_key).unwrap();
        let object = ((!options.omit_registration_object || index != 0)
            && (!options.omit_registration_primary || index != 0))
            .then(|| {
                DigestNodeV1::new(
                    object_key,
                    if options.registration_object_input && index == 0 {
                        vec![DigestInputRefV1::from_node(
                            registration_object_input.as_ref().unwrap(),
                        )]
                    } else {
                        Vec::new()
                    },
                    if options.registration_object_patch && index == 0 {
                        vec![DigestPatchIntentKey::new(
                            object_source,
                            registration.registration_definition.id(),
                            DefinitionAtomRole::Primary,
                            DigestSemanticFieldRole::DescriptorDefinition,
                        )]
                    } else {
                        Vec::new()
                    },
                )
                .unwrap()
            });
        let strong_key =
            DigestNodeKey::strong_registration(registration.registration_definition.id());
        let strong_source = DigestNodeId::from_key(&strong_key).unwrap();
        let mut inputs = Vec::new();
        if let Some(object) = &object {
            inputs.push(DigestInputRefV1::from_node(object));
        }
        if !options.omit_body_input || index != 0 {
            if let Some(body) = &body {
                inputs.push(DigestInputRefV1::from_node(body));
            }
        }
        let strong = DigestNodeV1::new(
            strong_key,
            inputs,
            if (options.omit_registration_patch || options.omit_registration_primary) && index == 0
            {
                Vec::new()
            } else {
                vec![DigestPatchIntentKey::new(
                    strong_source,
                    registration.registration_definition.id(),
                    DefinitionAtomRole::Primary,
                    DigestSemanticFieldRole::RegistrationDefinition,
                )]
            },
        )
        .unwrap();
        image_inputs.push(DigestInputRefV1::from_node(&strong));
        if let Some(body) = body {
            nodes.push(body);
        }
        if let Some(object) = object {
            nodes.push(object);
        }
        nodes.push(strong);
    }
    nodes.extend(registration_object_input);
    nodes.push(
        DigestNodeV1::new(
            DigestNodeKey::runtime_image(ConeIdentity::SINGLE_FILE),
            image_inputs,
            Vec::new(),
        )
        .unwrap(),
    );
    StrongDigestFinalizationPlanV1::new(nodes, foundation).unwrap()
}
