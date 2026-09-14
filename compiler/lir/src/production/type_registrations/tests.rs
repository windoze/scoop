use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope, DefinitionAtomRole,
    DefinitionAtomSubkey, DefinitionOwnerChain, DigestNodeId, DigestNodeKey, DigestPatchIntentKey,
    DigestSemanticFieldRole, DispatchTableKey, ExactTypeKey, LayoutKey, LinkageClass,
    ObjectDefinitionAtomKey, ObjectDefinitionPlanKey, PackagePath, PersistentDispatchTableId,
    PersistentExactTypeId, PersistentLayoutId, PersistentScanId, PersistentSymbolKey,
    PersistentSymbolRequest, PersistentSymbolRequestTable, PersistentTypeId, RepresentationRole,
    ScanKey, ScanRole, SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
    StrongDefinitionEntity, StrongDefinitionRole,
};

use super::*;
use crate::{
    CanonicalLirFoundation, DecodedStrongTypeRegistrationPlanV1, DigestNodeV1, RefScan,
    RuntimeTypeMappingRecord, StrongExternalLirBridgeSurfaceV1, TypeInstanceShapeV1,
};
use scoop_wire::{DecodeLimits, decode_canonical, encode};

#[test]
fn joins_every_type_to_its_descriptor_layout_and_digest_writers() {
    let fixture = Fixture::new(Options::default());

    let plans = fixture.build().unwrap();

    assert_eq!(plans.producer(), ConeIdentity::SINGLE_FILE);
    assert_eq!(
        plans.target(),
        &crate::LirTargetProfile::DARWIN_AARCH64.wire_id()
    );
    assert_eq!(plans.registrations().len(), 2);
    assert!(
        plans
            .registrations()
            .windows(2)
            .all(|pair| pair[0].exact_type() < pair[1].exact_type())
    );
    for plan in plans.registrations() {
        assert_eq!(
            plan.symbol().key(),
            PersistentSymbolKey::TypeRegistration(plan.exact_type())
        );
        assert_eq!(
            plan.descriptor_symbol().key(),
            PersistentSymbolKey::TypeDescriptor(plan.exact_type())
        );
        assert_eq!(
            plan.layout_symbol().key(),
            PersistentSymbolKey::Layout(plan.layout())
        );
        assert_eq!(
            plan.runtime_type(),
            RuntimeTypeMappingRecord::new(plan.exact_type())
                .unwrap()
                .runtime_type()
        );
        let registration = node(&fixture.digests, plan.registration_fingerprint_node());
        assert_eq!(registration.direct_inputs().len(), 3);
        assert_eq!(
            registration.patch_intents()[0].id(),
            plan.registration_definition_patch()
        );
        assert!(
            node(&fixture.digests, plan.descriptor_definition_node())
                .patch_intents()
                .iter()
                .any(|patch| patch.id() == plan.descriptor_definition_patch())
        );
        assert!(
            node(&fixture.digests, plan.layout_fingerprint_node())
                .patch_intents()
                .iter()
                .any(|patch| patch.id() == plan.layout_fingerprint_patch())
        );
    }
}

#[test]
fn wire_reader_reconstructs_the_complete_type_descriptor_semantics() {
    let fixture = Fixture::new(Options::default());
    let plans = fixture.build().unwrap();
    let decoded = plans
        .registrations()
        .iter()
        .map(|plan| {
            decode_canonical::<DecodedStrongTypeRegistrationPlanV1>(
                &encode(plan).unwrap(),
                DecodeLimits::default(),
            )
            .unwrap()
        })
        .collect();
    let external_bridges =
        StrongExternalLirBridgeSurfaceV1::try_new(ConeIdentity::SINGLE_FILE, Vec::new()).unwrap();

    let validated = crate::validate_types(
        decoded,
        crate::LirTargetProfile::DARWIN_AARCH64,
        &fixture.foundation,
        &fixture.identities,
        &external_bridges,
        &fixture.digests,
    )
    .unwrap();

    assert_eq!(validated, fixture.semantics);
}

#[test]
fn requires_complete_type_registration_coverage() {
    let fixture = Fixture::new(Options {
        omit_last_registration: true,
        ..Options::default()
    });

    assert!(matches!(
        fixture.build(),
        Err(StrongTypeRegistrationPlanBuildError::TypeSet { expected, actual })
            if expected.len() == 2 && actual.len() == 1
    ));
}

#[test]
fn requires_runtime_type_and_managed_object_layout_relations() {
    assert!(matches!(
        Fixture::new(Options {
            omit_first_runtime_type: true,
            ..Options::default()
        })
        .build(),
        Err(StrongTypeRegistrationPlanBuildError::RuntimeTypeSet { actual, .. })
            if actual.is_empty()
    ));
    assert!(matches!(
        Fixture::new(Options {
            omit_first_layout: true,
            ..Options::default()
        })
        .build(),
        Err(StrongTypeRegistrationPlanBuildError::MissingLayout(_))
    ));
}

#[test]
fn requires_registration_descriptor_and_layout_symbols() {
    for options in [
        Options {
            omit_registration_symbol: true,
            ..Options::default()
        },
        Options {
            omit_descriptor_symbol: true,
            ..Options::default()
        },
        Options {
            omit_layout_symbol: true,
            ..Options::default()
        },
    ] {
        assert!(matches!(
            Fixture::new(options).build(),
            Err(StrongTypeRegistrationPlanBuildError::MissingSymbol(_))
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
        Err(StrongTypeRegistrationPlanBuildError::RegistrationObjectInputs { .. })
    ));
    assert!(matches!(
        Fixture::new(Options {
            registration_object_patch: true,
            ..Options::default()
        })
        .build(),
        Err(StrongTypeRegistrationPlanBuildError::RegistrationObjectPatches { .. })
    ));
}

#[test]
fn requires_exact_registration_inputs_and_digest_writers() {
    assert!(matches!(
        Fixture::new(Options {
            omit_descriptor_input: true,
            ..Options::default()
        })
        .build(),
        Err(StrongTypeRegistrationPlanBuildError::DirectInputs { .. })
    ));
    assert!(matches!(
        Fixture::new(Options {
            omit_registration_patch: true,
            ..Options::default()
        })
        .build(),
        Err(StrongTypeRegistrationPlanBuildError::PatchSet { .. })
    ));
    assert!(matches!(
        Fixture::new(Options {
            omit_descriptor_patch: true,
            ..Options::default()
        })
        .build(),
        Err(StrongTypeRegistrationPlanBuildError::MissingPatch { .. })
    ));
    assert!(matches!(
        Fixture::new(Options {
            omit_layout_patch: true,
            ..Options::default()
        })
        .build(),
        Err(StrongTypeRegistrationPlanBuildError::MissingPatch { .. })
    ));
}

#[derive(Clone, Copy, Default)]
struct Options {
    omit_last_registration: bool,
    omit_first_runtime_type: bool,
    omit_first_layout: bool,
    omit_registration_symbol: bool,
    omit_descriptor_symbol: bool,
    omit_layout_symbol: bool,
    registration_object_input: bool,
    registration_object_patch: bool,
    omit_descriptor_input: bool,
    omit_registration_patch: bool,
    omit_descriptor_patch: bool,
    omit_layout_patch: bool,
}

struct TypeArtifacts {
    exact_type: PersistentExactTypeId,
    layout: CborIdentityRecord<PersistentLayoutId, LayoutKey>,
    scan: CborIdentityRecord<PersistentScanId, ScanKey>,
    vtable: CborIdentityRecord<PersistentDispatchTableId, DispatchTableKey>,
    descriptor_definition:
        CborIdentityRecord<scoop_identity::ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    descriptor_primary:
        CborIdentityRecord<scoop_identity::ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
    layout_definition:
        CborIdentityRecord<scoop_identity::ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    layout_primary:
        CborIdentityRecord<scoop_identity::ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
    registration_definition:
        CborIdentityRecord<scoop_identity::ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    registration_primary:
        CborIdentityRecord<scoop_identity::ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
}

struct Fixture {
    foundation: OdrFreeLirFoundation,
    identities: StrongRegistrationIdentitySurfaceV1,
    semantics: StrongTypeDescriptorSemanticPlanSetV1,
    digests: StrongDigestFinalizationPlanV1,
}

impl Fixture {
    fn new(options: Options) -> Self {
        let types = [type_artifacts(1), type_artifacts(2)];
        let mut canonical = CanonicalLirFoundation::empty();
        canonical
            .set_layouts(
                types
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| !(options.omit_first_layout && *index == 0))
                    .map(|(_, artifacts)| artifacts.layout.clone())
                    .collect(),
            )
            .unwrap();
        canonical
            .set_scans(
                types
                    .iter()
                    .map(|artifacts| artifacts.scan.clone())
                    .collect(),
            )
            .unwrap();
        canonical
            .set_dispatch_tables(
                types
                    .iter()
                    .map(|artifacts| artifacts.vtable.clone())
                    .collect(),
            )
            .unwrap();
        canonical
            .set_runtime_types(
                types
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| !(options.omit_first_runtime_type && *index == 0))
                    .map(|(_, artifacts)| {
                        RuntimeTypeMappingRecord::new(artifacts.exact_type).unwrap()
                    })
                    .collect(),
            )
            .unwrap();
        canonical
            .set_definition_plans(
                types
                    .iter()
                    .flat_map(|artifacts| {
                        [
                            artifacts.descriptor_definition.clone(),
                            artifacts.layout_definition.clone(),
                            artifacts.registration_definition.clone(),
                        ]
                    })
                    .filter(|plan| {
                        !(options.omit_last_registration
                            && plan.id() == types[1].registration_definition.id())
                    })
                    .collect(),
            )
            .unwrap();
        canonical
            .set_definition_atoms(
                types
                    .iter()
                    .flat_map(|artifacts| {
                        [
                            artifacts.descriptor_primary.clone(),
                            artifacts.layout_primary.clone(),
                            artifacts.registration_primary.clone(),
                        ]
                    })
                    .filter(|atom| {
                        !(options.omit_last_registration
                            && atom.key().plan() == types[1].registration_definition.id())
                    })
                    .collect(),
            )
            .unwrap();
        let symbols = types
            .iter()
            .flat_map(|artifacts| {
                [
                    PersistentSymbolRequest::new(
                        PersistentSymbolKey::TypeDescriptor(artifacts.exact_type),
                        LinkageClass::ConeStrong,
                    )
                    .unwrap(),
                    PersistentSymbolRequest::new(
                        PersistentSymbolKey::Layout(artifacts.layout.id()),
                        LinkageClass::ConeStrong,
                    )
                    .unwrap(),
                    PersistentSymbolRequest::new(
                        PersistentSymbolKey::TypeRegistration(artifacts.exact_type),
                        LinkageClass::ConeStrong,
                    )
                    .unwrap(),
                ]
            })
            .filter(|symbol| {
                !(options.omit_registration_symbol
                    && symbol.key() == PersistentSymbolKey::TypeRegistration(types[0].exact_type))
                    && !(options.omit_descriptor_symbol
                        && symbol.key() == PersistentSymbolKey::TypeDescriptor(types[0].exact_type))
                    && !(options.omit_layout_symbol
                        && symbol.key() == PersistentSymbolKey::Layout(types[0].layout.id()))
                    && !(options.omit_last_registration
                        && symbol.key()
                            == PersistentSymbolKey::TypeRegistration(types[1].exact_type))
            })
            .collect();
        canonical.set_symbol_requests(PersistentSymbolRequestTable::new(symbols).unwrap());
        let foundation =
            OdrFreeLirFoundation::try_new(ConeIdentity::SINGLE_FILE, canonical).unwrap();
        let digests = digest_plan(&foundation, &types, options);
        let identities =
            StrongRegistrationIdentitySurfaceV1::from_foundation(&foundation, &digests).unwrap();
        let semantics = StrongTypeDescriptorSemanticPlanSetV1::from_artifact(
            ConeIdentity::SINGLE_FILE,
            crate::LirTargetProfile::DARWIN_AARCH64.wire_id(),
            types
                .iter()
                .map(|artifacts| {
                    StrongTypeDescriptorSemanticPlanV1::from_artifact(
                        artifacts.exact_type,
                        format!("type-{}", artifacts.exact_type),
                        artifacts.layout.id(),
                        artifacts.scan.id(),
                        TypeInstanceShapeV1::fixed_object(
                            crate::LirTargetProfile::DARWIN_AARCH64,
                            16,
                            8,
                            RefScan::None,
                        )
                        .unwrap(),
                        None,
                        StrongTypeVtableSemanticPlanV1::from_artifact(
                            artifacts.vtable.id(),
                            Vec::new(),
                        ),
                        Vec::new(),
                    )
                })
                .collect(),
        );
        Self {
            foundation,
            identities,
            semantics,
            digests,
        }
    }

    fn build(
        &self,
    ) -> Result<StrongTypeRegistrationPlanSetV1, StrongTypeRegistrationPlanBuildError> {
        StrongTypeRegistrationPlanSetV1::new(
            crate::LirTargetProfile::DARWIN_AARCH64,
            &self.foundation,
            &self.identities,
            &self.semantics,
            &self.digests,
        )
    }
}

fn type_artifacts(seed: u8) -> TypeArtifacts {
    let exact_type = exact_type(&format!("Type{seed}"));
    let layout = CborIdentityRecord::from_key(LayoutKey::darwin_aarch64(
        exact_type,
        RepresentationRole::ManagedObject,
    ))
    .unwrap();
    let scan =
        CborIdentityRecord::from_key(ScanKey::new(layout.id(), ScanRole::ManagedObject)).unwrap();
    let vtable = CborIdentityRecord::from_key(DispatchTableKey::vtable(exact_type)).unwrap();
    let descriptor_definition = definition(
        StrongDefinitionEntity::exact_type(exact_type),
        StrongDefinitionRole::TypeDescriptor,
    );
    let descriptor_primary = primary(&descriptor_definition);
    let layout_definition = definition(
        StrongDefinitionEntity::layout(layout.id()),
        StrongDefinitionRole::Layout,
    );
    let layout_primary = primary(&layout_definition);
    let registration_definition = definition(
        StrongDefinitionEntity::exact_type(exact_type),
        StrongDefinitionRole::TypeRegistration,
    );
    let registration_primary = primary(&registration_definition);
    TypeArtifacts {
        exact_type,
        layout,
        scan,
        vtable,
        descriptor_definition,
        descriptor_primary,
        layout_definition,
        layout_primary,
        registration_definition,
        registration_primary,
    }
}

fn exact_type(name: &str) -> PersistentExactTypeId {
    let source = SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
        SourceNominalKind::Class,
        0,
    );
    let nominal = PersistentTypeId::from_source_declaration(&source).unwrap();
    PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(nominal)).unwrap()
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
    foundation: &OdrFreeLirFoundation,
    types: &[TypeArtifacts; 2],
    options: Options,
) -> StrongDigestFinalizationPlanV1 {
    let mut nodes = Vec::new();
    let mut image_inputs = Vec::new();
    for (index, artifacts) in types.iter().enumerate() {
        let descriptor_key = DigestNodeKey::object_definition(artifacts.descriptor_primary.id());
        let descriptor_source = DigestNodeId::from_key(&descriptor_key).unwrap();
        let layout_key = DigestNodeKey::layout(artifacts.layout.id());
        let layout_source = DigestNodeId::from_key(&layout_key).unwrap();

        let registration_present = !(options.omit_last_registration && index == 1);
        let descriptor = DigestNodeV1::new(
            descriptor_key,
            Vec::new(),
            if registration_present && !(options.omit_descriptor_patch && index == 0) {
                vec![DigestPatchIntentKey::new(
                    descriptor_source,
                    artifacts.registration_definition.id(),
                    DefinitionAtomRole::Primary,
                    DigestSemanticFieldRole::DescriptorDefinition,
                )]
            } else {
                Vec::new()
            },
        )
        .unwrap();
        let layout = (!options.omit_first_layout || index != 0).then(|| {
            DigestNodeV1::new(
                layout_key,
                Vec::new(),
                if registration_present && !(options.omit_layout_patch && index == 0) {
                    vec![DigestPatchIntentKey::new(
                        layout_source,
                        artifacts.registration_definition.id(),
                        DefinitionAtomRole::Primary,
                        DigestSemanticFieldRole::Layout,
                    )]
                } else {
                    Vec::new()
                },
            )
            .unwrap()
        });

        nodes.push(descriptor.clone());
        if let Some(layout) = &layout {
            nodes.push(layout.clone());
        }
        if !registration_present {
            continue;
        }

        let object_key = DigestNodeKey::object_definition(artifacts.registration_primary.id());
        let object_source = DigestNodeId::from_key(&object_key).unwrap();
        let object = DigestNodeV1::new(
            object_key,
            if options.registration_object_input && index == 0 {
                vec![DigestInputRefV1::from_node(layout.as_ref().unwrap())]
            } else {
                Vec::new()
            },
            if options.registration_object_patch && index == 0 {
                vec![DigestPatchIntentKey::new(
                    object_source,
                    artifacts.registration_definition.id(),
                    DefinitionAtomRole::Primary,
                    DigestSemanticFieldRole::CallableBodyDefinition,
                )]
            } else {
                Vec::new()
            },
        )
        .unwrap();
        let strong_key = DigestNodeKey::strong_registration(artifacts.registration_definition.id());
        let strong_source = DigestNodeId::from_key(&strong_key).unwrap();
        let mut inputs = vec![DigestInputRefV1::from_node(&object)];
        if !options.omit_descriptor_input || index != 0 {
            inputs.push(DigestInputRefV1::from_node(&descriptor));
        }
        if let Some(layout) = &layout {
            inputs.push(DigestInputRefV1::from_node(layout));
        }
        let strong = DigestNodeV1::new(
            strong_key,
            inputs,
            if options.omit_registration_patch && index == 0 {
                Vec::new()
            } else {
                vec![DigestPatchIntentKey::new(
                    strong_source,
                    artifacts.registration_definition.id(),
                    DefinitionAtomRole::Primary,
                    DigestSemanticFieldRole::RegistrationDefinition,
                )]
            },
        )
        .unwrap();
        image_inputs.push(DigestInputRefV1::from_node(&strong));
        nodes.push(object);
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

fn node(plan: &StrongDigestFinalizationPlanV1, id: DigestNodeId) -> &DigestNodeV1 {
    plan.nodes().iter().find(|node| node.id() == id).unwrap()
}
