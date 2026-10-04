use scoop_identity::{
    CborIdentityRecord, ConeCoordinate, ConeIdentity, DefinitionAtomRole, DefinitionAtomSubkey,
    LinkageClass, ObjectDefinitionAtomKey, PersistentSymbolRequest, PersistentSymbolRequestTable,
};
use scoop_lir::{
    CanonicalLirFoundation, ConeLirFoundation, ConeProductionSectionV1, ConeProductionSectionV2,
    CrossConeLayoutAbiSectionV1, CrossConeLirBridgeSectionV1, DecodedConeProductionSectionV2,
    EntryProductionSourceV1, ExactValueLayoutV1, ScalarRepresentationKindV1,
    StrongInitializationDefinitionCatalogV2, StrongRegistrationProductionSurfaceV1,
    StrongTypeReferenceDefinitionsV2,
};
use scoop_wire::{decode_canonical, encode};

use super::super::fixture::{Provider, TARGET, exports};
use super::super::*;
use crate::link_object::strong_relocation_closure::tests::verified_member_without_relocations;
use crate::link_object::symbol_verification::tests::{
    fixture_for_producer, object_for_plan_with_branch_relocation,
};
use crate::{
    CanonicalDefinedLinkSymbolOwnerSetV1, CanonicalScoopLirObjectUnitSetV1,
    PlannedLinkObjectMemberSetV1, PlannedStrongObjectSymbolRoleV1, PlannedStrongObjectSymbolSetV1,
    validate_scoop_lir_llvm_22_1_object_envelope_v1,
    verify_current_cone_strong_relocation_closure_v1, verify_member_object_relocations_v1,
    verify_member_strong_object_definitions_v1,
};

pub(super) struct Fixture {
    pub(super) provider: Provider,
    pub(super) provider_owners: CanonicalDefinedLinkSymbolOwnerSetV1,
}

impl Fixture {
    pub(super) fn new() -> Self {
        let provider = Provider::new();
        let provider_owners = owner_set_for_subject(
            provider.foundation.producer(),
            scoop_lir::ExternalStrongShapeSubjectV1::Layout(provider.layout),
        );
        Self {
            provider,
            provider_owners,
        }
    }

    pub(super) fn provider_id(&self) -> ConeIdentity {
        self.provider.foundation.producer()
    }

    pub(super) fn artifacts<'a>(
        &'a self,
        consumer: &'a Consumer<'a>,
        provider_owners: &'a CanonicalDefinedLinkSymbolOwnerSetV1,
        consumer_owners: &'a CanonicalDefinedLinkSymbolOwnerSetV1,
    ) -> Vec<CrossConeLayoutTerminalArtifactV1<'a>> {
        vec![
            artifact(
                &self.provider,
                &self.provider.production,
                &self.provider.section,
                provider_owners,
            ),
            consumer_artifact(consumer, consumer_owners),
        ]
    }
}

pub(super) struct Consumer<'provider> {
    foundation: ConeLirFoundation,
    production: ConeProductionSectionV2,
    ordinary: CrossConeLirBridgeSectionV1,
    pub(super) section: CrossConeLayoutAbiSectionV1<'provider>,
    pub(super) defined_symbols: CanonicalDefinedLinkSymbolOwnerSetV1,
}

impl<'provider> Consumer<'provider> {
    pub(super) fn new(provider: &'provider Provider) -> Self {
        let coordinate = ConeCoordinate::new("test", "layout-terminal-consumer", "1.0.0").unwrap();
        let consumer = coordinate.identity().unwrap();
        let section = provider.consumer(consumer);
        let (canonical, old) =
            crate::link_decode::tests::strong_production_fixture(coordinate.clone(), &[]);
        let foundation = ConeLirFoundation::try_new(consumer, canonical).unwrap();
        let production = replay(&old, coordinate, &foundation, &section);
        let ordinary = CrossConeLirBridgeSectionV1::try_new(&foundation, vec![], vec![]).unwrap();
        let defined_symbols = owner_set_for_callable(consumer, "consumerOwner");
        Self {
            foundation,
            production,
            ordinary,
            section,
            defined_symbols,
        }
    }

    pub(super) fn identity(&self) -> ConeIdentity {
        self.foundation.producer()
    }
}

pub(super) fn artifact<'a>(
    provider: &'a Provider,
    production: &'a ConeProductionSectionV2,
    section: &'a CrossConeLayoutAbiSectionV1<'a>,
    owners: &'a CanonicalDefinedLinkSymbolOwnerSetV1,
) -> CrossConeLayoutTerminalArtifactV1<'a> {
    CrossConeLayoutTerminalArtifactV1::try_new(CrossConeLayoutTerminalArtifactPartsV1 {
        foundation: &provider.foundation,
        production,
        ordinary: &provider.ordinary,
        section,

        defined_symbols: owners,
    })
    .unwrap()
}

pub(super) fn consumer_artifact<'a>(
    consumer: &'a Consumer<'a>,
    owners: &'a CanonicalDefinedLinkSymbolOwnerSetV1,
) -> CrossConeLayoutTerminalArtifactV1<'a> {
    CrossConeLayoutTerminalArtifactV1::try_new(CrossConeLayoutTerminalArtifactPartsV1 {
        foundation: &consumer.foundation,
        production: &consumer.production,
        ordinary: &consumer.ordinary,
        section: &consumer.section,

        defined_symbols: owners,
    })
    .unwrap()
}

pub(super) fn replay_provider_production(
    provider: &Provider,
    section: &CrossConeLayoutAbiSectionV1<'_>,
) -> ConeProductionSectionV2 {
    let coordinate = ConeCoordinate::new("test", "layout-link-provider", "1.0.0").unwrap();
    let digests = provider.production.digest_finalization_plan().clone();
    let old = ConeProductionSectionV1::new(
        coordinate.clone(),
        &[scoop_identity::ConeIdentity::CORE],
        &provider.foundation,
        digests.clone(),
        StrongRegistrationProductionSurfaceV1::empty(TARGET, &provider.foundation, &digests)
            .unwrap(),
        EntryProductionSourceV1::Library,
        &[],
        provider.production.canonical_callable_definitions().clone(),
        scoop_lir::CanonicalShapeLirDefinitionsV1::new(Vec::new(), &provider.foundation).unwrap(),
    )
    .unwrap();
    replay(&old, coordinate, &provider.foundation, section)
}

fn replay(
    old: &ConeProductionSectionV1,
    coordinate: ConeCoordinate,
    foundation: &ConeLirFoundation,
    section: &CrossConeLayoutAbiSectionV1<'_>,
) -> ConeProductionSectionV2 {
    let producer = foundation.producer();
    let raw: DecodedConeProductionSectionV2 = decode_canonical(&encode(old).unwrap()).unwrap();
    raw.replay(
        coordinate,
        old.image_plan().dependencies(),
        TARGET,
        foundation,
        EntryProductionSourceV1::Library,
        &[],
        &StrongTypeReferenceDefinitionsV2::new(producer, &[], &[]).unwrap(),
        &StrongInitializationDefinitionCatalogV2::new(producer, &[]).unwrap(),
    )
    .unwrap()
    .validate_layout_abi(section)
    .unwrap()
}

pub(super) fn changed_layout_section(provider: &Provider) -> CrossConeLayoutAbiSectionV1<'static> {
    let record = provider.section.layouts().get(provider.layout).unwrap();
    let value = ExactValueLayoutV1::scalar(
        record.identity().clone(),
        ScalarRepresentationKindV1::Integer(scoop_lir::IntegerKind::SIGNED_8),
        &provider.foundation,
    )
    .unwrap();
    let exports = exports(&provider.foundation, vec![value.into()]);

    CrossConeLayoutAbiSectionV1::try_new(exports, &[], vec![], &[]).unwrap()
}

pub(super) fn owner_set_for_callable(
    producer: ConeIdentity,
    name: &str,
) -> CanonicalDefinedLinkSymbolOwnerSetV1 {
    let fixture = fixture_for_producer(producer, name);
    let closure = verify_current_cone_strong_relocation_closure_v1(vec![
        verified_member_without_relocations(&fixture),
    ])
    .unwrap();
    CanonicalDefinedLinkSymbolOwnerSetV1::from_verified_strong_closure(&closure).unwrap()
}

fn owner_set_for_subject(
    producer: ConeIdentity,
    subject: scoop_lir::ExternalStrongShapeSubjectV1,
) -> CanonicalDefinedLinkSymbolOwnerSetV1 {
    let (plan, symbol) = subject.expected_definition(producer).unwrap();
    let plan = CborIdentityRecord::from_key(plan).unwrap();
    let atom = CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
        plan.id(),
        DefinitionAtomRole::Primary,
        DefinitionAtomSubkey::Singleton,
    ))
    .unwrap();
    let mut canonical = CanonicalLirFoundation::empty();
    canonical.set_definition_plans(vec![plan.clone()]).unwrap();
    canonical.set_definition_atoms(vec![atom]).unwrap();
    canonical.set_symbol_requests(
        PersistentSymbolRequestTable::new(vec![
            PersistentSymbolRequest::new(symbol, LinkageClass::ConeStrong).unwrap(),
        ])
        .unwrap(),
    );
    let foundation = ConeLirFoundation::try_new(producer, canonical).unwrap();
    let surface = scoop_lir::ObjectSymbolSurfaceV1::from_foundation(&foundation).unwrap();
    let partition = scoop_lir::ProducerUnitPartitionV1::from_foundation(&foundation).unwrap();
    let member_plan = PlannedLinkObjectMemberSetV1::new(
        scoop_lir::LirTargetProfile::DARWIN_AARCH64,
        &partition,
        vec![CanonicalScoopLirObjectUnitSetV1::new(vec![plan.id()]).unwrap()],
        vec![],
    )
    .unwrap();
    let symbols = PlannedStrongObjectSymbolSetV1::new(TARGET, &surface, &member_plan)
        .unwrap()
        .members()[0]
        .clone();
    let object = object_for_plan_with_branch_relocation(
        &symbols,
        |role| match role {
            PlannedStrongObjectSymbolRoleV1::PrimaryDefinition { .. }
            | PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart { .. } => 0,
            PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd { .. } => 4,
        },
        None,
    );
    let sections = validate_scoop_lir_llvm_22_1_object_envelope_v1(&object.bytes)
        .unwrap()
        .into_sections();
    let definitions =
        verify_member_strong_object_definitions_v1(&object.bytes, sections, &symbols).unwrap();
    let relocations = verify_member_object_relocations_v1(definitions).unwrap();
    let closure = verify_current_cone_strong_relocation_closure_v1(vec![relocations]).unwrap();
    CanonicalDefinedLinkSymbolOwnerSetV1::from_verified_strong_closure(&closure).unwrap()
}

pub(super) fn foundation_without_symbol(
    producer: ConeIdentity,
    subject: scoop_lir::ExternalStrongShapeSubjectV1,
) -> ConeLirFoundation {
    let (plan, _) = subject.expected_definition(producer).unwrap();
    let plan = CborIdentityRecord::from_key(plan).unwrap();
    let atom = CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
        plan.id(),
        DefinitionAtomRole::Primary,
        DefinitionAtomSubkey::Singleton,
    ))
    .unwrap();
    let mut canonical = CanonicalLirFoundation::empty();
    canonical.set_definition_plans(vec![plan]).unwrap();
    canonical.set_definition_atoms(vec![atom]).unwrap();
    ConeLirFoundation::try_new(producer, canonical).unwrap()
}
