use super::*;
use crate::types::ArrayKind;

impl Lowerer {
    pub(super) fn is_annotation_parameter_type(&self, ty: hir::TypeId) -> bool {
        self.annotation_scalar_kind(ty).is_some() || self.annotation_array_element(ty).is_some()
    }

    fn annotation_array_element(&self, ty: hir::TypeId) -> Option<hir::TypeId> {
        let array = self.array_type_info(ty)?;
        (array.kind == ArrayKind::Immutable && self.annotation_scalar_kind(array.element).is_some())
            .then_some(array.element)
    }

    pub(super) fn annotation_value(
        &mut self,
        value: &ast::AnnotationLiteral,
        ty: hir::TypeId,
        span: ast::Span,
    ) -> Option<hir::CanonicalAnnotationValueV1> {
        let Some(element) = self.annotation_array_element(ty) else {
            return self
                .annotation_constant(value, ty, span)
                .map(hir::CanonicalAnnotationValueV1::Scalar);
        };
        let ast::AnnotationLiteral::Array(source) = value else {
            self.error(
                span,
                format!(
                    "annotation argument for {} must use an array literal",
                    self.type_name(ty)
                ),
            );
            return None;
        };
        let elements = source
            .iter()
            .map(|source| self.annotation_constant(&source.value, element, source.span))
            .collect::<Option<Vec<_>>>()?;
        Some(hir::CanonicalAnnotationValueV1::Array {
            element_type: self
                .annotation_scalar_kind(element)
                .expect("annotation array elements have a scalar type"),
            elements,
        })
    }
}
