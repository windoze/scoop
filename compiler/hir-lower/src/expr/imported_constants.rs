//! Winner-only selection and local literal materialization for dependency
//! constants. The selected record retains the import route while the HIR body
//! contains only an ordinary literal in the consumer's local id domain.

use scoop_ast::Span;
use scoop_hir as hir;

use super::imported_origins::{ImportedDefinitionOriginError, ImportedDefinitionOriginError::*};
use crate::Lowerer;

mod values;

pub(crate) struct ImportedDependencyConstantValue {
    pub(crate) value: hir::ConstPropertyValue,
    pub(crate) ty: hir::TypeId,
    pub(crate) span: Span,
    pub(crate) origin: hir::ExpressionOrigin,
}

impl Lowerer {
    pub(crate) fn lower_imported_const_reference(
        &mut self,
        access: &scoop_ast::FieldAccess,
        expected: Option<hir::TypeId>,
    ) -> Result<Option<(hir::ConstPropertyValue, hir::TypeId)>, ()> {
        let Some(owner) = self.resolve_imported_nominal_qualifier(&access.receiver)? else {
            return Ok(None);
        };
        let expression = self
            .lower_imported_qualified_field(owner, access, expected)
            .ok_or(())?;
        let Some(value) = self.expression_constant(&expression) else {
            self.error(
                access.span,
                "const initializer may only reference constant values".into(),
            );
            return Err(());
        };
        Ok(Some((value, expression.ty)))
    }

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
        let Some(value) = self.imported_constant_image(candidate.record().value().clone(), ty)
        else {
            self.error(
                usage_span,
                "dependency constant payload does not match its declared type".into(),
            );
            return None;
        };
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
        let hir::ImportedTarget::Property(property) = binding.target() else {
            unreachable!("a selected dependency constant is a logical property")
        };
        self.observe_constant_reference(
            usage_span,
            crate::editing::constants::ConstantTarget::Bound(hir::SemanticProperty::Imported(
                hir::PropertyDeclarationId::Property(property.persistent()),
            )),
        );
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
        Some(self.constant_expression(
            constant.value,
            constant.ty,
            hir::StringConstantOwner::CurrentDefinition,
            constant.span,
            constant.origin,
        ))
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
