use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope, DefinitionAtomRole,
    DefinitionAtomSubkey, DefinitionOwnerChain, DigestNodeId, DigestNodeKey, DigestPatchIntentKey,
    DigestSemanticFieldRole, ExactTypeKey, ImmortalObjectKey, ImmortalObjectOwner, LayoutKey,
    LinkageClass, ObjectDefinitionAtomKey, ObjectDefinitionPlanKey, PackagePath,
    PersistentExactTypeId, PersistentImmortalObjectId, PersistentLayoutId, PersistentPropertyId,
    PersistentScanId, PersistentStaticStorageId, PersistentSymbolKey, PersistentSymbolRequest,
    PersistentSymbolRequestTable, PersistentTypeId, PropertyOwner, RepresentationRole, ScanKey,
    ScanRole, SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind, StaticStorageKey,
    StrongDefinitionEntity, StrongDefinitionRole, StructuralDefinitionPath,
    StructuralDefinitionSiteRole, StructuralPathSegment,
};

use super::*;
use crate::{
    CanonicalLirFoundation, StaticImmortalRelocationPlanV1, StaticStorageScanKindV1,
    StrongStaticStorageInitialStatePlanV1, StrongStaticStorageSemanticPlanSetV1,
    StrongStaticStorageSemanticPlanV1,
};

#[test]
fn joins_every_storage_to_its_shape_definitions_and_digest_writers() {
    let fixture = Fixture::new(Options::default());

    let plans = fixture.build().unwrap();

    assert_eq!(plans.producer(), ConeIdentity::SINGLE_FILE);
    assert_eq!(plans.registrations().len(), 2);
    assert!(
        plans
            .registrations()
            .windows(2)
            .all(|pair| { pair[0].semantic().storage() < pair[1].semantic().storage() })
    );
    for plan in plans.registrations() {
        let semantic = plan.semantic();
        assert_eq!(
            plan.registration_symbol().key(),
            PersistentSymbolKey::RootRegistration(semantic.storage())
        );
        assert_eq!(
            plan.layout_symbol().key(),
            PersistentSymbolKey::Layout(semantic.layout())
        );
        assert_eq!(
            plan.scan_symbol().key(),
            PersistentSymbolKey::ScanProgram(semantic.scan())
        );
        assert_eq!(
            node(&fixture.digests, plan.registration_fingerprint_node())
                .direct_inputs()
                .len(),
            4
        );
        assert_eq!(
            node(&fixture.digests, plan.registration_fingerprint_node()).patch_intents()[0].id(),
            plan.registration_definition_patch()
        );
        assert!(
            node(&fixture.digests, plan.layout_fingerprint_node())
                .patch_intents()
                .iter()
                .any(|patch| patch.id() == plan.layout_fingerprint_patch())
        );
        assert!(
            node(&fixture.digests, plan.scan_fingerprint_node())
                .patch_intents()
                .iter()
                .any(|patch| patch.id() == plan.scan_fingerprint_patch())
        );
    }
}

#[test]
fn requires_complete_static_storage_registration_coverage() {
    let fixture = Fixture::new(Options {
        omit_last_registration: true,
        ..Options::default()
    });

    assert!(matches!(
        fixture.build(),
        Err(StrongStaticStorageRegistrationPlanBuildError::StorageSet { expected, actual })
            if expected.len() == 2 && actual.len() == 1
    ));
}

#[test]
fn requires_every_typed_immortal_target_registration() {
    let mut fixture = Fixture::new(Options::default());
    let target = PersistentImmortalObjectId::from_key(&ImmortalObjectKey::string_constant(
        ImmortalObjectOwner::Property(PropertyOwner::Property(
            PersistentPropertyId::from_source_declaration(&SourceDeclarationKey::property(
                source_site(),
                CanonicalIdentifier::new("immortal").unwrap(),
            ))
            .unwrap(),
        )),
        StructuralDefinitionPath::from_first(
            StructuralPathSegment::new(StructuralDefinitionSiteRole::StringConstant, 0),
            [],
        ),
    ))
    .unwrap();
    fixture.semantics.storages[0].initial_state =
        StrongStaticStorageInitialStatePlanV1::EncodedStaticValue {
            initial_template: vec![0; 8],
            immortal_relocations: vec![StaticImmortalRelocationPlanV1 {
                pointer_offset: 0,
                target,
            }],
        };

    assert_eq!(
        fixture.build(),
        Err(
            StrongStaticStorageRegistrationPlanBuildError::ImmortalTargetRegistrationSet {
                storage: fixture.semantics.storages[0].storage(),
                target,
                actual: 0,
            }
        )
    );
}

#[test]
fn requires_storage_registration_and_shape_symbols() {
    for options in [
        Options {
            omit_storage_symbol: true,
            ..Options::default()
        },
        Options {
            omit_registration_symbol: true,
            ..Options::default()
        },
        Options {
            omit_layout_symbol: true,
            ..Options::default()
        },
        Options {
            omit_scan_symbol: true,
            ..Options::default()
        },
    ] {
        assert!(matches!(
            Fixture::new(options).build(),
            Err(StrongStaticStorageRegistrationPlanBuildError::MissingSymbol(_))
        ));
    }
}

#[test]
fn requires_leaf_registration_and_storage_object_nodes() {
    for options in [
        Options {
            registration_object_input: true,
            ..Options::default()
        },
        Options {
            storage_object_input: true,
            ..Options::default()
        },
        Options {
            storage_object_patch: true,
            ..Options::default()
        },
    ] {
        assert!(matches!(
            Fixture::new(options).build(),
            Err(StrongStaticStorageRegistrationPlanBuildError::ObjectLeafInputs { .. })
                | Err(StrongStaticStorageRegistrationPlanBuildError::ObjectLeafPatches { .. })
        ));
    }
}

#[test]
fn requires_the_initial_state_specific_associated_atoms() {
    assert!(matches!(
        Fixture::new(Options {
            omit_template_atom: true,
            ..Options::default()
        })
        .build(),
        Err(StrongStaticStorageRegistrationPlanBuildError::MissingAtom(
            _
        )) | Err(StrongStaticStorageRegistrationPlanBuildError::InitialArtifactSet { .. })
    ));
}

#[test]
fn requires_exact_registration_inputs_and_all_three_digest_writers() {
    assert!(matches!(
        Fixture::new(Options {
            omit_storage_input: true,
            ..Options::default()
        })
        .build(),
        Err(StrongStaticStorageRegistrationPlanBuildError::DirectInputs { .. })
    ));
    assert!(matches!(
        Fixture::new(Options {
            omit_registration_patch: true,
            ..Options::default()
        })
        .build(),
        Err(StrongStaticStorageRegistrationPlanBuildError::PatchSet { .. })
    ));
    for options in [
        Options {
            omit_layout_patch: true,
            ..Options::default()
        },
        Options {
            omit_scan_patch: true,
            ..Options::default()
        },
    ] {
        assert!(matches!(
            Fixture::new(options).build(),
            Err(StrongStaticStorageRegistrationPlanBuildError::MissingPatch { .. })
        ));
    }
}

#[derive(Clone, Copy, Default)]
struct Options {
    omit_last_registration: bool,
    omit_storage_symbol: bool,
    omit_registration_symbol: bool,
    omit_layout_symbol: bool,
    omit_scan_symbol: bool,
    registration_object_input: bool,
    storage_object_input: bool,
    storage_object_patch: bool,
    omit_storage_input: bool,
    omit_registration_patch: bool,
    omit_layout_patch: bool,
    omit_scan_patch: bool,
    omit_template_atom: bool,
}

struct StorageArtifacts {
    storage: CborIdentityRecord<PersistentStaticStorageId, StaticStorageKey>,
    storage_definition:
        CborIdentityRecord<scoop_identity::ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    storage_primary:
        CborIdentityRecord<scoop_identity::ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
    template_atom:
        CborIdentityRecord<scoop_identity::ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
    registration_definition:
        CborIdentityRecord<scoop_identity::ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    registration_primary:
        CborIdentityRecord<scoop_identity::ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
}

struct ShapeArtifacts {
    layout: CborIdentityRecord<PersistentLayoutId, LayoutKey>,
    scan: CborIdentityRecord<PersistentScanId, ScanKey>,
    layout_definition:
        CborIdentityRecord<scoop_identity::ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    layout_primary:
        CborIdentityRecord<scoop_identity::ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
    scan_definition:
        CborIdentityRecord<scoop_identity::ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    scan_primary:
        CborIdentityRecord<scoop_identity::ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
}

struct Fixture {
    foundation: OdrFreeLirFoundation,
    identities: StrongRegistrationIdentitySurfaceV1,
    semantics: StrongStaticStorageSemanticPlanSetV1,
    digests: StrongDigestFinalizationPlanV1,
}

impl Fixture {
    fn new(options: Options) -> Self {
        let shape = shape_artifacts();
        let storages = [storage_artifacts("first"), storage_artifacts("second")];
        let mut canonical = CanonicalLirFoundation::empty();
        canonical
            .set_static_storages(
                storages
                    .iter()
                    .map(|artifacts| artifacts.storage.clone())
                    .collect(),
            )
            .unwrap();
        canonical.set_layouts(vec![shape.layout.clone()]).unwrap();
        canonical.set_scans(vec![shape.scan.clone()]).unwrap();
        canonical
            .set_definition_plans(
                [
                    shape.layout_definition.clone(),
                    shape.scan_definition.clone(),
                ]
                .into_iter()
                .chain(storages.iter().flat_map(|artifacts| {
                    [
                        artifacts.storage_definition.clone(),
                        artifacts.registration_definition.clone(),
                    ]
                }))
                .filter(|plan| {
                    !(options.omit_last_registration
                        && plan.id() == storages[1].registration_definition.id())
                })
                .collect(),
            )
            .unwrap();
        canonical
            .set_definition_atoms(
                [shape.layout_primary.clone(), shape.scan_primary.clone()]
                    .into_iter()
                    .chain(storages.iter().flat_map(|artifacts| {
                        [
                            artifacts.storage_primary.clone(),
                            artifacts.template_atom.clone(),
                            artifacts.registration_primary.clone(),
                        ]
                    }))
                    .filter(|atom| {
                        !(options.omit_last_registration
                            && atom.key().plan() == storages[1].registration_definition.id())
                            && !(options.omit_template_atom
                                && atom.id() == storages[0].template_atom.id())
                    })
                    .collect(),
            )
            .unwrap();
        let symbols = [
            symbol(PersistentSymbolKey::Layout(shape.layout.id())),
            symbol(PersistentSymbolKey::ScanProgram(shape.scan.id())),
        ]
        .into_iter()
        .chain(storages.iter().flat_map(|artifacts| {
            [
                symbol(PersistentSymbolKey::StaticStorage(artifacts.storage.id())),
                symbol(PersistentSymbolKey::RootRegistration(
                    artifacts.storage.id(),
                )),
            ]
        }))
        .filter(|request| {
            !(options.omit_storage_symbol
                && request.key() == PersistentSymbolKey::StaticStorage(storages[0].storage.id()))
                && !(options.omit_registration_symbol
                    && request.key()
                        == PersistentSymbolKey::RootRegistration(storages[0].storage.id()))
                && !(options.omit_layout_symbol
                    && request.key() == PersistentSymbolKey::Layout(shape.layout.id()))
                && !(options.omit_scan_symbol
                    && request.key() == PersistentSymbolKey::ScanProgram(shape.scan.id()))
                && !(options.omit_last_registration
                    && request.key()
                        == PersistentSymbolKey::RootRegistration(storages[1].storage.id()))
        })
        .collect();
        canonical.set_symbol_requests(PersistentSymbolRequestTable::new(symbols).unwrap());

        let foundation =
            OdrFreeLirFoundation::try_new(ConeIdentity::SINGLE_FILE, canonical).unwrap();
        let digests = digest_plan(&foundation, &shape, &storages, options);
        let identities =
            StrongRegistrationIdentitySurfaceV1::from_foundation(&foundation, &digests).unwrap();
        let mut semantic_storages = storages
            .iter()
            .map(|artifacts| StrongStaticStorageSemanticPlanV1 {
                storage: artifacts.storage.id(),
                symbol: symbol(PersistentSymbolKey::StaticStorage(artifacts.storage.id())),
                layout: crate::LayoutIdentity::managed_value(
                    shape.layout.key().exact_type(),
                    crate::LirTargetProfile::DARWIN_AARCH64,
                    crate::MaterializationRoot::cone_owned(),
                )
                .unwrap()
                .into(),
                layout_provider: ConeIdentity::SINGLE_FILE,
                scan_program: crate::RefScan::References(vec![0]),
                scan_kind: StaticStorageScanKindV1::Recursive,
                byte_size: 8,
                allocation_extent: 8,
                required_alignment: 8,
                initial_state: StrongStaticStorageInitialStatePlanV1::EncodedStaticValue {
                    initial_template: vec![0; 8],
                    immortal_relocations: Vec::new(),
                },
            })
            .collect::<Vec<_>>();
        semantic_storages.sort_unstable_by_key(StrongStaticStorageSemanticPlanV1::storage);
        let semantics = StrongStaticStorageSemanticPlanSetV1 {
            producer: ConeIdentity::SINGLE_FILE,
            storages: semantic_storages,
        };
        Self {
            foundation,
            identities,
            semantics,
            digests,
        }
    }

    fn build(
        &self,
    ) -> Result<
        StrongStaticStorageRegistrationPlanSetV1,
        StrongStaticStorageRegistrationPlanBuildError,
    > {
        StrongStaticStorageRegistrationPlanSetV1::new(
            &self.foundation,
            &self.identities,
            &self.semantics,
            &self.digests,
        )
    }
}

fn shape_artifacts() -> ShapeArtifacts {
    let layout = CborIdentityRecord::from_key(LayoutKey::darwin_aarch64(
        exact_type("StorageValue"),
        RepresentationRole::ManagedValue,
    ))
    .unwrap();
    let scan =
        CborIdentityRecord::from_key(ScanKey::new(layout.id(), ScanRole::InlineValue)).unwrap();
    let layout_definition = definition(
        StrongDefinitionEntity::layout(layout.id()),
        StrongDefinitionRole::Layout,
    );
    let layout_primary = primary(&layout_definition);
    let scan_definition = definition(
        StrongDefinitionEntity::scan(scan.id()),
        StrongDefinitionRole::ScanProgram,
    );
    let scan_primary = primary(&scan_definition);
    ShapeArtifacts {
        layout,
        scan,
        layout_definition,
        layout_primary,
        scan_definition,
        scan_primary,
    }
}

fn storage_artifacts(name: &str) -> StorageArtifacts {
    let storage =
        CborIdentityRecord::from_key(StaticStorageKey::property_backing(PropertyOwner::Property(
            PersistentPropertyId::from_source_declaration(&SourceDeclarationKey::property(
                source_site(),
                CanonicalIdentifier::new(name).unwrap(),
            ))
            .unwrap(),
        )))
        .unwrap();
    let storage_definition = definition(
        StrongDefinitionEntity::static_storage(storage.id()),
        StrongDefinitionRole::StaticStorage,
    );
    let storage_primary = primary(&storage_definition);
    let template_atom = CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
        storage_definition.id(),
        DefinitionAtomRole::AddressTakenConstant,
        DefinitionAtomSubkey::StaticStorage(storage.id()),
    ))
    .unwrap();
    let registration_definition = definition(
        StrongDefinitionEntity::static_storage(storage.id()),
        StrongDefinitionRole::RootRegistration,
    );
    let registration_primary = primary(&registration_definition);
    StorageArtifacts {
        storage,
        storage_definition,
        storage_primary,
        template_atom,
        registration_definition,
        registration_primary,
    }
}

fn digest_plan(
    foundation: &OdrFreeLirFoundation,
    shape: &ShapeArtifacts,
    storages: &[StorageArtifacts; 2],
    options: Options,
) -> StrongDigestFinalizationPlanV1 {
    let layout_key = DigestNodeKey::layout(shape.layout.id());
    let layout_id = DigestNodeId::from_key(&layout_key).unwrap();
    let scan_key = DigestNodeKey::scan(shape.scan.id());
    let scan_id = DigestNodeId::from_key(&scan_key).unwrap();
    let registration_definitions = storages
        .iter()
        .enumerate()
        .filter(|(index, _)| !(options.omit_last_registration && *index == 1))
        .collect::<Vec<_>>();
    let layout = DigestNodeV1::new(
        layout_key,
        Vec::new(),
        registration_definitions
            .iter()
            .filter(|(index, _)| !(options.omit_layout_patch && *index == 0))
            .map(|(_, artifacts)| {
                DigestPatchIntentKey::new(
                    layout_id,
                    artifacts.registration_definition.id(),
                    DefinitionAtomRole::Primary,
                    DigestSemanticFieldRole::Layout,
                )
            })
            .collect(),
    )
    .unwrap();
    let scan = DigestNodeV1::new(
        scan_key,
        Vec::new(),
        registration_definitions
            .iter()
            .filter(|(index, _)| !(options.omit_scan_patch && *index == 0))
            .map(|(_, artifacts)| {
                DigestPatchIntentKey::new(
                    scan_id,
                    artifacts.registration_definition.id(),
                    DefinitionAtomRole::Primary,
                    DigestSemanticFieldRole::Scan,
                )
            })
            .collect(),
    )
    .unwrap();

    let mut nodes = vec![layout.clone(), scan.clone()];
    let mut image_inputs = Vec::new();
    for (index, artifacts) in storages.iter().enumerate() {
        let storage_key = DigestNodeKey::object_definition(artifacts.storage_primary.id());
        let storage_id = DigestNodeId::from_key(&storage_key).unwrap();
        let storage = DigestNodeV1::new(
            storage_key,
            if options.storage_object_input && index == 0 {
                vec![DigestInputRefV1::from_node(&layout)]
            } else {
                Vec::new()
            },
            if options.storage_object_patch && index == 0 {
                vec![DigestPatchIntentKey::new(
                    storage_id,
                    artifacts.registration_definition.id(),
                    DefinitionAtomRole::Primary,
                    DigestSemanticFieldRole::DescriptorDefinition,
                )]
            } else {
                Vec::new()
            },
        )
        .unwrap();
        nodes.push(storage.clone());
        if options.omit_last_registration && index == 1 {
            continue;
        }

        let registration_object = DigestNodeV1::new(
            DigestNodeKey::object_definition(artifacts.registration_primary.id()),
            if options.registration_object_input && index == 0 {
                vec![DigestInputRefV1::from_node(&scan)]
            } else {
                Vec::new()
            },
            Vec::new(),
        )
        .unwrap();
        let strong_key = DigestNodeKey::strong_registration(artifacts.registration_definition.id());
        let strong_id = DigestNodeId::from_key(&strong_key).unwrap();
        let mut inputs = vec![
            DigestInputRefV1::from_node(&registration_object),
            DigestInputRefV1::from_node(&layout),
            DigestInputRefV1::from_node(&scan),
        ];
        if !options.omit_storage_input || index != 0 {
            inputs.push(DigestInputRefV1::from_node(&storage));
        }
        let strong = DigestNodeV1::new(
            strong_key,
            inputs,
            if options.omit_registration_patch && index == 0 {
                Vec::new()
            } else {
                vec![DigestPatchIntentKey::new(
                    strong_id,
                    artifacts.registration_definition.id(),
                    DefinitionAtomRole::Primary,
                    DigestSemanticFieldRole::RegistrationDefinition,
                )]
            },
        )
        .unwrap();
        nodes.push(registration_object);
        image_inputs.push(DigestInputRefV1::from_node(&strong));
        nodes.push(strong);
    }
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

fn node(plan: &StrongDigestFinalizationPlanV1, id: DigestNodeId) -> &DigestNodeV1 {
    plan.nodes().iter().find(|node| node.id() == id).unwrap()
}

fn exact_type(name: &str) -> PersistentExactTypeId {
    let source = SourceDeclarationKey::nominal(
        source_site(),
        CanonicalIdentifier::new(name).unwrap(),
        SourceNominalKind::Struct,
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
