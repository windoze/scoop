use super::*;
use scoop_identity::CallableTemplateOrigin;

impl DeclarationAnalysis {
    pub fn function_source_text(&self, function: hir::FunctionId) -> Option<&str> {
        let file = self.lowerer.function_definition_file(function)?;
        Some(&self.lowerer.intrinsic_sources[file].source)
    }
}

impl Lowerer {
    pub(super) fn editing_callable_origin(
        &self,
        callable: hir::SemanticCallable,
    ) -> Option<hir::SemanticSourceLocation> {
        use hir::{DefaultCallableDeclarationV1 as D, SemanticCallable as C};
        match callable {
            C::Current(function) => {
                let span = self
                    .source_function_declarations
                    .get(&function)?
                    .name_span?;
                let file = self.function_definition_file(function)?;
                Some(hir::SemanticSourceLocation {
                    source: self.source_identity(file).clone(),
                    span,
                })
            }
            C::Imported(declaration) => {
                let origin = match declaration {
                    D::Function(function) => CallableTemplateOrigin::Function(function),
                    D::GenericFunction(function) => {
                        CallableTemplateOrigin::GenericFunction(function)
                    }
                    _ => return None,
                };
                let declaration = self
                    .dependencies
                    .as_ref()?
                    .callable_declaration(origin)
                    .ok()?;
                crate::semantic_queries::imported_location(declaration.definition_origin().origin())
                    .ok()
            }
            C::CurrentGetter(_) | C::CurrentSetter(_) => None,
        }
    }
}
