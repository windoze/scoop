use super::*;

impl ReferenceBuilder {
    pub(super) fn body(
        &mut self,
        analysis: &mut DeclarationAnalysis,
        function: &hir::EditingFunction,
        check: &Rc<dyn Fn() -> bool>,
    ) -> Result<(), EditingCancelled> {
        let cancellation = check.clone();
        let V::Known(facts) = analysis.function_facts(function.function, move || cancellation())?
        else {
            self.unavailable.push(function.name_origin.clone());
            return Ok(());
        };
        let locals = facts.locals.clone();
        let callables = facts.callables.clone();
        let properties = facts.properties.clone();
        let local_functions = facts.local_functions.clone();
        let mut parameters = HashMap::new();
        for declaration in std::iter::once(function).chain(local_functions.iter()) {
            if check() {
                return Err(EditingCancelled);
            }
            if declaration.function != function.function {
                self.function(analysis, declaration);
            }
            let V::Known(signature) = analysis.signature(declaration.function) else {
                continue;
            };
            let Some(callable) = self.callable(
                &mut analysis.lowerer,
                hir::SemanticCallable::Current(declaration.function),
            ) else {
                continue;
            };
            for (index, parameter) in signature.parameters.iter().enumerate() {
                parameters.insert(
                    parameter.origin.clone(),
                    Target::Parameter {
                        callable,
                        index: index as u32,
                    },
                );
            }
        }
        let source = &function.name_origin.source;
        for local in locals {
            if check() {
                return Err(EditingCancelled);
            }
            let target = parameters
                .get(&local.reference.definition)
                .copied()
                .unwrap_or(Target::Binding(local.reference.binding));
            self.record(
                target,
                hir::SemanticSourceLocation {
                    source: source.clone(),
                    span: local.span,
                },
                local.role == hir::EditingLocalRole::Declaration,
            );
        }
        for reference in callables {
            if check() {
                return Err(EditingCancelled);
            }
            let origin = hir::SemanticSourceLocation {
                source: source.clone(),
                span: reference.span,
            };
            match self.callable(&mut analysis.lowerer, reference.target) {
                Some(target) => self.record(Target::Callable(target), origin, false),
                None => self.unavailable.push(origin),
            }
        }
        for reference in properties {
            if check() {
                return Err(EditingCancelled);
            }
            let origin = hir::SemanticSourceLocation {
                source: source.clone(),
                span: reference.span,
            };
            match self.property(&mut analysis.lowerer, reference.target) {
                Some(target) => self.record(Target::Property(target), origin, false),
                None => self.unavailable.push(origin),
            }
        }
        let cancellation = check.clone();
        let V::Known(calls) = analysis.calls(function.function, move || cancellation())? else {
            return Ok(());
        };
        let calls = calls.to_vec();
        for call in calls {
            if check() {
                return Err(EditingCancelled);
            }
            let Some(callee) = call
                .callee
                .and_then(|callee| self.callable(&mut analysis.lowerer, callee))
            else {
                continue;
            };
            for argument in call.arguments {
                if let Some(span) = argument.name_span {
                    self.record(
                        Target::Parameter {
                            callable: callee,
                            index: argument.parameter as u32,
                        },
                        hir::SemanticSourceLocation {
                            source: source.clone(),
                            span,
                        },
                        false,
                    );
                }
            }
        }
        Ok(())
    }
}
