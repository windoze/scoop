use super::*;
use scoop_identity::ValidatedIdentityGraph;

pub(in crate::foundation::wire::validation) fn validate_graph_shape_coverage(
    graph: &ValidatedIdentityGraph,
    definitions: &[NativeBoundaryTypeDefinitionRecord],
) -> Result<(), HirFoundationValidationError> {
    if definitions.is_empty() {
        return Ok(());
    }
    let fields = graph
        .closure_records::<PersistentFieldId, FieldIdentityKey>(&WirePath::root().field(12))
        .map_err(HirFoundationValidationError::Identity)?;
    let variants = graph
        .closure_records::<PersistentEnumVariantId, EnumVariantIdentityKey>(
            &WirePath::root().field(13),
        )
        .map_err(HirFoundationValidationError::Identity)?;
    let variant_fields = graph
        .closure_records::<PersistentEnumVariantFieldId, EnumVariantFieldKey>(
            &WirePath::root().field(14),
        )
        .map_err(HirFoundationValidationError::Identity)?;
    validate_shape_coverage(&fields, &variants, &variant_fields, definitions)
}
