use super::*;

pub(in super::super) fn public_value(
    actual: &NominalRepresentationShapeV1,
    expected: &NominalSourceShapeV1,

    path: &WirePath,
) -> Result<bool, WireError> {
    match (actual, expected) {
        (
            NominalRepresentationShapeV1::Class {
                declared_fields, ..
            },
            NominalSourceShapeV1::Class(_),
        )
        | (
            NominalRepresentationShapeV1::Object {
                declared_fields, ..
            },
            NominalSourceShapeV1::Object(_),
        ) => types::fields(
            declared_fields
                .iter()
                .map(|field| (field.field(), field.value_type())),
            expected
                .declared_fields()
                .iter()
                .map(|field| (field.field(), field.value_type())),
            path,
        ),
        (NominalRepresentationShapeV1::Interface, NominalSourceShapeV1::Interface) => Ok(true),
        (
            NominalRepresentationShapeV1::Intrinsic { representation },
            NominalSourceShapeV1::Intrinsic(source),
        ) => Ok(representation == source),
        (
            NominalRepresentationShapeV1::Struct {
                fields,
                c_layout_policy,
            },
            NominalSourceShapeV1::Struct(source),
        ) => {
            if *c_layout_policy != source.c_layout_policy() {
                return Ok(false);
            }
            types::fields(
                fields.iter().map(|f| (f.field(), f.value_type())),
                source.fields().iter().map(|f| (f.field(), f.value_type())),
                path,
            )
        }
        (NominalRepresentationShapeV1::Enum { variants }, NominalSourceShapeV1::Enum(source)) => {
            if variants.len() != source.variants().len() {
                return Ok(false);
            }
            for (index, (actual, expected)) in variants.iter().zip(source.variants()).enumerate() {
                let at = path.clone().index(index as u64);

                if actual.variant() != expected.variant()
                    || !types::fields(
                        actual.fields().iter().map(|f| (f.field(), f.value_type())),
                        expected
                            .fields()
                            .iter()
                            .map(|f| (f.field(), f.value_type())),
                        &at,
                    )?
                {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}
