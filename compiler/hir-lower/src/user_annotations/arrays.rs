use super::*;
use crate::types::ArrayKind;
use hir::{AnnotationElementTypeV1, AnnotationParameterType, CanonicalAnnotationValueV1};

impl Lowerer {
    pub(super) fn is_annotation_parameter_type(&self, ty: hir::TypeId) -> bool {
        self.annotation_scalar_kind(ty).is_some()
            || self.integral_enum_kind(ty).is_some()
            || self.annotation_array_element(ty).is_some()
    }

    fn annotation_array_element(&self, ty: hir::TypeId) -> Option<hir::TypeId> {
        let array = self.array_type_info(ty)?;
        (array.kind == ArrayKind::Immutable
            && (self.annotation_scalar_kind(array.element).is_some()
                || self.integral_enum_kind(array.element).is_some()))
        .then_some(array.element)
    }

    pub(super) fn annotation_value(
        &mut self,
        value: &ast::AnnotationLiteral,
        ty: AnnotationParameterType<hir::TypeId>,
        span: ast::Span,
    ) -> Option<CanonicalAnnotationValueV1> {
        let (element, element_type) = match ty {
            AnnotationParameterType::Annotation(id) => {
                return self.nested_annotation_value(value, id, span);
            }
            AnnotationParameterType::AnnotationArray { element, .. } => (
                AnnotationParameterType::Annotation(element),
                AnnotationElementTypeV1::Annotation(element),
            ),
            AnnotationParameterType::Value(ty) => {
                let Some(element) = self.annotation_array_element(ty) else {
                    if self.integral_enum_kind(ty).is_some() {
                        return self.integral_annotation_value(value, ty, span);
                    }
                    return self
                        .annotation_constant(value, ty, span)
                        .map(CanonicalAnnotationValueV1::Scalar);
                };
                let kind = match self.annotation_scalar_kind(element) {
                    Some(kind) => AnnotationElementTypeV1::Scalar(kind),
                    None => AnnotationElementTypeV1::IntegralEnum(
                        self.annotation_signature_type(element, span)?,
                    ),
                };
                (AnnotationParameterType::Value(element), kind)
            }
        };
        let ast::AnnotationLiteral::Array(source) = value else {
            self.error(
                span,
                "annotation array argument must use an array literal".into(),
            );
            return None;
        };
        let elements = source
            .iter()
            .map(|source| {
                Some(hir::AnnotationElementV1 {
                    value: self.annotation_value(&source.value, element, source.span)?,
                    source: self.intrinsic_sources[self.current_file].identity.clone(),
                    span: scoop_identity::SourceSpan::new(
                        source.span.start.into(),
                        source.span.end.into(),
                    )
                    .expect("parser spans are ordered"),
                })
            })
            .collect::<Option<_>>()?;
        Some(CanonicalAnnotationValueV1::Array {
            element_type,
            elements,
        })
    }
}
