use super::*;
use hir::{
    DefaultOperationEntityShapeV1 as Shape, DefaultSourceNominalOperationError as Error,
    DefaultSourceNominalOperationV1 as Target,
};
use scoop_identity::{NonEmptyVec, SignatureTypeKey as Type};
use scoop_wire::WirePath;
mod generics;
mod rejection;

mod support;
use support::*;

const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-default-nominal-operations/standalone.scoop"
));
const COMBINED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-default-nominal-operations/combined.scoop"
));

#[test]
fn nominal_operation_shapes_preserve_real_declaration_order_and_immutable_fields() {
    with_source(SOURCE, |output, nominals| {
        let mut dump = Vec::new();
        for name in ["packet", "empty"] {
            let ty = value_type(output, name, 0);
            let Shape::Aggregate(shape) = query(nominals, Target::Struct(&ty)) else {
                panic!("struct aggregate");
            };
            assert_eq!(shape.owner_type(), &ty);
            dump.push(format!("{name}: Struct fields={}", shape.fields().len()));
            let source = source(nominals, &ty);
            let hir::NominalSourceShapeV1::Struct(source_shape) = source.source_shape() else {
                panic!("source struct");
            };
            assert_eq!(
                shape.fields(),
                source_shape
                    .fields()
                    .iter()
                    .map(|f| f.value_type().clone())
                    .collect::<Vec<_>>()
            );
            for (index, field) in source_shape.fields().iter().enumerate() {
                let Shape::Field(shape) = query(
                    nominals,
                    Target::StructField {
                        declaration: field.field(),
                        owner_type: &ty,
                    },
                ) else {
                    panic!("struct field");
                };
                assert_eq!(shape.kind(), hir::DefaultFieldOperationKindV1::Struct);
                assert_eq!(shape.owner_type(), &ty);
                assert_eq!(shape.value_type(), &shape_type(nominals, &ty, index));
                assert_eq!(shape.declaration_index(), index as u32);
                assert_eq!(shape.mutable(), hir::CanonicalBooleanV1::False);
                dump.push(format!(
                    "{name}.field{index}: index={} mutable={:?}",
                    shape.declaration_index(),
                    shape.mutable()
                ));
            }
        }
        let ty = value_type(output, "choice", 0);
        let hir::NominalSourceShapeV1::Enum(shape) = source(nominals, &ty).source_shape() else {
            panic!("source enum");
        };
        for (index, variant) in shape.variants().iter().enumerate() {
            let reference = hir::DefaultEnumVariantRefV1::new(variant.variant(), ty.clone());
            let Shape::Aggregate(shape) = query(nominals, Target::Variant(&reference)) else {
                panic!("variant aggregate");
            };
            assert_eq!(shape.owner_type(), &ty);
            assert_eq!(
                shape.fields(),
                variant
                    .fields()
                    .iter()
                    .map(|f| f.value_type().clone())
                    .collect::<Vec<_>>()
            );
            dump.push(format!(
                "choice.variant{index}: fields={}",
                shape.fields().len()
            ));
            for (position, field) in variant.fields().iter().enumerate() {
                let reference = hir::DefaultEnumVariantFieldRefV1::new(field.field(), ty.clone());
                let Shape::VariantField(shape) = query(nominals, Target::VariantField(&reference))
                else {
                    panic!("variant field");
                };
                assert_eq!(shape.owner_type(), &ty);
                assert_eq!(shape.value_type(), field.value_type());
                assert_eq!(shape.declaration_index(), position as u32);
                dump.push(format!(
                    "choice.variant{index}.field{position}: index={}",
                    shape.declaration_index()
                ));
            }
        }
        for (name, position, kind) in [
            ("choice", 0, hir::PublicNominalKindV1::Enum),
            ("box", 0, hir::PublicNominalKindV1::Class),
            ("marker", 1, hir::PublicNominalKindV1::Interface),
        ] {
            let ty = value_type(output, name, position);
            assert_eq!(
                query(
                    nominals,
                    Target::Type {
                        owner_type: &ty,
                        expected: kind
                    }
                ),
                Shape::Type(ty)
            );
            dump.push(format!("{name}: {kind:?}"));
        }
        let ty = value_type(output, "cache", 0);
        let hir::NominalSourceShapeV1::Object(shape) = source(nominals, &ty).source_shape() else {
            panic!("source object");
        };
        assert_eq!(
            query(nominals, Target::Singleton(shape.value())),
            Shape::Type(ty)
        );
        dump.push("cache: Object".to_owned());
        dump.sort();
        assert_eq!(
            dump.join("\n") + "\n",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m23-default-nominal-operations/standalone.snap"
            ))
        );
    });
}
