use super::*;
use crate::types::ResolvedTypeName;
use hir::{CanonicalAnnotationValueV1, CanonicalIntegerConstantV1, IntegerKind};

impl Lowerer {
    pub(super) fn nested_annotation_value(
        &mut self,
        value: &ast::AnnotationLiteral,
        expected: PersistentAnnotationId,
        span: ast::Span,
    ) -> Option<CanonicalAnnotationValueV1> {
        let ast::AnnotationLiteral::Application(application) = value else {
            self.error(
                span,
                "annotation argument requires a nested annotation application".into(),
            );
            return None;
        };
        let path = application.name.segments().cloned().collect::<Vec<_>>();
        let Some(ResolvedTypeName::Annotation(id)) =
            self.resolve_type_name_path(&path, &[]).ok()?
        else {
            self.error(
                application.name.span,
                format!("`{}` is not an annotation class", application.name),
            );
            return None;
        };
        if id != expected {
            self.error(
                span,
                "nested annotation argument has a different annotation type".into(),
            );
            return None;
        }
        let parameters = self.annotation_parameters(id, span)?;
        let arguments = self.bind_annotation_arguments(application, &parameters)?;
        Some(CanonicalAnnotationValueV1::Annotation {
            annotation: id,
            arguments,
        })
    }

    pub(super) fn integral_annotation_value(
        &mut self,
        value: &ast::AnnotationLiteral,
        expected: hir::TypeId,
        span: ast::Span,
    ) -> Option<CanonicalAnnotationValueV1> {
        let ast::AnnotationLiteral::ConstReference(source) = value else {
            self.error(
                span,
                "integral enum annotation argument requires a variant of the declared enum type"
                    .into(),
            );
            return None;
        };
        let mut statements = Vec::new();
        let value = self.lower_expr(source, &mut statements, Some(expected))?;
        if !statements.is_empty() || self.integral_annotation_constant(&value).is_none() {
            self.error(
                span,
                "integral enum annotation argument must be a static variant".into(),
            );
            return None;
        }
        if !self.types_equal(value.ty, expected) {
            self.error(
                span,
                format!(
                    "annotation argument must be {}, found {}",
                    self.type_name(expected),
                    self.type_name(value.ty)
                ),
            );
            return None;
        }
        let Some((variant, constant)) = self.integral_annotation_constant(&value) else {
            self.error(
                span,
                "integral enum annotation argument must be a static variant".into(),
            );
            return None;
        };
        Some(CanonicalAnnotationValueV1::IntegralEnum {
            value_type: self.annotation_signature_type(expected, span)?,
            variant,
            value: constant,
        })
    }

    pub(super) fn integral_annotation_constant(
        &self,
        value: &hir::Expr,
    ) -> Option<(
        scoop_identity::PersistentEnumVariantId,
        CanonicalIntegerConstantV1,
    )> {
        let hir::ExprKind::VariantConstruct { variant, .. } = value.kind else {
            return None;
        };
        let definition = self.enum_definition(self.enum_applications[variant.owner].template);
        let hir::EnumRepresentation::Integral { kind, values } = &definition.representation else {
            return None;
        };
        let raw = values[self.enum_variant_index(variant) as usize];
        let value = match *kind {
            IntegerKind::SIGNED_8 => CanonicalIntegerConstantV1::Signed8(raw as u8),
            IntegerKind::SIGNED_16 => CanonicalIntegerConstantV1::Signed16(raw as u16),
            IntegerKind::SIGNED_32 => CanonicalIntegerConstantV1::Signed32(raw as u32),
            IntegerKind::SIGNED_64 => CanonicalIntegerConstantV1::Signed64(raw),
            IntegerKind::UNSIGNED_8 => CanonicalIntegerConstantV1::Unsigned8(raw as u8),
            IntegerKind::UNSIGNED_16 => CanonicalIntegerConstantV1::Unsigned16(raw as u16),
            IntegerKind::UNSIGNED_32 => CanonicalIntegerConstantV1::Unsigned32(raw as u32),
            IntegerKind::UNSIGNED_64 => CanonicalIntegerConstantV1::Unsigned64(raw),
        };
        Some((variant.variant, value))
    }
}
