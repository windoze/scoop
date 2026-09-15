use scoop_identity::{
    CborIdentityRecord, ConeIdentity, CoreBuiltinNominal, DecodedCborIdentityRecord,
    DecodedSourceDeclarationKey, DefinitionAtomRole, DefinitionAtomSubkey, DigestNodeKey,
    ExactTypeKey, GeneratedNominalKey, IdentityLayer, LayoutKey, LinkageClass,
    ObjectDefinitionAtomKey, ObjectDefinitionPlanKey, PendingIdentityValidation,
    PersistentExactTypeId, PersistentLayoutId, PersistentScanId, PersistentSymbolKey,
    PersistentSymbolRequest, PersistentSymbolRequestTable, PersistentTypeId, RepresentationRole,
    ScanKey, ScanRole, SourceDeclarationKey, StrongDefinitionEntity, StrongDefinitionRole,
    ValidatedIdentityGraph,
};
use scoop_wire::{DecodeLimits, decode_canonical, encode};

use super::*;
use crate::{
    CanonicalLirFoundation, DigestInputRefV1, DigestNodeV1, OdrFreeLirFoundation,
    StrongDigestFinalizationPlanV1, StrongRegistrationIdentitySurfaceV1,
};

#[test]
fn struct_subject_materializes_all_eight_roles_and_round_trips() {
    let fixture = fixture(CoreBuiltinNominal::Unit);
    let plan = ParamFreeShapeSupportPlanSetV1::from_core_sources(
        [&fixture.source],
        &fixture.foundation,
        &fixture.registrations,
    )
    .unwrap();
    let closure = &plan.closures()[0];
    assert_eq!(closure.root(), ConeIdentity::CORE);
    assert!(closure.roles().source_nominal().available().is_some());
    assert!(closure.roles().value_layout().available().is_some());
    assert!(closure.roles().ref_scan().available().is_some());
    assert!(closure.roles().type_descriptor().available().is_some());
    assert!(closure.roles().type_registration().available().is_some());
    assert!(closure.roles().boxed_value().available().is_some());
    assert!(closure.roles().coroutine_step().available().is_some());
    assert!(closure.roles().coroutine_slot().available().is_some());

    let bytes = encode(&plan).unwrap();
    let decoded: DecodedParamFreeShapeSupportPlanSetV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    let mut identities = source_graph(&fixture.source);
    assert_eq!(
        decoded
            .validate(
                [&fixture.source],
                &mut identities,
                &fixture.foundation,
                &fixture.registrations,
            )
            .unwrap(),
        plan
    );
}

#[test]
fn reference_subject_closes_only_the_boxed_value_role() {
    let fixture = fixture(CoreBuiltinNominal::Any);
    let plan = ParamFreeShapeSupportPlanSetV1::from_core_sources(
        [&fixture.source],
        &fixture.foundation,
        &fixture.registrations,
    )
    .unwrap();
    let roles = plan.closures()[0].roles();
    assert_eq!(
        roles.boxed_value(),
        &ShapeSupportAvailabilityV1::NotApplicable(
            ClosedShapeSupportReasonV1::ReferenceNominalRequiresNoBox
        )
    );
    assert!(roles.coroutine_step().available().is_some());
    assert!(roles.coroutine_slot().available().is_some());
}

#[test]
fn availability_and_empty_plan_have_fixed_wire_shapes() {
    let source = CoreBuiltinNominal::Unit.identity_record().id();
    let available = ShapeSupportAvailabilityV1::Available(source);
    assert_eq!(
        hex(&encode(&available).unwrap()),
        format!("a20001015820{source}")
    );
    assert_eq!(
        encode(
            &ShapeSupportAvailabilityV1::<PersistentTypeId>::NotApplicable(
                ClosedShapeSupportReasonV1::ReferenceNominalRequiresNoBox,
            )
        )
        .unwrap(),
        b"\xa2\x00\x02\x01\x01"
    );

    let foundation =
        OdrFreeLirFoundation::try_new(ConeIdentity::CORE, CanonicalLirFoundation::empty()).unwrap();
    let digests = StrongDigestFinalizationPlanV1::new(
        vec![
            DigestNodeV1::new(
                DigestNodeKey::runtime_image(ConeIdentity::CORE),
                Vec::new(),
                Vec::new(),
            )
            .unwrap(),
        ],
        &foundation,
    )
    .unwrap();
    let registrations =
        StrongRegistrationIdentitySurfaceV1::from_foundation(&foundation, &digests).unwrap();
    let empty = ParamFreeShapeSupportPlanSetV1::from_core_sources(
        std::iter::empty(),
        &foundation,
        &registrations,
    )
    .unwrap();
    assert_eq!(encode(&empty).unwrap(), b"\x80");
}

#[test]
fn roles_wire_is_closed_to_eight_fields() {
    let fixture = fixture(CoreBuiltinNominal::Unit);
    let plan = ParamFreeShapeSupportPlanSetV1::from_core_sources(
        [&fixture.source],
        &fixture.foundation,
        &fixture.registrations,
    )
    .unwrap();
    let bytes = encode(plan.closures()[0].roles()).unwrap();
    assert_eq!(bytes[0], 0xa8);

    let mut old_ten_field_wire = bytes;
    old_ten_field_wire[0] = 0xaa;
    assert!(
        decode_canonical::<DecodedParamFreeShapeSupportRolesV1>(
            &old_ten_field_wire,
            DecodeLimits::default(),
        )
        .is_err()
    );
}

#[test]
fn reader_rebuilds_roles_instead_of_accepting_a_checked_but_wrong_closure() {
    let fixture = fixture(CoreBuiltinNominal::Unit);
    let plan = ParamFreeShapeSupportPlanSetV1::from_core_sources(
        [&fixture.source],
        &fixture.foundation,
        &fixture.registrations,
    )
    .unwrap();
    let mut decoded: DecodedParamFreeShapeSupportPlanSetV1 =
        decode_canonical(&encode(&plan).unwrap(), DecodeLimits::default()).unwrap();
    decoded.closures[0].roles.boxed_value = DecodedShapeSupportAvailabilityV1::NotApplicable(
        ClosedShapeSupportReasonV1::ReferenceNominalRequiresNoBox,
    );
    let mut identities = source_graph(&fixture.source);
    assert!(matches!(
        decoded.validate(
            [&fixture.source],
            &mut identities,
            &fixture.foundation,
            &fixture.registrations,
        ),
        Err(ParamFreeShapeSupportValidationError::PlanMismatch)
    ));
}

#[test]
fn reader_requires_complete_authoritative_source_coverage() {
    let fixture = fixture(CoreBuiltinNominal::Any);
    let plan = ParamFreeShapeSupportPlanSetV1::from_core_sources(
        [&fixture.source],
        &fixture.foundation,
        &fixture.registrations,
    )
    .unwrap();
    let mut decoded: DecodedParamFreeShapeSupportPlanSetV1 =
        decode_canonical(&encode(&plan).unwrap(), DecodeLimits::default()).unwrap();
    decoded.closures.clear();
    let mut identities = source_graph(&fixture.source);
    assert!(matches!(
        decoded.validate(
            [&fixture.source],
            &mut identities,
            &fixture.foundation,
            &fixture.registrations,
        ),
        Err(ParamFreeShapeSupportValidationError::PlanMismatch)
    ));
}

#[test]
fn reader_rejects_a_non_core_root_and_non_closed_availability_sum() {
    let fixture = fixture(CoreBuiltinNominal::Any);
    let plan = ParamFreeShapeSupportPlanSetV1::from_core_sources(
        [&fixture.source],
        &fixture.foundation,
        &fixture.registrations,
    )
    .unwrap();
    let mut decoded: DecodedParamFreeShapeSupportPlanSetV1 =
        decode_canonical(&encode(&plan).unwrap(), DecodeLimits::default()).unwrap();
    decoded.closures[0].root = decode_canonical(
        &encode(&ConeIdentity::SINGLE_FILE).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap();
    let mut identities = source_graph(&fixture.source);
    assert!(matches!(
        decoded.validate(
            [&fixture.source],
            &mut identities,
            &fixture.foundation,
            &fixture.registrations,
        ),
        Err(ParamFreeShapeSupportValidationError::WrongRoot)
    ));

    assert!(
        decode_canonical::<
            DecodedShapeSupportAvailabilityV1<
                scoop_identity::DecodedPersistentId<PersistentTypeId>,
            >,
        >(b"\xa2\x00\x03\x01\x01", DecodeLimits::default(),)
        .is_err()
    );
    assert!(
        decode_canonical::<
            DecodedShapeSupportAvailabilityV1<
                scoop_identity::DecodedPersistentId<PersistentTypeId>,
            >,
        >(b"\xa3\x00\x02\x01\x01\x02\x01", DecodeLimits::default(),)
        .is_err()
    );
}

#[test]
fn builder_rejects_a_missing_strong_definition() {
    let fixture = fixture_parts(CoreBuiltinNominal::Unit);
    let missing = fixture
        .plans
        .iter()
        .find(|record| {
            record.key().definition_role()
                == scoop_identity::ObjectDefinitionPlanRole::Strong(
                    StrongDefinitionRole::TypeDescriptor,
                )
                && matches!(
                    record.key().owner(),
                    scoop_identity::ObjectDefinitionPlanOwner::Strong {
                        entity,
                        ..
                    } if entity == StrongDefinitionEntity::exact_type(fixture.owner)
                )
        })
        .unwrap()
        .id();
    let fixture = finish_fixture(fixture, Some(missing));
    assert!(matches!(
        ParamFreeShapeSupportPlanSetV1::from_core_sources(
            [&fixture.source],
            &fixture.foundation,
            &fixture.registrations,
        ),
        Err(ParamFreeShapeSupportBuildError::MissingDefinition(id)) if id == missing
    ));
}

#[test]
fn non_core_foundation_cannot_build_the_core_branch() {
    let foundation =
        OdrFreeLirFoundation::try_new(ConeIdentity::SINGLE_FILE, CanonicalLirFoundation::empty())
            .unwrap();
    let digests = StrongDigestFinalizationPlanV1::new(
        vec![
            DigestNodeV1::new(
                DigestNodeKey::runtime_image(ConeIdentity::SINGLE_FILE),
                Vec::new(),
                Vec::new(),
            )
            .unwrap(),
        ],
        &foundation,
    )
    .unwrap();
    let registrations =
        StrongRegistrationIdentitySurfaceV1::from_foundation(&foundation, &digests).unwrap();
    assert!(matches!(
        ParamFreeShapeSupportPlanSetV1::from_core_sources(
            [&CoreBuiltinNominal::Unit.declaration_key()],
            &foundation,
            &registrations,
        ),
        Err(ParamFreeShapeSupportBuildError::ProducerNotCore(
            ConeIdentity::SINGLE_FILE
        ))
    ));
}

struct Fixture {
    source: SourceDeclarationKey,
    foundation: OdrFreeLirFoundation,
    registrations: StrongRegistrationIdentitySurfaceV1,
}

struct FixtureParts {
    source: SourceDeclarationKey,
    owner: PersistentExactTypeId,
    exact_types: Vec<CborIdentityRecord<PersistentExactTypeId, ExactTypeKey>>,
    layouts: Vec<CborIdentityRecord<PersistentLayoutId, LayoutKey>>,
    scans: Vec<CborIdentityRecord<PersistentScanId, ScanKey>>,
    plans: Vec<CborIdentityRecord<scoop_identity::ObjectDefinitionPlanId, ObjectDefinitionPlanKey>>,
    atoms: Vec<CborIdentityRecord<scoop_identity::ObjectDefinitionAtomId, ObjectDefinitionAtomKey>>,
    symbols: Vec<PersistentSymbolRequest>,
}

fn fixture(builtin: CoreBuiltinNominal) -> Fixture {
    finish_fixture(fixture_parts(builtin), None)
}

fn fixture_parts(builtin: CoreBuiltinNominal) -> FixtureParts {
    let source = builtin.declaration_key();
    let source_nominal = PersistentTypeId::from_source_declaration(&source).unwrap();
    let owner = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(source_nominal)).unwrap();
    let mut parts = FixtureParts {
        source,
        owner,
        exact_types: Vec::new(),
        layouts: Vec::new(),
        scans: Vec::new(),
        plans: Vec::new(),
        atoms: Vec::new(),
        symbols: Vec::new(),
    };
    add_exact_subject(&mut parts, source_nominal);
    if builtin == CoreBuiltinNominal::Unit {
        let boxed = PersistentTypeId::from_generated_key(&GeneratedNominalKey::BoxedValue {
            payload: owner,
        })
        .unwrap();
        add_exact_subject(&mut parts, boxed);
    }
    for key in [
        GeneratedNominalKey::CoroutineStep { result: owner },
        GeneratedNominalKey::CoroutineSlot { value: owner },
    ] {
        add_exact_subject(
            &mut parts,
            PersistentTypeId::from_generated_key(&key).unwrap(),
        );
    }
    parts
}

fn add_exact_subject(parts: &mut FixtureParts, nominal: PersistentTypeId) {
    let exact = CborIdentityRecord::from_key(ExactTypeKey::Nominal(nominal)).unwrap();
    let layout = CborIdentityRecord::from_key(LayoutKey::darwin_aarch64(
        exact.id(),
        RepresentationRole::ManagedValue,
    ))
    .unwrap();
    let scan =
        CborIdentityRecord::from_key(ScanKey::new(layout.id(), ScanRole::InlineValue)).unwrap();
    parts.exact_types.push(exact.clone());
    parts.layouts.push(layout.clone());
    parts.scans.push(scan.clone());
    for (entity, role, symbol) in [
        (
            StrongDefinitionEntity::layout(layout.id()),
            StrongDefinitionRole::Layout,
            PersistentSymbolKey::Layout(layout.id()),
        ),
        (
            StrongDefinitionEntity::scan(scan.id()),
            StrongDefinitionRole::ScanProgram,
            PersistentSymbolKey::ScanProgram(scan.id()),
        ),
        (
            StrongDefinitionEntity::exact_type(exact.id()),
            StrongDefinitionRole::TypeDescriptor,
            PersistentSymbolKey::TypeDescriptor(exact.id()),
        ),
        (
            StrongDefinitionEntity::exact_type(exact.id()),
            StrongDefinitionRole::TypeRegistration,
            PersistentSymbolKey::TypeRegistration(exact.id()),
        ),
    ] {
        add_definition(parts, entity, role, symbol);
    }
}

fn add_definition(
    parts: &mut FixtureParts,
    entity: StrongDefinitionEntity,
    role: StrongDefinitionRole,
    symbol: PersistentSymbolKey,
) {
    let plan = CborIdentityRecord::from_key(
        ObjectDefinitionPlanKey::strong(ConeIdentity::CORE, entity, role).unwrap(),
    )
    .unwrap();
    let atom = CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
        plan.id(),
        DefinitionAtomRole::Primary,
        DefinitionAtomSubkey::Singleton,
    ))
    .unwrap();
    parts.plans.push(plan);
    parts.atoms.push(atom);
    parts
        .symbols
        .push(PersistentSymbolRequest::new(symbol, LinkageClass::ConeStrong).unwrap());
}

fn finish_fixture(
    mut parts: FixtureParts,
    omitted_plan: Option<scoop_identity::ObjectDefinitionPlanId>,
) -> Fixture {
    if let Some(omitted) = omitted_plan {
        parts.plans.retain(|record| record.id() != omitted);
        parts.atoms.retain(|record| record.key().plan() != omitted);
    }
    let mut canonical = CanonicalLirFoundation::empty();
    canonical.set_exact_types(parts.exact_types).unwrap();
    canonical.set_layouts(parts.layouts).unwrap();
    canonical.set_scans(parts.scans).unwrap();
    canonical.set_definition_plans(parts.plans).unwrap();
    canonical.set_definition_atoms(parts.atoms).unwrap();
    canonical.set_symbol_requests(PersistentSymbolRequestTable::new(parts.symbols).unwrap());
    let foundation = OdrFreeLirFoundation::try_new(ConeIdentity::CORE, canonical).unwrap();
    let mut nodes = foundation
        .definition_plans()
        .iter()
        .filter(|record| {
            matches!(
                record.key().definition_role(),
                scoop_identity::ObjectDefinitionPlanRole::Strong(
                    StrongDefinitionRole::TypeRegistration
                )
            )
        })
        .map(|record| {
            DigestNodeV1::new(
                DigestNodeKey::strong_registration(record.id()),
                Vec::new(),
                Vec::new(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let inputs = nodes.iter().map(DigestInputRefV1::from_node).collect();
    nodes.push(
        DigestNodeV1::new(
            DigestNodeKey::runtime_image(ConeIdentity::CORE),
            inputs,
            Vec::new(),
        )
        .unwrap(),
    );
    let digests = StrongDigestFinalizationPlanV1::new(nodes, &foundation).unwrap();
    let registrations =
        StrongRegistrationIdentitySurfaceV1::from_foundation(&foundation, &digests).unwrap();
    Fixture {
        source: parts.source,
        foundation,
        registrations,
    }
}

fn source_graph(source: &SourceDeclarationKey) -> ValidatedIdentityGraph {
    let record = CborIdentityRecord::<PersistentTypeId, _>::from_key(source.clone()).unwrap();
    let decoded: DecodedCborIdentityRecord<PersistentTypeId, DecodedSourceDeclarationKey> =
        decode_canonical(&encode(&record).unwrap(), DecodeLimits::default()).unwrap();
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(ConeIdentity::CORE).unwrap();
    pending.register(IdentityLayer::Hir, &decoded).unwrap();
    pending.resolve(&decoded).unwrap();
    pending.finish().unwrap()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
