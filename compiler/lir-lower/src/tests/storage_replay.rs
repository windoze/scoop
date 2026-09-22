use super::*;

fn context() -> LoweringContext {
    LoweringContext::new(lir::LirTargetProfile::DARWIN_AARCH64)
}

fn finish(mut builder: Builder) -> mir::Module {
    let main = builder.main(Arena::new(), Vec::new());
    builder.finish(main)
}

#[test]
fn producer_ordinary_fields_and_c_packing_use_the_shared_checked_cursor() {
    let mut builder = Builder::new();
    let packed = builder.c_strukt(
        "Packed",
        mir::MirCLayoutValue::A16,
        mir::MirCLayoutValue::A1,
        false,
        &[("byte", mir::Type::Boolean), ("long", LONG)],
    );
    let module = finish(builder);
    let context = context();
    let enums = lower_enums(&context, &module).unwrap();
    let enum_shape = |id| Ok(repr_shape(&context, &enums[enum_def_id(id)].repr));
    assert_eq!(
        aggregate_shape(
            &context,
            &module,
            &enum_shape,
            &[mir::Type::Boolean, LONG, mir::Type::Unit, LONG]
        )
        .unwrap(),
        (vec![0, 8, 0, 16], 24, 8),
    );
    let (fields, size, alignment) =
        struct_shape(&context, &module, &enum_shape, &module.structs[packed]).unwrap();
    assert_eq!((size, alignment), (16, 16));
    assert_eq!(
        fields
            .iter()
            .map(|field| (field.offset, field.access_align))
            .collect::<Vec<_>>(),
        vec![(0, 1), (1, 1)]
    );
}

#[test]
fn producer_preserves_base_tail_padding_and_elides_zst_fields_at_every_depth() {
    let mut builder = Builder::new();
    let base_fields = [("long", LONG), ("flag", mir::Type::Boolean)];
    let base = builder.class("Base", None, &base_fields, Vec::new(), Vec::new());
    let mut child_fields = base_fields.to_vec();
    child_fields.extend([("empty", mir::Type::Unit), ("own", mir::Type::Boolean)]);
    let child = builder.class("Child", Some(base), &child_fields, Vec::new(), Vec::new());
    let mut leaf_fields = child_fields;
    leaf_fields.push(("reference", mir::Type::String));
    let leaf = builder.class("Leaf", Some(child), &leaf_fields, Vec::new(), Vec::new());
    let module = finish(builder);
    let context = context();
    let enums = lower_enums(&context, &module).unwrap();
    assert_eq!(
        class_shape(&context, &module, &enums, &module.classes[base]).unwrap(),
        (vec![16, 24], 32, 8)
    );
    assert_eq!(
        class_shape(&context, &module, &enums, &module.classes[child]).unwrap(),
        (vec![16, 24, 0, 32], 40, 8)
    );
    assert_eq!(
        class_shape(&context, &module, &enums, &module.classes[leaf]).unwrap(),
        (vec![16, 24, 0, 32, 40], 48, 8)
    );
    assert_eq!(
        class_layout(&context, &module, &enums, &module.classes[leaf]).unwrap(),
        (48, 8, lir::RefScan::References(vec![40]))
    );
}

#[test]
fn producer_rejects_changed_base_prefix_and_inheritance_cycles() {
    let mut builder = Builder::new();
    let base = builder.class("Base", None, &[("base", LONG)], Vec::new(), Vec::new());
    let child = builder.class(
        "Child",
        Some(base),
        &[("changed", LONG)],
        Vec::new(),
        Vec::new(),
    );
    let mut module = finish(builder);
    let context = context();
    let enums = lower_enums(&context, &module).unwrap();
    assert_eq!(
        class_shape(&context, &module, &enums, &module.classes[child]),
        Err(StorageLoweringError::InvalidRepresentation(
            "class fields do not preserve the complete base prefix"
        ))
    );
    let mir::ClassRepresentation::Declared { base_class, .. } =
        &mut module.classes[base].representation
    else {
        panic!("declared test base")
    };
    *base_class = Some(child);
    assert_eq!(
        class_order(&module),
        Err(StorageLoweringError::InvalidRepresentation(
            "class base cycle"
        ))
    );
    assert_eq!(
        class_shape(&context, &module, &enums, &module.classes[child]),
        Err(StorageLoweringError::InvalidRepresentation(
            "class base cycle"
        ))
    );
}

#[test]
fn tagged_enum_shared_region_retains_its_size_and_zst_payload_never_removes_tag() {
    let mut builder = Builder::new();
    let aligned = builder.c_strukt(
        "Aligned",
        mir::MirCLayoutValue::A16,
        mir::MirCLayoutValue::Natural,
        false,
        &[("long", LONG)],
    );
    let triple = builder.strukt("Triple", &[("a", LONG), ("b", LONG), ("c", LONG)]);
    let enum_id = builder.enums.alloc(mir::EnumDef {
        name: "Mixed".to_string(),
        type_arguments: Vec::new(),
        gc_free: false,
        variants: vec![
            test_variant(
                "Aligned".to_string(),
                true,
                vec![mir::Field {
                    name: "value".to_string(),
                    ty: mir::Type::Struct(aligned),
                }],
            ),
            test_variant(
                "Triple".to_string(),
                true,
                vec![mir::Field {
                    name: "value".to_string(),
                    ty: mir::Type::Struct(triple),
                }],
            ),
            test_variant(
                "Reference".to_string(),
                false,
                vec![mir::Field {
                    name: "value".to_string(),
                    ty: mir::Type::String,
                }],
            ),
        ],
    });
    let zst = builder.option_enum("Option<Unit>", mir::Type::Unit);
    let module = finish(builder);
    let enums = lower_enums(&context(), &module).unwrap();
    let lir::EnumRepr::Tagged {
        variants,
        size,
        align,
    } = &enums[enum_def_id(enum_id)].repr
    else {
        panic!("tagged enum")
    };
    assert_eq!((*size, *align), (48, 16));
    assert_eq!(
        variants
            .iter()
            .map(|variant| variant.slot_offset)
            .collect::<Vec<_>>(),
        vec![16, 16, 40]
    );
    assert_eq!(
        enums[enum_def_id(enum_id)].scan,
        lir::RefScan::References(vec![40])
    );
    let lir::EnumRepr::Tagged {
        variants,
        size,
        align,
    } = &enums[enum_def_id(zst)].repr
    else {
        panic!("ZST payload keeps the discriminant")
    };
    assert_eq!((*size, *align), (8, 8));
    assert_eq!(variants[0].fields[0].offset, 0);
}

#[test]
fn aggregate_and_scan_overflow_return_errors_to_the_lowering_boundary() {
    let mut builder = Builder::new();
    let id = builder.option_enum("Option<Long>", LONG);
    let module = finish(builder);
    let context = context();
    let oversized = aggregate_shape(
        &context,
        &module,
        &|_| Ok((i64::MAX as u64, 1)),
        &[mir::Type::Enum(id, Vec::new()), mir::Type::Boolean],
    );
    assert!(matches!(
        oversized,
        Err(StorageLoweringError::Shape(
            lir::TypeInstanceShapeError::ManagedObjectTooLarge { .. }
        ))
    ));
    let enums = lower_enums(&context, &module).unwrap();
    let error = scan_fields(
        &context,
        &module,
        &enums,
        &[mir::Type::String],
        &[8],
        u64::MAX,
    )
    .unwrap_err();
    assert_eq!(
        error,
        StorageLoweringError::Scan(lir::RefScanValidationError::OffsetOverflow)
    );
    let error: StrongLirLoweringError = error.into();
    assert!(matches!(error, StrongLirLoweringError::StorageReplay(_)));
    assert!(std::error::Error::source(&error).is_some());
}

#[test]
fn empty_c_layout_and_zero_sized_c_fields_are_rejected_before_lir_emission() {
    for fields in [Vec::new(), vec![("unit", mir::Type::Unit)]] {
        let mut builder = Builder::new();
        builder.c_strukt(
            "InvalidC",
            mir::MirCLayoutValue::Natural,
            mir::MirCLayoutValue::Natural,
            false,
            &fields,
        );
        let module = finish(builder);
        let enums = lower_enums(&context(), &module).unwrap();
        let error = lower_structs(&context(), &module, &enums).unwrap_err();
        if fields.is_empty() {
            assert_eq!(
                error,
                StorageLoweringError::Replay(lir::StorageReplayError::EmptyCLayout)
            );
        } else {
            assert_eq!(
                error,
                StorageLoweringError::Shape(lir::TypeInstanceShapeError::ZeroInlineSize)
            );
        }
    }
}

#[test]
fn zero_sized_producer_storage_cannot_discard_an_inconsistent_reference_scan() {
    assert!(matches!(
        value_storage(&context(), 0, 8, lir::RefScan::References(vec![0])),
        Err(StorageLoweringError::Scan(
            lir::RefScanValidationError::OutOfBounds { .. }
        )),
    ));
    let empty = value_storage(&context(), 0, 16, lir::RefScan::None).unwrap();
    assert!(matches!(
        empty.kind(),
        lir::ValueStorageKindV1::ZeroSized { .. }
    ));
}
