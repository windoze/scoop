use super::*;
use scoop_identity::{PendingIdentityValidation, PersistentTypeId};
use scoop_wire::{DecodeLimits, decode_canonical, encode};

mod fixture;
use fixture::*;

#[test]
fn empty_section_roundtrips_as_one_complete_six_field_product() {
    let exports = empty_exports(cone("empty"));
    let expected = exports.clone();
    let source = Source::default();
    let section =
        CrossConeLayoutAbiSectionV1::try_new(exports, &[], Vec::new(), &source, &mut meter())
            .unwrap();
    assert!(section.selected().is_empty());

    let bytes = encode(&section).unwrap();
    assert_eq!(bytes[0], 0xa6);
    let decoded: DecodedCrossConeLayoutAbiSectionV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    let mut identities = PendingIdentityValidation::new().finish().unwrap();
    let replayed = decoded
        .validate(
            &expected,
            &[],
            Vec::new(),
            &source,
            &mut identities,
            &mut meter(),
        )
        .unwrap();
    assert_eq!(encode(&replayed).unwrap(), bytes);
}

#[test]
fn nested_layout_use_selects_the_unique_terminal_record() {
    let remote = cone("remote");
    let consumer = cone("consumer");
    let (remote_value, remote_foundation) = empty_struct(remote, "RemoteValue");
    let remote_layout = remote_value.identity().layout();
    let terminal = section(
        exports(&remote_foundation, vec![remote_value.into()]),
        &[],
        &Source::default(),
    )
    .unwrap();

    let (local_value, local_foundation) = struct_with_field(
        consumer,
        "LocalValue",
        "remote",
        terminal.layouts().get(remote_layout).unwrap(),
    );
    let dependencies = [&terminal];
    let selected = section(
        exports(&local_foundation, vec![local_value.into()]),
        &dependencies,
        &Source::default(),
    )
    .unwrap();
    let target = LayoutAbiSemanticTargetV1::Layout(remote_layout);
    let reference = selected.selected().reference(remote, target).unwrap();
    assert_eq!(
        selected.selected().relation(reference).unwrap().target(),
        target
    );
    assert!(matches!(
        selected.selected().resolve(reference),
        Some(LayoutAbiSemanticRecordV1::Layout(record))
            if record == terminal.layouts().get(remote_layout).unwrap()
    ));

    let other = section(
        empty_exports(cone("other-consumer")),
        &dependencies,
        &Source(vec![LayoutAbiDependencyV1::new(remote, target)]),
    )
    .unwrap();
    assert!(other.selected().relation(reference).is_none());
}

#[test]
fn reader_recomputes_selected_semantics_instead_of_trusting_wire() {
    let remote = cone("reader-remote");
    let consumer = cone("reader-consumer");
    let (remote_value, remote_foundation) = empty_struct(remote, "RemoteValue");
    let remote_layout = remote_value.identity().layout();
    let terminal = section(
        exports(&remote_foundation, vec![remote_value.into()]),
        &[],
        &Source::default(),
    )
    .unwrap();
    let target = LayoutAbiSemanticTargetV1::Layout(remote_layout);
    let roots = Source(vec![LayoutAbiDependencyV1::new(remote, target)]);
    let expected = empty_exports(consumer);
    let dependencies = [&terminal];
    let section = section(expected.clone(), &dependencies, &roots).unwrap();
    let bytes = encode(&section).unwrap();
    let decoded: DecodedCrossConeLayoutAbiSectionV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(remote).unwrap();
    pending.register_authority(remote_layout).unwrap();
    let mut identities = pending.finish().unwrap();
    let replayed = decoded
        .validate(
            &expected,
            &dependencies,
            Vec::new(),
            &roots,
            &mut identities,
            &mut meter(),
        )
        .unwrap();
    assert_eq!(encode(&replayed).unwrap(), bytes);

    let decoded: DecodedCrossConeLayoutAbiSectionV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    assert!(matches!(
        decoded.validate(
            &expected,
            &dependencies,
            Vec::new(),
            &Source::default(),
            &mut identities,
            &mut meter(),
        ),
        Err(LayoutAbiSectionError::SelectedClosure)
    ));
}

#[test]
fn closure_rejects_a_forged_embedded_layout_constituent() {
    let remote = cone("forged-remote");
    let consumer = cone("forged-consumer");
    let (remote_value, remote_foundation) = empty_struct(remote, "Payload");
    let terminal = section(
        exports(&remote_foundation, vec![remote_value.clone().into()]),
        &[],
        &Source::default(),
    )
    .unwrap();
    let forged = scalar_with_identity(remote, remote_value.identity().exact_record().clone());
    let (boxed, boxed_foundation) = boxed_with_payload(consumer, &forged);
    let dependencies = [&terminal];
    let result = section(
        exports(&boxed_foundation, vec![boxed.into()]),
        &dependencies,
        &Source::default(),
    );
    assert!(matches!(
        result,
        Err(LayoutAbiSectionError::Semantic(
            LayoutAbiSemanticClosureError::EmbeddedRecord(_)
        ))
    ));
}

#[test]
fn source_roots_must_be_canonical_and_external() {
    let provider = cone("root-provider");
    let (value, foundation) = empty_struct(provider, "Root");
    let target = LayoutAbiSemanticTargetV1::Layout(value.identity().layout());
    let terminal = section(
        exports(&foundation, vec![value.into()]),
        &[],
        &Source::default(),
    )
    .unwrap();
    let consumer = cone("root-consumer");
    let relation = LayoutAbiDependencyV1::new(provider, target);
    let dependencies = [&terminal];
    let duplicate = Source(vec![relation, relation]);
    assert!(matches!(
        section(empty_exports(consumer), &dependencies, &duplicate),
        Err(LayoutAbiSectionError::Semantic(
            LayoutAbiSemanticClosureError::NonCanonicalRoots
        ))
    ));

    let local_target = LayoutAbiSemanticTargetV1::ShapeSupport(
        PersistentTypeId::from_source_declaration(&source(consumer, "Local")).unwrap(),
    );
    assert!(matches!(
        section(
            empty_exports(consumer),
            &dependencies,
            &Source(vec![LayoutAbiDependencyV1::new(consumer, local_target)]),
        ),
        Err(LayoutAbiSectionError::Semantic(
            LayoutAbiSemanticClosureError::CurrentProvider(_)
        ))
    ));
}
