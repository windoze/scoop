use super::*;
use scoop_identity::{PendingIdentityValidation, PersistentTypeId};
use scoop_wire::{decode_canonical, encode};

mod fixture;
mod layout_replay;
mod resolved_dependencies;
use fixture::*;

#[test]
fn empty_section_roundtrips_as_one_complete_six_field_product() {
    let exports = empty_exports(cone("empty"));
    let expected = exports.clone();
    let source = [];
    let section = CrossConeLayoutAbiSectionV1::try_new(exports, &[], Vec::new(), &source).unwrap();
    assert!(section.selected().is_empty());

    let bytes = encode(&section).unwrap();
    assert_eq!(bytes[0], 0xa6);
    let decoded: DecodedCrossConeLayoutAbiSectionV1 = decode_canonical(&bytes).unwrap();
    let mut identities = PendingIdentityValidation::new().finish().unwrap();
    let replayed = decoded
        .validate_layouts(expected.layouts())
        .unwrap()
        .validate_callables(expected.callables())
        .unwrap()
        .validate_dispatch(expected.dispatch())
        .unwrap()
        .validate_descriptors(expected.descriptors())
        .unwrap()
        .validate_shape_support(expected.shape_support(), expected.direct_callables())
        .unwrap()
        .resolve_dependencies(&mut identities)
        .unwrap();
    assert_eq!(replayed.exports(), &expected);
    assert!(replayed.selected_relations().is_empty());
}

#[test]
fn embedded_field_storage_does_not_require_a_layout_symbol() {
    let remote = cone("remote");
    let consumer = cone("consumer");
    let (remote_value, remote_foundation) = empty_struct(remote, "RemoteValue");
    let remote_layout = remote_value.identity().layout();
    let terminal = section(
        exports(&remote_foundation, vec![remote_value.into()]),
        &[],
        &[],
    )
    .unwrap();

    let (local_value, local_foundation) = struct_with_field(
        consumer,
        "LocalValue",
        "remote",
        terminal.layouts().get(remote_layout).unwrap(),
    );
    let dependencies = [terminal.exports()];
    let selected = section(
        exports(&local_foundation, vec![local_value.into()]),
        &dependencies,
        &[],
    )
    .unwrap();
    assert!(selected.selected().is_empty());
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
        &[],
    )
    .unwrap();
    let target = LayoutAbiSemanticTargetV1::Layout(remote_layout);
    let roots = vec![LayoutAbiDependencyV1::new(remote, target)];
    let expected = empty_exports(consumer);
    let dependencies = [terminal.exports()];
    let section = section(expected.clone(), &dependencies, &roots).unwrap();
    let bytes = encode(&section).unwrap();
    let decoded: DecodedCrossConeLayoutAbiSectionV1 = decode_canonical(&bytes).unwrap();
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(remote).unwrap();
    pending.register_authority(remote_layout).unwrap();
    let mut identities = pending.finish().unwrap();
    let replayed = decoded
        .validate_layouts(expected.layouts())
        .unwrap()
        .validate_callables(expected.callables())
        .unwrap()
        .validate_dispatch(expected.dispatch())
        .unwrap()
        .validate_descriptors(expected.descriptors())
        .unwrap()
        .validate_shape_support(expected.shape_support(), expected.direct_callables())
        .unwrap()
        .resolve_dependencies(&mut identities)
        .unwrap();
    replayed
        .replay_dependency_closure(&dependencies, &roots)
        .unwrap();
    assert_eq!(replayed.selected_relations(), roots);
    assert!(matches!(
        replayed.replay_dependency_closure(&dependencies, &[]),
        Err(LayoutAbiSectionError::SelectedClosure)
    ));
}

#[test]
fn source_roots_must_be_canonical_and_external() {
    let provider = cone("root-provider");
    let (value, foundation) = empty_struct(provider, "Root");
    let target = LayoutAbiSemanticTargetV1::Layout(value.identity().layout());
    let terminal = section(exports(&foundation, vec![value.into()]), &[], &[]).unwrap();
    let consumer = cone("root-consumer");
    let relation = LayoutAbiDependencyV1::new(provider, target);
    let dependencies = [terminal.exports()];
    let duplicate = vec![relation, relation];
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
            &[LayoutAbiDependencyV1::new(consumer, local_target)],
        ),
        Err(LayoutAbiSectionError::Semantic(
            LayoutAbiSemanticClosureError::CurrentProvider(_)
        ))
    ));
}
