use super::*;
use scoop_identity::RepresentationRole;
use scoop_wire::{decode_canonical, encode};

mod support;
use support::{Fixture, exact};

#[test]
fn equal_physical_nominals_keep_their_distinct_exact_identities() {
    let mut builder = Builder::new();
    let first = builder.strukt("Payload", &[("value", LONG)]);
    let second = builder.strukt("Payload", &[("value", LONG)]);
    let variant = || test_variant("Ready".into(), true, Vec::new());
    let first_enum = builder.enums.alloc(mir::EnumDef {
        name: "State".into(),
        type_arguments: Vec::new(),
        gc_free: true,
        variants: vec![variant()],
    });
    let second_enum = builder.enums.alloc(mir::EnumDef {
        name: "State".into(),
        type_arguments: Vec::new(),
        gc_free: true,
        variants: vec![variant()],
    });
    let fixture = Fixture::new(builder);
    let input = fixture.input.module();
    let output = fixture.output.module();
    let first = &output.structs[struct_def_id(first)];
    let second = &output.structs[struct_def_id(second)];
    assert_eq!(
        (first.name.as_str(), first.size, first.align),
        (second.name.as_str(), second.size, second.align)
    );
    assert_ne!(first.exact_type, second.exact_type);
    for (id, definition) in input.structs.iter() {
        let actual = &output.structs[struct_def_id(id)];
        assert_eq!(
            actual.exact_type,
            exact(input, &definition.physical_type(id))
        );
        assert!(
            output
                .meta
                .exact_types
                .iter()
                .any(|record| record.id() == actual.exact_type)
        );
    }
    let first = &output.enums[enum_def_id(first_enum)];
    let second = &output.enums[enum_def_id(second_enum)];
    assert_eq!(first.name, second.name);
    let lir::EnumRepr::Tagged { size, align, .. } = &first.repr else {
        panic!("the first unit enum has a tagged representation")
    };
    let lir::EnumRepr::Tagged {
        size: second_size,
        align: second_align,
        ..
    } = &second.repr
    else {
        panic!("the second unit enum has a tagged representation")
    };
    assert_eq!((size, align), (second_size, second_align));
    assert_ne!(first.exact_type, second.exact_type);
    for (id, definition) in input.enums.iter() {
        let actual = &output.enums[enum_def_id(id)];
        assert_eq!(
            actual.exact_type,
            exact(
                input,
                &mir::Type::Enum(id, definition.type_arguments.clone())
            )
        );
        assert!(
            output
                .meta
                .exact_types
                .iter()
                .any(|record| record.id() == actual.exact_type)
        );
    }
}

fn find(
    table: &lir::CanonicalExactLayoutExportsV1,
    exact: PersistentExactTypeId,
    role: RepresentationRole,
) -> &lir::ExactLayoutExportV1 {
    table
        .records()
        .iter()
        .find(|record| {
            record.identity().exact() == exact
                && record.identity().layout_key().representation() == role
        })
        .unwrap()
}

#[test]
fn exact_layout_producer_replays_all_physical_roots_and_zst_descriptor() {
    let mut builder = Builder::new();
    let empty = builder.strukt("Empty", &[]);
    let value = builder.strukt(
        "Payload",
        &[
            ("flag", mir::Type::Boolean),
            ("marker", mir::Type::Unit),
            ("value", LONG),
        ],
    );
    let fixture = Fixture::new(builder);
    let table = fixture.replay().unwrap();
    let expected: std::collections::BTreeSet<_> = fixture
        .output
        .module()
        .meta
        .layouts
        .iter()
        .filter(|(_, layout)| {
            fixture
                .types
                .get(layout.identity.layout_record().key().exact_type())
                .is_some()
        })
        .map(|(_, layout)| layout.identity.layout_record().id())
        .chain(
            fixture
                .output
                .module()
                .meta
                .type_descriptors
                .iter()
                .filter(|(_, descriptor)| {
                    fixture
                        .types
                        .get(
                            descriptor
                                .instance_layout
                                .layout_record()
                                .key()
                                .exact_type(),
                        )
                        .is_some()
                })
                .map(|(_, descriptor)| descriptor.instance_layout.layout_record().id()),
        )
        .collect();
    assert_eq!(
        table
            .records()
            .iter()
            .map(|record| record.identity().layout())
            .collect::<std::collections::BTreeSet<_>>(),
        expected
    );
    let empty_exact = exact(fixture.input.module(), &mir::Type::Struct(empty));
    assert_eq!(
        find(&table, empty_exact, RepresentationRole::ManagedValue)
            .value_handle()
            .unwrap()
            .value()
            .storage()
            .byte_size(),
        0
    );
    assert_eq!(
        find(&table, empty_exact, RepresentationRole::ManagedObject)
            .instance_handle()
            .unwrap()
            .shape()
            .minimum_size(),
        16
    );
    let value = find(
        &table,
        exact(fixture.input.module(), &mir::Type::Struct(value)),
        RepresentationRole::ManagedValue,
    )
    .value_handle()
    .unwrap();
    let lir::ExactRepresentationKindV1::Struct(layout) = value.representation().kind() else {
        panic!("struct")
    };
    assert_eq!(
        layout
            .fields()
            .iter()
            .map(|field| field.storage().offset().get())
            .collect::<Vec<_>>(),
        vec![0, 0, 8]
    );
    let bytes = encode(&table).unwrap();
    let raw: lir::DecodedCanonicalExactLayoutExportsV1 = decode_canonical(&bytes).unwrap();
    assert_eq!(raw.validate_against(&table).unwrap(), table);
}

#[test]
fn exact_layout_producer_preserves_class_prefix_and_enum_slots() {
    let mut builder = Builder::new();
    let base = builder.class(
        "Base",
        None,
        &[("byte", mir::Type::Boolean)],
        vec![],
        vec![],
    );
    let child = builder.class(
        "Child",
        Some(base),
        &[
            ("byte", mir::Type::Boolean),
            ("empty", mir::Type::Unit),
            ("reference", mir::Type::Class(base)),
        ],
        vec![],
        vec![],
    );
    let enumeration = builder.enums.alloc(mir::EnumDef {
        name: "Choice".into(),
        type_arguments: vec![],
        gc_free: false,
        variants: vec![
            test_variant(
                "Value".into(),
                true,
                vec![mir::Field {
                    name: "_1".into(),
                    ty: LONG,
                }],
            ),
            test_variant(
                "Reference".into(),
                false,
                vec![
                    mir::Field {
                        name: "_1".into(),
                        ty: mir::Type::Class(child),
                    },
                    mir::Field {
                        name: "_2".into(),
                        ty: mir::Type::Unit,
                    },
                ],
            ),
        ],
    });
    let fixture = Fixture::new(builder);
    let table = fixture.replay().unwrap();
    let child = find(
        &table,
        exact(fixture.input.module(), &mir::Type::Class(child)),
        RepresentationRole::ManagedObject,
    )
    .instance_handle()
    .unwrap();
    assert_eq!(child.shape().minimum_size(), 32);
    assert_eq!(
        child.shape().object_scan(),
        &lir::RefScan::References(vec![24])
    );
    let enumeration = find(
        &table,
        exact(
            fixture.input.module(),
            &mir::Type::Enum(enumeration, vec![]),
        ),
        RepresentationRole::ManagedValue,
    )
    .value_handle()
    .unwrap();
    let lir::ExactRepresentationKindV1::TaggedEnum(layout) = enumeration.representation().kind()
    else {
        panic!("tagged enum")
    };
    assert_eq!(layout.geometry().storage().size(), 24);
    assert_eq!(
        layout.variants()[1].fields()[0].storage().offset().get(),
        16
    );
    assert_eq!(layout.variants()[1].fields()[1].storage().offset().get(), 0);
}

#[test]
fn exact_layout_producer_rejects_mir_field_mismatch_and_different_physical_output() {
    let mut builder = Builder::new();
    let id = builder.strukt("Payload", &[("value", LONG)]);
    let mut fixture = Fixture::new(builder);
    let ty = mir::Type::Struct(id);
    let source = fixture
        .types
        .get(exact(fixture.input.module(), &ty))
        .unwrap();
    let mut representation = source.representation().clone();
    let facts = source.facts();
    let mir::MirTypeRepresentationV1::Struct { fields, .. } = &mut representation else {
        panic!("struct")
    };
    fields[0].value = exact(fixture.input.module(), &mir::Type::Boolean);
    fixture.replace(&ty, representation, facts);
    assert!(matches!(
        fixture.replay(),
        Err(ExactLayoutLoweringError::SourceFields(_))
    ));

    let mut builder = Builder::new();
    builder.strukt("Payload", &[("value", LONG)]);
    let expected = Fixture::new(builder);
    let mut builder = Builder::new();
    builder.strukt("Payload", &[("value", mir::Type::Boolean)]);
    let altered = Fixture::new(builder);
    assert!(matches!(
        lower_exact_layout_exports(
            &expected.input,
            &altered.output,
            &expected.types,
            &expected.graph,
            &[]
        ),
        Err(ExactLayoutLoweringError::PhysicalLayout(_))
            | Err(ExactLayoutLoweringError::PhysicalDescriptor(_))
    ));
}

#[test]
fn exact_layout_producer_replays_c_layout_without_a_native_boundary() {
    let mut builder = Builder::new();
    let id = builder.c_strukt(
        "Packed",
        mir::MirCLayoutValue::A16,
        mir::MirCLayoutValue::A1,
        false,
        &[("flag", mir::Type::Boolean), ("value", LONG)],
    );
    let fixture = Fixture::new(builder);
    assert!(fixture.input.module().extern_functions.is_empty());
    let table = fixture.replay().unwrap();
    let exact = exact(fixture.input.module(), &mir::Type::Struct(id));
    for role in [RepresentationRole::CValue, RepresentationRole::ManagedValue] {
        let value = find(&table, exact, role).value_handle().unwrap();
        let lir::ExactRepresentationKindV1::Struct(layout) = value.representation().kind() else {
            panic!("struct")
        };
        assert_eq!(value.value().storage().byte_size(), 16);
        assert_eq!(value.value().storage().alignment().get(), 16);
        assert_eq!(layout.fields()[1].storage().offset().get(), 1);
        assert_eq!(layout.fields()[1].access_alignment().get(), 1);
        assert!(matches!(
            layout.policy(),
            lir::StructLayoutPolicyV1::CLayout(_)
        ));
    }
    assert_eq!(fixture.output.foundation().c_abi_layouts().len(), 1);
}
