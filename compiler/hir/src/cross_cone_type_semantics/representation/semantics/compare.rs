use super::*;
use scoop_identity::OptionalSignatureType;
use scoop_wire::WireError;

mod public;
pub(super) use public::public_value;

pub(super) fn shape(
    left: &NominalRepresentationShapeV1,
    right: &NominalRepresentationShapeV1,

    path: &WirePath,
) -> Result<bool, WireError> {
    use NominalRepresentationShapeV1 as Shape;

    match (left, right) {
        (
            Shape::Struct {
                fields: left,
                c_layout_policy: left_policy,
            },
            Shape::Struct {
                fields: right,
                c_layout_policy: right_policy,
            },
        ) => {
            if left_policy != right_policy {
                return Ok(false);
            }
            types::fields(
                left.iter()
                    .map(|f| ((f.field(), f.owner()), f.value_type())),
                right
                    .iter()
                    .map(|f| ((f.field(), f.owner()), f.value_type())),
                path,
            )
        }
        (Shape::Enum { variants: left }, Shape::Enum { variants: right }) => {
            if left.len() != right.len() {
                return Ok(false);
            }
            for (index, (left, right)) in left.iter().zip(right).enumerate() {
                let at = path.clone().index(index as u64);

                if left.variant() != right.variant()
                    || left.owner() != right.owner()
                    || left.gc() != right.gc()
                    || !types::fields(
                        left.fields()
                            .iter()
                            .map(|f| ((f.field(), f.variant()), f.value_type())),
                        right
                            .fields()
                            .iter()
                            .map(|f| ((f.field(), f.variant()), f.value_type())),
                        &at,
                    )?
                {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        (
            Shape::Class {
                base: left_base,
                declared_fields: left,
            },
            Shape::Class {
                base: right_base,
                declared_fields: right,
            },
        ) => {
            let base_matches = match (left_base, right_base) {
                (OptionalSignatureType::Absent, OptionalSignatureType::Absent) => true,
                (OptionalSignatureType::Present(left), OptionalSignatureType::Present(right)) => {
                    crate::compare_default_signature_reference_targets(left, right, path)
                        .map(|ordering| ordering.is_eq())?
                }
                _ => false,
            };
            if !base_matches {
                return Ok(false);
            }
            class_fields(left, right, path)
        }
        (
            Shape::Object {
                backing_class: left_backing,
                declared_fields: left,
            },
            Shape::Object {
                backing_class: right_backing,
                declared_fields: right,
            },
        ) => {
            if left_backing != right_backing {
                return Ok(false);
            }
            class_fields(left, right, path)
        }
        (Shape::Interface, Shape::Interface) => Ok(true),
        (
            Shape::Intrinsic {
                representation: left,
            },
            Shape::Intrinsic {
                representation: right,
            },
        ) => Ok(left == right),
        _ => Ok(false),
    }
}
fn class_fields(
    left: &[crate::ClassRepresentationFieldV1],
    right: &[crate::ClassRepresentationFieldV1],

    path: &WirePath,
) -> Result<bool, WireError> {
    types::fields(
        left.iter()
            .map(|f| ((f.field(), f.owner()), f.value_type())),
        right
            .iter()
            .map(|f| ((f.field(), f.owner()), f.value_type())),
        path,
    )
}
