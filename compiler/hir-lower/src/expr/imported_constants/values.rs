//! Match an imported payload to its exact type at the materialization boundary.

use crate::Lowerer;
use scoop_hir as hir;

impl Lowerer {
    pub(super) fn imported_constant_image(
        &mut self,
        value: hir::CanonicalValueConstantV1,
        ty: hir::TypeId,
    ) -> Option<hir::HirConstantImage> {
        use hir::{
            CanonicalConstValueV1 as S, CanonicalValueConstantV1 as V, HirConstantImage as H,
            Type as T,
        };
        Some(match (value, self.types[ty].clone()) {
            (V::Scalar(S::Integer(value)), T::Integer(kind)) if value.kind() == kind => {
                H::Integer(value.into())
            }
            (V::Scalar(S::Boolean(value)), T::Boolean) => H::Boolean(value.into()),
            (V::Scalar(S::String(value)), T::String) => H::String(value),
            (V::Scalar(S::Char(value)), _) if self.is_char_type(ty) => H::Char(value.into()),
            (V::Scalar(S::Float(value)), _) if self.float_kind(ty) == Some(value.kind()) => {
                H::Float(value)
            }
            (V::Unit, T::Unit) => H::Unit,
            (V::Tuple(elements), T::Tuple(types)) => {
                H::Tuple(self.imported_constant_fields(elements, types)?)
            }
            (V::Struct(fields), T::Struct(application)) => {
                let template = self.struct_applications[application].template;
                if !matches!(
                    self.struct_definition(template).representation,
                    hir::StructRepresentation::Declared(_)
                ) {
                    return None;
                }
                let types = self
                    .struct_fields(ty)?
                    .fields
                    .into_iter()
                    .map(|field| field.ty)
                    .collect();
                H::Struct {
                    application,
                    fields: self.imported_constant_fields(fields, types)?,
                }
            }
            (V::Enum { variant, fields }, T::Enum(owner)) => {
                let count = self
                    .enum_definition(self.enum_applications[owner].template)
                    .variants
                    .len();
                let variant = (0..count)
                    .map(|index| self.enum_variant_at(owner, index as u32))
                    .find(|candidate| candidate.variant == variant)?;
                let types = self
                    .enum_variant(variant)
                    .fields
                    .into_iter()
                    .map(|(_, ty)| ty)
                    .collect();
                H::Enum {
                    variant,
                    fields: self.imported_constant_fields(fields, types)?,
                }
            }
            _ => return None,
        })
    }

    fn imported_constant_fields(
        &mut self,
        fields: Vec<hir::CanonicalValueConstantV1>,
        types: Vec<hir::TypeId>,
    ) -> Option<Vec<hir::HirConstantImage>> {
        if fields.len() != types.len() {
            return None;
        }
        fields
            .into_iter()
            .zip(types)
            .map(|(value, ty)| self.imported_constant_image(value, ty))
            .collect()
    }
}
