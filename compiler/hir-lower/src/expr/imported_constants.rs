//! Winner-only selection and local literal materialization for dependency
//! constants. The selected record retains the import route while the HIR body
//! contains only an ordinary literal in the consumer's local id domain.

use scoop_ast::Span;
use scoop_hir as hir;

use super::imported_origins::{ImportedDefinitionOriginError, ImportedDefinitionOriginError::*};
use crate::Lowerer;

pub(crate) struct ImportedDependencyConstantValue {
    pub(crate) value: hir::ConstPropertyValue,
    pub(crate) ty: hir::TypeId,
    pub(crate) span: Span,
    pub(crate) origin: hir::ExpressionOrigin,
}

impl Lowerer {
    pub(crate) fn select_imported_dependency_constant(
        &mut self,
        binding: &hir::DirectImportedTargetBinding,
        usage_span: Span,
    ) -> Option<ImportedDependencyConstantValue> {
        let candidate = match self
            .dependencies
            .as_ref()
            .expect("ordinary lowering carries a dependency selection plan")
            .constant_candidate(binding)
        {
            Ok(candidate) => candidate,
            Err(error) => {
                self.error(
                    usage_span,
                    format!("invalid imported dependency constant: {error}"),
                );
                return None;
            }
        };
        if candidate.exact_type().is_none() {
            self.error(
                usage_span,
                "dependency constant is outside the M23-5 core-closed exact subset".to_string(),
            );
            return None;
        }
        let ty = match self.imported_signature_type(candidate.record().value_type()) {
            Ok(ty) => ty,
            Err(error) => {
                self.error(
                    usage_span,
                    format!("dependency constant has an unsupported signature type: {error:?}"),
                );
                return None;
            }
        };
        let value = hir::ConstPropertyValue::from(candidate.record().value().clone());
        let source = candidate.record().definition_origin().clone();
        let imported = match candidate.definition_source(&source) {
            Some(imported) => imported,
            None => {
                self.report_imported_constant_origin_error(
                    usage_span,
                    MissingSource {
                        context: source.origin().context(),
                    },
                );
                return None;
            }
        };
        let definition = match self.import_dependency_definition_origin(&source, imported) {
            Ok(definition) => definition,
            Err(error) => {
                self.report_imported_constant_origin_error(usage_span, error);
                return None;
            }
        };
        if let Err(error) = self
            .dependencies
            .as_mut()
            .expect("ordinary lowering carries a dependency selection plan")
            .select_constant(candidate)
        {
            self.error(
                usage_span,
                format!("failed to select imported dependency constant: {error}"),
            );
            return None;
        }
        Some(ImportedDependencyConstantValue {
            value,
            ty,
            span: definition.span,
            origin: hir::ExpressionOrigin::Instantiated(hir::ConcreteExpressionOrigin {
                definition,
                evaluation: self.definition_origin(usage_span).into(),
            }),
        })
    }

    pub(crate) fn lower_imported_dependency_constant(
        &mut self,
        binding: &hir::DirectImportedTargetBinding,
        usage_span: Span,
    ) -> Option<hir::Expr> {
        let constant = self.select_imported_dependency_constant(binding, usage_span)?;
        let kind = match constant.value {
            hir::ConstPropertyValue::Integer(value) => hir::ExprKind::IntegerLiteral(value),
            hir::ConstPropertyValue::Float(value) => hir::ExprKind::FloatLiteral(value),
            hir::ConstPropertyValue::Char(value) => hir::ExprKind::CharLiteral(value),
            hir::ConstPropertyValue::Boolean(value) => hir::ExprKind::BoolLiteral(value),
            hir::ConstPropertyValue::String(value) => hir::ExprKind::StringLiteral {
                value,
                owner: hir::StringConstantOwner::CurrentDefinition,
            },
        };
        Some(hir::Expr {
            kind,
            ty: constant.ty,
            span: constant.span,
            origin: constant.origin,
        })
    }

    fn report_imported_constant_origin_error(
        &mut self,
        usage_span: Span,
        error: ImportedDefinitionOriginError,
    ) {
        self.error(
            usage_span,
            format!("failed to import dependency constant origin: {error}"),
        );
    }
}
