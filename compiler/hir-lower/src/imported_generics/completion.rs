//! Complete imported bodies and the scalar conformance inputs they expose.

use super::*;

impl Lowerer {
    pub(crate) fn complete_imported_generic_bodies(&mut self) {
        let mut index = 0;
        let mut constructor = 0;
        loop {
            if index == self.imported_generic_templates.templates.len()
                && constructor == self.imported_constructor_templates.templates.len()
            {
                if let Err(error) = self.prepare_imported_encoding_inputs() {
                    self.error(
                        scoop_ast::Span::new(0, 0),
                        error.diagnostic("container element encoding"),
                    );
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
