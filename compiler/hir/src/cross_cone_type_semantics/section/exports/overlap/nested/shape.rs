use super::*;

pub(super) fn matches(
    left: &NominalSourceShapeV1,
    right: &NominalSourceShapeV1,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<bool, WireError> {
    // Both are source metadata, including generic binders and variant styles.
    for shape in [left, right] {
        match shape {
            NominalSourceShapeV1::Struct(value) => {
                sequence(value.fields().len(), meter, path)?;
                for field in value.fields() {
                    signature(field.value_type(), field.value_type(), meter, path)?;
                }
            }
            NominalSourceShapeV1::Enum(value) => {
                sequence(value.variants().len(), meter, path)?;
                for variant in value.variants() {
                    sequence(variant.fields().len(), meter, path)?;
                    for field in variant.fields() {
                        signature(field.value_type(), field.value_type(), meter, path)?;
                    }
                }
            }
            NominalSourceShapeV1::Class
            | NominalSourceShapeV1::Interface
            | NominalSourceShapeV1::Object(_) => {}
        }
    }
    Ok(left == right)
}
