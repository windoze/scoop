use super::*;
use hir::NominalSourceShapeSemanticAuthority as Shape;
use std::borrow::Cow;

fn same_key<T: Clone>(actual: Cow<'_, T>, expected: &T) {
    let Cow::Borrowed(actual) = actual else {
        panic!("artifact source keys must be borrowed");
    };
    assert!(std::ptr::eq(actual, expected));
}

#[test]
fn shape_queries_borrow_the_artifact_keys_for_all_four_identity_roles() {
    with_source(DECLARATIONS, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let table = sources(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        let mut bound = foundation
            .bind_nominal_sources(&table, &mut meter())
            .unwrap();
        let mut roles = [0; 4];
        for source in table.records() {
            match source.source_shape() {
                hir::NominalSourceShapeV1::Struct(shape) => {
                    for field in shape.fields() {
                        let key = bound.struct_field_key(field.field()).unwrap();
                        same_key(
                            Shape::struct_field_key(&mut bound, field.field()).unwrap(),
                            key,
                        );
                        roles[0] += 1;
                    }
                }
                hir::NominalSourceShapeV1::Enum(shape) => {
                    for variant in shape.variants() {
                        let key = bound.enum_variant_key(variant.variant()).unwrap();
                        same_key(
                            Shape::enum_variant_key(&mut bound, variant.variant()).unwrap(),
                            key,
                        );
                        roles[1] += 1;
                        for field in variant.fields() {
                            let key = bound.enum_variant_field_key(field.field()).unwrap();
                            same_key(
                                Shape::enum_variant_field_key(&mut bound, field.field()).unwrap(),
                                key,
                            );
                            roles[2] += 1;
                        }
                    }
                }
                hir::NominalSourceShapeV1::Object(shape) => {
                    let key = bound.object_value_key(shape.value()).unwrap();
                    same_key(
                        Shape::object_value_key(&mut bound, shape.value()).unwrap(),
                        key,
                    );
                    roles[3] += 1;
                }
                hir::NominalSourceShapeV1::Class
                | hir::NominalSourceShapeV1::Interface
                | hir::NominalSourceShapeV1::Intrinsic(_) => continue,
            }
        }
        assert!(roles.into_iter().all(|count| count > 0));
    });
}
