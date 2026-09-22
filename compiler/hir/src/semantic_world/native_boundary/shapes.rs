use super::*;
use crate::{
    NativeBoundaryFieldDefinition, NativeBoundaryVariantDefinition,
    NativeBoundaryVariantFieldDefinition, NominalSourceShapeV1,
};

pub(super) fn project(
    provider: &ImportedProvider<'_>,
    source: &NominalSourceShapeV1,
) -> Result<Shape, Error> {
    let foundation = provider.foundation().canonical_for_semantic_authority();
    match source {
        NominalSourceShapeV1::Struct(source) => {
            let fields = source
                .fields()
                .iter()
                .map(|field| {
                    let (_, key) = foundation.field_by_bytes(field.field().as_array()).ok_or(
                        Error::MissingDependencyField {
                            field: field.field(),
                        },
                    )?;
                    NativeBoundaryFieldDefinition::new(key, field.value_type().clone())
                        .map_err(Error::InvalidDefinition)
                })
                .collect::<Result<_, _>>()?;
            Ok(Shape::Struct {
                c_layout: source.c_layout_policy().into(),
                fields,
            })
        }
        NominalSourceShapeV1::Enum(source) => {
            let variants = source
                .variants()
                .iter()
                .map(|variant| {
                    let (_, key) = foundation
                        .enum_variant_by_bytes(variant.variant().as_array())
                        .ok_or(Error::MissingDependencyVariant {
                            variant: variant.variant(),
                        })?;
                    let fields = variant
                        .fields()
                        .iter()
                        .map(|field| {
                            let (_, key) = foundation
                                .enum_variant_field_by_bytes(field.field().as_array())
                                .ok_or(Error::MissingDependencyVariantField {
                                    field: field.field(),
                                })?;
                            NativeBoundaryVariantFieldDefinition::new(
                                key,
                                field.value_type().clone(),
                            )
                            .map_err(Error::InvalidDefinition)
                        })
                        .collect::<Result<_, _>>()?;
                    NativeBoundaryVariantDefinition::new(key, fields)
                        .map_err(Error::InvalidDefinition)
                })
                .collect::<Result<_, _>>()?;
            Ok(Shape::Enum { variants })
        }
        NominalSourceShapeV1::Class
        | NominalSourceShapeV1::Interface
        | NominalSourceShapeV1::Object(_) => Ok(Shape::Reference),
    }
}
