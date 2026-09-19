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
            NominalRepresentationShapeV1::Struct { fields, .. },
            NominalSourceShapeV1::Struct(source),
        ) => types::fields(
            fields.iter().map(|f| (f.field(), f.value_type())),
            source.fields().iter().map(|f| (f.field(), f.value_type())),
            4,
            meter,
            path,
        ),
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
