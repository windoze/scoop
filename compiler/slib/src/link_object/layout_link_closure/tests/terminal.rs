use scoop_lir::{
    CanonicalLirFoundation, NoShapeLinkSupportV1, OdrFreeLirFoundation, ShapeLinkProductionV1,
};

use super::fixture::empty_section;
use super::*;

mod support;
use support::*;
mod foreign;

#[test]
fn terminal_closure_replays_real_v2_provider_and_final_strong_owner() {
    let fixture = Fixture::new();
    let consumer = Consumer::new(&fixture.provider);
    let artifacts = fixture.artifacts(
        &consumer,
        &fixture.provider_owners,
        &consumer.defined_symbols,
    );
    let validated = validate_cross_cone_layout_terminal_closure_v1(&artifacts).unwrap();

    assert_eq!(validated.artifact_count(), 2);
    assert_eq!(validated.import_count(), 1);
    assert!(validated.artifact(fixture.provider_id()).is_some());
    assert!(validated.artifact(consumer.identity()).is_some());
}

#[test]
fn terminal_closure_requires_the_explicit_unique_provider() {
    assert!(matches!(
        validate_cross_cone_layout_terminal_closure_v1(&[]),
        Err(CrossConeLayoutTerminalValidationError::NoArtifacts)
    ));
    let fixture = Fixture::new();
    let consumer = Consumer::new(&fixture.provider);
    let mut artifacts = fixture.artifacts(
        &consumer,
        &fixture.provider_owners,
        &consumer.defined_symbols,
    );
    artifacts.remove(0);
    assert!(matches!(
        validate_cross_cone_layout_terminal_closure_v1(&artifacts),
        Err(CrossConeLayoutTerminalValidationError::MissingProvider {
            consumer: actual_consumer,
            provider,
        }) if actual_consumer == consumer.identity() && provider == fixture.provider_id()
    ));

    let mut artifacts = fixture.artifacts(
        &consumer,
        &fixture.provider_owners,
        &consumer.defined_symbols,
    );
    let duplicate = artifact(
        &fixture.provider,
        &fixture.provider.production,
        &fixture.provider.section,
        &fixture.provider_owners,
    );
    artifacts.push(duplicate);
    assert!(matches!(
        validate_cross_cone_layout_terminal_closure_v1(&artifacts),
        Err(CrossConeLayoutTerminalValidationError::DuplicateProvider { provider, .. })
            if provider == fixture.provider_id()
    ));
}

#[test]
fn terminal_closure_rejects_missing_subject_and_changed_contract() {
    let fixture = Fixture::new();
    let consumer = Consumer::new(&fixture.provider);
    let empty = empty_section(fixture.provider_id());
    let empty_production = replay_provider_production(&fixture.provider, &empty);
    let wrong_provider = artifact(
        &fixture.provider,
        &empty_production,
        &empty,
        &fixture.provider_owners,
    );
    let consumer_entry = consumer_artifact(&consumer, &consumer.defined_symbols);
    assert!(matches!(
        validate_cross_cone_layout_terminal_closure_v1(
            &[wrong_provider, consumer_entry]
        ),
        Err(CrossConeLayoutTerminalValidationError::ImportReplay { source, .. })
            if matches!(source.as_ref(), scoop_lir::ShapeLinkError::MissingSubject(_))
    ));

    let changed = changed_layout_section(&fixture.provider);
    let changed_production = replay_provider_production(&fixture.provider, &changed);
    let wrong_provider = artifact(
        &fixture.provider,
        &changed_production,
        &changed,
        &fixture.provider_owners,
    );
    let consumer_entry = consumer_artifact(&consumer, &consumer.defined_symbols);
    assert!(matches!(
        validate_cross_cone_layout_terminal_closure_v1(&[wrong_provider, consumer_entry]),
        Err(CrossConeLayoutTerminalValidationError::ContractMismatch(_))
    ));
}

#[test]
fn terminal_closure_rejects_missing_symbol_definition_and_wrong_artifact_provider() {
    let fixture = Fixture::new();
    let consumer = Consumer::new(&fixture.provider);
    let missing =
        OdrFreeLirFoundation::try_new(fixture.provider_id(), CanonicalLirFoundation::empty())
            .unwrap();
    let result =
        CrossConeLayoutTerminalArtifactV1::try_new(CrossConeLayoutTerminalArtifactPartsV1 {
            foundation: &missing,
            production: ShapeLinkProductionV1::Reader(&fixture.provider.production),
            ordinary: &fixture.provider.ordinary,
            section: &fixture.provider.section,
            support: &NoShapeLinkSupportV1,
            defined_symbols: &fixture.provider_owners,
        });
    assert!(result.is_ok());
    let missing_provider = result.unwrap();
    let consumer_entry = consumer_artifact(&consumer, &consumer.defined_symbols);
    assert!(matches!(
        validate_cross_cone_layout_terminal_closure_v1(
            &[missing_provider, consumer_entry]
        ),
        Err(CrossConeLayoutTerminalValidationError::ImportReplay { source, .. })
            if matches!(source.as_ref(), scoop_lir::ShapeLinkError::Definition(_))
    ));

    let missing_symbol = foundation_without_symbol(
        fixture.provider_id(),
        scoop_lir::ExternalStrongShapeSubjectV1::Layout(fixture.provider.layout),
    );
    let missing_provider =
        CrossConeLayoutTerminalArtifactV1::try_new(CrossConeLayoutTerminalArtifactPartsV1 {
            foundation: &missing_symbol,
            production: ShapeLinkProductionV1::Reader(&fixture.provider.production),
            ordinary: &fixture.provider.ordinary,
            section: &fixture.provider.section,
            support: &NoShapeLinkSupportV1,
            defined_symbols: &fixture.provider_owners,
        })
        .unwrap();
    let consumer_entry = consumer_artifact(&consumer, &consumer.defined_symbols);
    assert!(matches!(
        validate_cross_cone_layout_terminal_closure_v1(
            &[missing_provider, consumer_entry]
        ),
        Err(CrossConeLayoutTerminalValidationError::ImportReplay { source, .. })
            if matches!(source.as_ref(), scoop_lir::ShapeLinkError::Definition(
                scoop_lir::StrongShapeDefinitionError::MissingSymbol(_)
            ))
    ));

    let result =
        CrossConeLayoutTerminalArtifactV1::try_new(CrossConeLayoutTerminalArtifactPartsV1 {
            foundation: &fixture.provider.foundation,
            production: ShapeLinkProductionV1::Reader(&fixture.provider.production),
            ordinary: &fixture.provider.ordinary,
            section: &fixture.provider.section,
            support: &NoShapeLinkSupportV1,
            defined_symbols: &consumer.defined_symbols,
        });
    assert!(matches!(
        result,
        Err(CrossConeLayoutTerminalValidationError::ArtifactIdentityMismatch { .. })
    ));
}

#[test]
fn terminal_closure_requires_exact_final_symbol_owner_and_unique_ownership() {
    let fixture = Fixture::new();
    let consumer = Consumer::new(&fixture.provider);
    let wrong_symbols = owner_set_for_callable(fixture.provider_id(), "wrongLayoutOwner");
    let artifacts = fixture.artifacts(&consumer, &wrong_symbols, &consumer.defined_symbols);
    assert!(matches!(
        validate_cross_cone_layout_terminal_closure_v1(&artifacts),
        Err(CrossConeLayoutTerminalValidationError::MissingStrongDefinition { .. })
    ));

    let import = &consumer.section.selected().physical_imports().records()[0];
    let name = TARGET
        .contract()
        .native_symbol_normalization()
        .compiler_generated_object_symbol(import.expected_symbol().symbol().as_str())
        .into_bytes();
    let wrong_owner =
        owner_set_for_callable(fixture.provider_id(), "wrongOwnerKind").owners()[0].owner();
    let wrong_owners = fixture
        .provider_owners
        .replace_owner_for_test(&name, wrong_owner);
    let artifacts = fixture.artifacts(&consumer, &wrong_owners, &consumer.defined_symbols);
    assert!(matches!(
        validate_cross_cone_layout_terminal_closure_v1(&artifacts),
        Err(CrossConeLayoutTerminalValidationError::StrongOwnerMismatch { .. })
    ));

    let expected_owner = fixture.provider_owners.owners()[0].owner();
    let foreign = consumer
        .defined_symbols
        .insert_foreign_owner_for_test(name, expected_owner);
    let artifacts = fixture.artifacts(&consumer, &fixture.provider_owners, &foreign);
    assert!(matches!(
        validate_cross_cone_layout_terminal_closure_v1(&artifacts),
        Err(CrossConeLayoutTerminalValidationError::ForeignStrongDefinition {
            actual_owner,
            ..
        }) if actual_owner == consumer.identity()
    ));
}
