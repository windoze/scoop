use super::*;
use crate::call_resolution::candidates::{
    NominalConstructorSource, ValueParameterCalling, VarargOmission,
};
use crate::defaults::{DefaultArgumentSource, DefaultExprTemplateRef};
use hir::ImportedCallableSource;

impl Lowerer {
    pub(in crate::class::inheritance) fn current_coding_omissions(
        &mut self,
        source: NominalConstructorSource,
        span: Span,
    ) -> Option<Vec<bool>> {
        let parameters = self
            .nominal_constructor_view(source, span)
            .signature
            .value_parameters;
        parameters
            .into_iter()
            .map(|parameter| {
                if self.as_option(parameter.ty).is_none() {
                    return Some(false);
                }
                match parameter.calling {
                    ValueParameterCalling::Required => Some(true),
                    ValueParameterCalling::Default(source)
                    | ValueParameterCalling::Vararg {
                        omission: VarargOmission::Default(source),
                        ..
                    } => self.coding_default_is_none(source, span),
                    ValueParameterCalling::Vararg {
                        omission: VarargOmission::EmptyArray,
                        ..
                    } => Some(false),
                }
            })
            .collect()
    }

    pub(in crate::class::inheritance) fn dependency_coding_omissions(
        &self,
        origin: scoop_identity::CallableTemplateOrigin,
    ) -> Vec<bool> {
        let declaration = self
            .dependencies
            .as_ref()
            .expect("a dependency has a catalog")
            .callable_declaration(origin)
            .expect("the constructor is declared");
        declaration
            .source_interface()
            .expect("a constructor retains its parameters")
            .parameters()
            .parameters()
            .iter()
            .map(|parameter| {
                if !self.signature_is_core_option(parameter.value_type()) {
                    return false;
                }
                match parameter.calling() {
                    hir::CallableParameterCallingV1::Required => true,
                    hir::CallableParameterCallingV1::Default { template }
                    | hir::CallableParameterCallingV1::VarargDefault { template, .. } => self
                        .imported_coding_default_is_none(
                            declaration
                                .default_template(*template)
                                .expect("a default parameter retains its body"),
                        ),
                    hir::CallableParameterCallingV1::VarargEmpty { .. } => false,
                }
            })
            .collect()
    }

    fn coding_default_is_none(
        &mut self,
        source: DefaultArgumentSource,
        span: Span,
    ) -> Option<bool> {
        let source = self.resolve_default_argument(source, span)?;
        let expression = match source {
            DefaultExprTemplateRef::Local(local) => {
                &self.local_default_exprs[local].body.expression
            }
            DefaultExprTemplateRef::Export(source) => match &self.export_default_sources[source] {
                hir::ExportDefaultSource::Declared { expression, .. } => {
                    &self.export_default_exprs[*expression].expression
                }
                hir::ExportDefaultSource::Imported { template, .. } => {
                    return Some(self.imported_coding_default_is_none(template));
                }
            },
        };
        Some(
            expression.body.statements.is_empty()
                && self.as_option(expression.value.ty).is_some()
                && match &expression.value.kind {
                    hir::ExprKind::NoneLiteral => true,
                    hir::ExprKind::VariantConstruct { args, .. } => args.is_empty(),
                    _ => false,
                },
        )
    }

    fn imported_coding_default_is_none(&self, template: &hir::ExportDefaultTemplateV1) -> bool {
        let body = template.body();
        body.statements().is_empty()
            && self.signature_is_core_option(body.value().result_type())
            && match body.value().kind() {
                hir::DefaultExpressionKindV1::NoneLiteral => true,
                hir::DefaultExpressionKindV1::VariantConstruct { arguments, .. } => {
                    arguments.is_empty()
                }
                _ => false,
            }
    }
}
