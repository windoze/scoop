use super::*;

pub(super) fn matches(
    left: &NominalSourceShapeV1,
    right: &NominalSourceShapeV1,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<bool, WireError> {
    // Both are source metadata, including generic binders and variant styles.
    for shape in [left, right] {
        sequence(shape.declared_fields().len(), meter, path)?;
        for field in shape.declared_fields() {
            signature(field.value_type(), field.value_type(), meter, path)?;
        }
        match shape {
            NominalSourceShapeV1::Enum(value) => {
                sequence(value.variants().len(), meter, path)?;
                for variant in value.variants() {
                    sequence(variant.fields().len(), meter, path)?;
                    for field in variant.fields() {
                        signature(field.value_type(), field.value_type(), meter, path)?;
                    }
                }
            }
            NominalSourceShapeV1::Struct(_)
            | NominalSourceShapeV1::Class(_)
            | NominalSourceShapeV1::Interface
            | NominalSourceShapeV1::Object(_)
            | NominalSourceShapeV1::Intrinsic(_) => meter.charge_work(1, path)?,
        }
    }
    Ok(left == right)
}
