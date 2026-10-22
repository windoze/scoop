use super::*;
use hir::{EditingValue as V, ImportedCallableSource};

impl DeclarationAnalysis {
    pub fn argument_definition(
        &mut self,
        function: hir::FunctionId,
        span: ast::Span,
        is_cancelled: impl Fn() -> bool + 'static,
    ) -> Result<V<Option<hir::SemanticSourceLocation>>, EditingCancelled> {
        Ok(match self.named_argument(function, span, is_cancelled)? {
            V::Known(argument) => V::Known(argument.and_then(|argument| argument.definition)),
            V::Unavailable(reason) => V::Unavailable(reason),
        })
    }

    pub fn named_argument(
        &mut self,
        function: hir::FunctionId,
        span: ast::Span,
        is_cancelled: impl Fn() -> bool + 'static,
    ) -> Result<V<Option<hir::EditingNamedArgument>>, EditingCancelled> {
        let calls = match self.calls(function, is_cancelled)? {
            V::Known(calls) => calls,
            V::Unavailable(reason) => return Ok(V::Unavailable(reason)),
        };
        let mut selected = calls.iter().find_map(|call| {
            call.arguments
                .iter()
                .find(|argument| argument.name_span == Some(span))
                .map(|argument| hir::EditingNamedArgument {
                    parameter: call.parameters[argument.parameter].clone(),
                    index: argument.parameter,
                    callee: call.callee,
                    context: call.context,
                    definition: None,
                })
        });
        if let Some(argument) = &mut selected {
            argument.definition = argument.callee.and_then(|callee| {
                self.lowerer
                    .editing_parameter_origin(callee, argument.index)
            });
        }
        Ok(V::Known(selected))
    }
}

impl Lowerer {
    pub(super) fn editing_property_origin(
        &self,
        target: hir::SemanticProperty,
    ) -> Option<hir::SemanticSourceLocation> {
        match target {
            hir::SemanticProperty::Current(property) => {
                let span = *self.property_name_spans.get(&property)?;
                let file = self.property_definition_file(property)?;
                Some(hir::SemanticSourceLocation {
                    source: self.source_identity(file).clone(),
                    span,
                })
            }
            hir::SemanticProperty::Imported(property) => self
                .dependencies
                .as_ref()?
                .property_definition_origin(property)
                .and_then(|origin| crate::semantic_queries::imported_location(origin).ok()),
        }
    }

    fn editing_parameter_origin(
        &self,
        callable: hir::SemanticCallable,
        parameter: usize,
    ) -> Option<hir::SemanticSourceLocation> {
        use hir::{DefaultCallableDeclarationV1 as D, SemanticCallable as C};
        use scoop_identity::CallableTemplateOrigin as Origin;
        match callable {
            C::Current(function) => {
                let declaration = self.signatures.get(&function)?.params.get(parameter)?;
                Some(hir::SemanticSourceLocation {
                    source: self
                        .source_identity(self.function_definition_file(function)?)
                        .clone(),
                    span: declaration.name.span,
                })
            }
            C::Imported(callable) => {
                let identity = match callable {
                    D::Function(id) => Origin::Function(id),
                    D::GenericFunction(id) => Origin::GenericFunction(id),
                    _ => return None,
                };
                let declaration = self
                    .dependencies
                    .as_ref()?
                    .callable_declaration(identity)
                    .ok()?;
                let parameter = declaration
                    .source_interface()?
                    .parameters()
                    .parameters()
                    .get(parameter)?;
                crate::semantic_queries::imported_location(parameter.definition_origin().origin())
                    .ok()
            }
            C::CurrentGetter(_) | C::CurrentSetter(_) => None,
        }
    }
}
