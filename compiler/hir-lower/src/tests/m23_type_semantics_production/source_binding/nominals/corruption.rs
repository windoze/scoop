use super::*;
use hir::{EnumSourceShapeV1, EnumSourceVariantV1, NominalSourceShapeV1 as Shape};

#[test]
fn nominal_binding_requires_exact_roots_and_complete_nonpublic_member_lists() {
    with_source(DECLARATIONS, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let table = sources(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        let missing = Table::try_new(table.records()[1..].to_vec(), &mut meter()).unwrap();
        assert!(matches!(
            foundation.bind_nominal_sources(&missing, &mut meter()),
            Err(Error::Inventory("nominal owners"))
        ));
        let source = named(&fixture, &table, "Defaults");
        assert_eq!(source.constructors().values().len(), 3);
        let missing = rebuild(
            source,
            hir::CanonicalPersistentIdsV1::try_new(source.constructors().values()[1..].to_vec())
                .unwrap(),
            source.members().clone(),
            source.children().clone(),
            source.source_shape().clone(),
        );
        assert!(matches!(
            foundation.bind_nominal_sources(&replace(&table, missing), &mut meter()),
            Err(Error::Inventory("nominal constructors"))
        ));
        let source = named(&fixture, &table, "Marker");
        assert_eq!(source.members().values().len(), 2);
        let missing = rebuild(
            source,
            source.constructors().clone(),
            hir::CanonicalNestedMemberRefsV1::try_new(source.members().values()[1..].to_vec())
                .unwrap(),
            source.children().clone(),
            source.source_shape().clone(),
        );
        assert!(matches!(
            foundation.bind_nominal_sources(&replace(&table, missing), &mut meter()),
            Err(Error::Inventory("nominal members"))
        ));
    });
    with_source(NESTED, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let table = sources(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        let source = named(&fixture, &table, "Envelope");
        let missing = rebuild(
            source,
            source.constructors().clone(),
            source.members().clone(),
            hir::CanonicalNestedNominalRefsV1::default(),
            source.source_shape().clone(),
        );
        assert!(matches!(
            foundation.bind_nominal_sources(&replace(&table, missing), &mut meter()),
            Err(Error::Inventory("nominal children"))
        ));
    });
}

#[test]
fn nominal_binding_rejects_omitted_fields_variants_and_variant_fields() {
    with_source(DECLARATIONS, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let table = sources(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        let pair = named(&fixture, &table, "Pair");
        let Shape::Struct(shape) = pair.source_shape() else {
            panic!("struct")
        };
        let missing = Shape::Struct(
            hir::StructSourceShapeV1::try_new(
                shape.fields()[1..].to_vec(),
                hir::NominalCLayoutPolicyV1::Ordinary,
            )
            .unwrap(),
        );
        let record = rebuild(
            pair,
            pair.constructors().clone(),
            pair.members().clone(),
            pair.children().clone(),
            missing,
        );
        assert!(matches!(
            foundation.bind_nominal_sources(&replace(&table, record), &mut meter()),
            Err(Error::Inventory("struct fields"))
        ));
        let choice = named(&fixture, &table, "Choice");
        let Shape::Enum(shape) = choice.source_shape() else {
            panic!("enum")
        };
        let missing_variant =
            Shape::Enum(EnumSourceShapeV1::try_new(shape.variants()[1..].to_vec()).unwrap());
        let mut variants = shape.variants().to_vec();
        let variant = &variants[1];
        variants[1] =
            EnumSourceVariantV1::try_new(variant.variant(), variant.style(), vec![]).unwrap();
        let missing_field = Shape::Enum(EnumSourceShapeV1::try_new(variants).unwrap());
        for (shape, expected) in [
            (missing_variant, "enum variants"),
            (missing_field, "enum variant fields"),
        ] {
            let record = rebuild(
                choice,
                choice.constructors().clone(),
                choice.members().clone(),
                choice.children().clone(),
                shape,
            );
            assert!(
                matches!(foundation.bind_nominal_sources(&replace(&table, record), &mut meter()), Err(Error::Inventory(actual)) if actual == expected)
            );
        }
    });
}

#[test]
fn nominal_binding_rejects_invalid_selectors_and_out_of_scope_field_types() {
    with_source(DECLARATIONS, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let table = sources(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        let choice = named(&fixture, &table, "Choice");
        let Shape::Enum(shape) = choice.source_shape() else {
            panic!("enum")
        };
        for bad_type in [false, true] {
            let mut variants = shape.variants().to_vec();
            let variant = &variants[1];
            let mut fields = variant.fields().to_vec();
            let style = if bad_type {
                fields[0] = hir::EnumSourceFieldV1::new(
                    fields[0].field(),
                    scoop_identity::SignatureTypeKey::Binder { depth: 1, index: 0 },
                );
                variant.style()
            } else {
                hir::EnumSourceVariantStyleV1::Named
            };
            variants[1] = EnumSourceVariantV1::try_new(variant.variant(), style, fields).unwrap();
            let record = rebuild(
                choice,
                choice.constructors().clone(),
                choice.members().clone(),
                choice.children().clone(),
                Shape::Enum(EnumSourceShapeV1::try_new(variants).unwrap()),
            );
            assert!(matches!(
                foundation.bind_nominal_sources(&replace(&table, record), &mut meter()),
                Err(Error::Contract { .. })
            ));
        }
    });
}

#[test]
fn nominal_binding_requires_artifact_owned_keys_even_when_graph_resolves_them() {
    with_source(DECLARATIONS, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let table = sources(output, &mut fixture);
        for field in [1, 2, 3, 4, 5, 6] {
            let mut canonical = fixture.foundation.as_canonical().clone();
            match field {
                1 => canonical.set_fields(vec![]).unwrap(),
                2 => canonical.set_enum_variants(vec![]).unwrap(),
                3 => canonical.set_enum_variant_fields(vec![]).unwrap(),
                4 => canonical.set_object_values(vec![]).unwrap(),
                5 => canonical.set_functions(vec![]).unwrap(),
                6 => canonical.set_constructors(vec![]).unwrap(),
                _ => unreachable!(),
            }
            let incomplete = hir::OdrFreeHirFoundation::try_new(canonical).unwrap();
            let foundation = fixture
                .source
                .bind_to_foundation(&incomplete, &fixture.identities, &mut meter())
                .unwrap();
            let result = foundation.bind_nominal_sources(&table, &mut meter());
            assert!(
                matches!(result, Err(Error::Inventory(_) | Error::MissingObject(_))),
                "field {field}: {result:?}"
            );
        }
    });
}
