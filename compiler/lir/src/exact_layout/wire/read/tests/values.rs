use super::*;

#[test]
fn value_reader_checks_scalar_kind_field_identity_order_and_storage_scan() {
    for kind in IntegerKind::ALL {
        let expected = ExactLayoutExportV1::from(integer(kind.canonical_name(), kind));
        roundtrip(&expected);
        reject(&expected, |raw| {
            *value(raw) = RawValue::Scalar(ScalarRepresentationKindV1::Boolean)
        });
    }
    let expected = fixtures::aggregate();
    roundtrip(&expected);
    reject(&expected, |raw| {
        let RawValue::Struct {
            interior_mutable, ..
        } = value(raw)
        else {
            panic!("struct")
        };
        *interior_mutable = !*interior_mutable;
    });
    reject(&expected, |raw| {
        let RawValue::Struct { fields, .. } = value(raw) else {
            panic!("struct")
        };
        fields.swap(0, 1);
    });
    reject(&expected, |raw| {
        let RawValue::Struct { fields, .. } = value(raw) else {
            panic!("struct")
        };
        fields[1].id = fields[0].id;
    });
    reject(&expected, |raw| {
        let RawValue::Struct { fields, .. } = value(raw) else {
            panic!("struct")
        };
        fields[1].alignment = 1;
    });
    reject(&expected, |raw| {
        let RawValue::Struct { fields, .. } = value(raw) else {
            panic!("struct")
        };
        fields.pop();
    });
    reject(&expected, |raw| {
        let RawBody::Value { storage, .. } = &mut raw.semantic.body else {
            panic!("value")
        };
        let wrong = ValueStorageLayoutV1::inline(8, 8, RefScan::None).unwrap();
        *storage = decode_canonical(&encode(&wrong).unwrap()).unwrap();
    });
    reject(&expected, |raw| {
        let RawValue::Struct { fields, .. } = value(raw) else {
            panic!("struct")
        };
        // A ZST cannot acquire a stored layout by copying a nonzero field.
        fields[0].storage = decode_canonical(&encode(&fields[1].storage).unwrap()).unwrap();
    });
}

#[test]
fn tuple_reader_requires_exact_positional_order() {
    let expected = fixtures::tuple();
    roundtrip(&expected);
    reject(&expected, |raw| {
        let RawValue::Tuple(elements) = value(raw) else {
            panic!("tuple")
        };
        elements.swap(0, 1);
    });
}

#[test]
fn tagged_enum_reader_checks_all_regions_slots_and_absolute_fields() {
    let expected = fixtures::enumeration(false);
    roundtrip(&expected);
    reject(&expected, |raw| {
        let RawValue::TaggedEnum { tag, .. } = value(raw) else {
            panic!("tagged")
        };
        tag.size = 1;
    });
    reject(&expected, |raw| {
        let RawValue::TaggedEnum { pure, .. } = value(raw) else {
            panic!("tagged")
        };
        pure.size += 8;
    });
    reject(&expected, |raw| {
        let RawValue::TaggedEnum { variants, .. } = value(raw) else {
            panic!("tagged")
        };
        let RawSlot::Dedicated(region) = &mut variants[1].slot else {
            panic!("dedicated")
        };
        region.offset = u64::MAX;
    });
    reject(&expected, |raw| {
        let RawValue::TaggedEnum { variants, .. } = value(raw) else {
            panic!("tagged")
        };
        let RawSlot::Shared { size, .. } = &mut variants[0].slot else {
            panic!("shared")
        };
        *size = 0;
    });
    reject(&expected, |raw| {
        let RawValue::TaggedEnum { variants, .. } = value(raw) else {
            panic!("tagged")
        };
        variants[1].variant.fields[0].alignment = 1;
    });
    reject(&expected, |raw| {
        let RawValue::TaggedEnum { variants, .. } = value(raw) else {
            panic!("tagged")
        };
        variants.swap(0, 1);
    });
}

#[test]
fn niche_enum_reader_requires_the_replayed_pointer_family_and_payload_variant() {
    let expected = fixtures::enumeration(true);
    roundtrip(&expected);
    reject(&expected, |raw| {
        let RawValue::NicheEnum { pointer, .. } = value(raw) else {
            panic!("niche")
        };
        *pointer = NullNicheKind::Raw;
    });
    reject(&expected, |raw| {
        let RawValue::NicheEnum {
            payload, variants, ..
        } = value(raw)
        else {
            panic!("niche")
        };
        *payload = variants[0].id;
    });
}

struct StoredAt {
    exact: PersistentExactTypeId,
    layout: PersistentLayoutId,
    offset: u64,
}
impl WireEncode for StoredAt {
    fn encode(&self, encoder: &mut Encoder) -> EncodeResult {
        encoder.map(4)?;
        encoder.field(0)?;
        encoder.unsigned(2)?;
        encoder.field(1)?;
        self.exact.encode(encoder)?;
        encoder.field(2)?;
        encoder.unsigned(self.offset)?;
        encoder.field(3)?;
        self.layout.encode(encoder)
    }
}

#[test]
fn stored_field_offset_cannot_overflow_or_change_from_enum_absolute_to_slot_relative() {
    let expected = fixtures::enumeration(false);
    let ExactLayoutBodyKindV1::Value(layout) = expected.kind() else {
        panic!("value")
    };
    let ExactRepresentationKindV1::TaggedEnum(layout) = layout.representation().kind() else {
        panic!("tagged")
    };
    let FieldStorageKindV1::Stored { exact, layout, .. } =
        layout.variants()[1].fields()[0].storage().kind()
    else {
        panic!("stored")
    };
    for offset in [0, u64::MAX] {
        reject(&expected, |raw| {
            let RawValue::TaggedEnum { variants, .. } = value(raw) else {
                panic!("tagged")
            };
            let wire = StoredAt {
                exact,
                layout: layout.layout(),
                offset,
            };
            variants[1].variant.fields[0].storage =
                decode_canonical(&encode(&wire).unwrap()).unwrap();
        });
    }
}
