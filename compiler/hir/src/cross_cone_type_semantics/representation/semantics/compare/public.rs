use super::*;

pub(in super::super) fn public_value(
    actual: &NominalRepresentationShapeV1,
    expected: &NominalSourceShapeV1,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<bool, WireError> {
    meter.check_semantic_depth(2, path)?;
    meter.charge_nodes(1, path)?;
    meter.charge_work(1, path)?;
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
            4,
            meter,
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
            meter.charge_work(3, path)?;
            if *c_layout_policy != source.c_layout_policy() {
                return Ok(false);
            }
            types::fields(
                fields.iter().map(|f| (f.field(), f.value_type())),
                source.fields().iter().map(|f| (f.field(), f.value_type())),
                4,
                meter,
                path,
            )
        }
        (NominalRepresentationShapeV1::Enum { variants }, NominalSourceShapeV1::Enum(source)) => {
            source::sequence(variants.len(), meter, path)?;
            source::sequence(source.variants().len(), meter, path)?;
            if variants.len() != source.variants().len() {
                return Ok(false);
            }
            for (index, (actual, expected)) in variants.iter().zip(source.variants()).enumerate() {
                let at = path.clone().index(index as u64);
                meter.check_semantic_depth(3, &at)?;
                meter.charge_work(32, &at)?;
                if actual.variant() != expected.variant()
                    || !types::fields(
                        actual.fields().iter().map(|f| (f.field(), f.value_type())),
                        expected
                            .fields()
                            .iter()
                            .map(|f| (f.field(), f.value_type())),
                        5,
                        meter,
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
