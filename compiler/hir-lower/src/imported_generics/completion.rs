//! Complete imported function and constructor bodies.

use super::*;

impl Lowerer {
    pub(crate) fn complete_imported_generic_bodies(&mut self) {
        let mut index = 0;
        let mut constructor = 0;
        loop {
            if index == self.imported_generic_templates.templates.len()
                && constructor == self.imported_constructor_templates.templates.len()
            {
                if !self.diagnostics.is_empty() {
                    break;
                }
                if let Err(error) = self.retain_generic_primitive_declarations() {
                    self.diagnostics.push(scoop_ast::Diagnostic::without_span(
                        scoop_ast::DiagnosticSeverity::Error,
                        self.primary_output_file(),
                        error.diagnostic("generic primitive argument"),
                    ));
                    break;
                }
                if index == self.imported_generic_templates.templates.len()
                    && constructor == self.imported_constructor_templates.templates.len()
                {
                    break;
                }
            }
            if index == self.imported_generic_templates.templates.len() {
                self.complete_imported_constructor(constructor);
                constructor += 1;
                continue;
            }
            let Some(template) = self.imported_generic_templates.templates[index].clone() else {
                assert!(
                    !self.diagnostics.is_empty(),
                    "a failed dependency signature must have a diagnostic"
                );
                index += 1;
                continue;
            };
            let bodyless = match &template.source {
                PreparedImportedCallableSource::Declaration(declaration) => {
                    declaration.interface().modality() == hir::CallableModalityV1::Abstract
                        || matches!(
                            declaration.interface().effects().implementation(),
                            hir::CallableImplementationV1::Intrinsic(_)
                        )
                }
                PreparedImportedCallableSource::InitializationEnsure => true,
                PreparedImportedCallableSource::Body(_) => false,
            };
            if bodyless || template.statements.is_some() {
                index += 1;
                continue;
            }
            match self.materialize_imported_callable_body(&template) {
                Ok(body) => {
                    let target = self.imported_generic_templates.templates[index]
                        .as_mut()
                        .expect("body completion follows signature completion");
                    target.locals = body.locals;
                    target.statements = Some(body.statements);
                }
                Err(error) => {
                    self.current_file = template.origin.file as usize;
                    self.error(
                        template.span,
                        format!(
                            "cannot instantiate dependency body `{}`: {error}",
                            template.name
                        ),
                    );
                }
            }
            index += 1;
        }
    }
}
