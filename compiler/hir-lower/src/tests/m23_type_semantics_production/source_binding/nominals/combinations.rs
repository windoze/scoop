use super::*;
use hir::{
    EnumSourceShapeV1, EnumSourceVariantV1, NestedSourceMemberRefV1, NominalSourceShapeV1 as Shape,
};

const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-nominals/binding.scoop"
));

#[test]
fn nominal_binding_keeps_generic_members_and_rejects_another_owners_method() {
    with_source(SOURCE, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let table = sources(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        foundation
            .bind_nominal_sources(&table, &mut meter())
            .unwrap();
        let host = named(&fixture, &table, "Host");
        let other = named(&fixture, &table, "Other");
        assert_eq!(host.members().values().len(), 4);
        assert_eq!(host.children().values().len(), 2);
        let generic = |record: &Record| {
            record
                .members()
                .values()
                .iter()
                .copied()
                .find(|member| matches!(member, NestedSourceMemberRefV1::GenericFunction(_)))
                .unwrap()
        };
        let own = generic(host);
        let foreign = generic(other);
        assert_ne!(own, foreign);
        let members = hir::CanonicalNestedMemberRefsV1::try_new(
            host.members()
                .values()
                .iter()
                .map(|member| if *member == own { foreign } else { *member })
                .collect(),
        )
        .unwrap();
        let wrong_owner = rebuild(
            host,
            host.constructors().clone(),
            members,
            host.children().clone(),
            host.source_shape().clone(),
        );
        assert!(matches!(
            foundation.bind_nominal_sources(&replace(&table, wrong_owner), &mut meter()),
            Err(Error::Inventory("nominal members"))
        ));
        let mut canonical = fixture.foundation.as_canonical().clone();
        canonical.set_generic_functions(vec![]).unwrap();
        let incomplete = hir::OdrFreeHirFoundation::try_new(canonical).unwrap();
        let foundation = fixture
            .source
            .bind_to_foundation(&incomplete, &fixture.identities, &mut meter())
            .unwrap();
        assert!(matches!(
            foundation.bind_nominal_sources(&table, &mut meter()),
            Err(Error::Inventory("nominal members"))
        ));
    });
}

#[test]
fn nominal_binding_rejects_reordered_positional_fields_and_other_variant_fields() {
    with_source(SOURCE, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let table = sources(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        let choice = named(&fixture, &table, "Choice");
        let Shape::Enum(shape) = choice.source_shape() else {
            panic!("enum")
        };
        for foreign_variant in [false, true] {
            let mut variants = shape.variants().to_vec();
            let variant = &variants[1];
            assert_eq!(variant.fields().len(), 2);
            let mut fields = variant.fields().to_vec();
            if foreign_variant {
                fields[0] = variants[2].fields()[0].clone();
            } else {
                fields.reverse();
            }
            variants[1] =
                EnumSourceVariantV1::try_new(variant.variant(), variant.style(), fields).unwrap();
            let record = rebuild(
                choice,
                choice.constructors().clone(),
                choice.members().clone(),
                choice.children().clone(),
                Shape::Enum(EnumSourceShapeV1::try_new(variants).unwrap()),
            );
            let modified = replace(&table, record);
            let error = foundation
                .bind_nominal_sources(&modified, &mut meter())
                .unwrap_err();
            assert!(matches!(
                (foreign_variant, error),
                (false, Error::Contract { .. }) | (true, Error::Inventory("enum variant fields"))
            ));
        }
    });
}
