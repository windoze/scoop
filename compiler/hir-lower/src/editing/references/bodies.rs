use super::*;

impl ReferenceBuilder {
    pub(super) fn body(
        &mut self,
        analysis: &mut DeclarationAnalysis,
        function: &hir::EditingSourceBody,
        check: &Rc<dyn Fn() -> bool>,
    ) -> Result<(), EditingCancelled> {
        let cancellation = check.clone();
        let V::Known(facts) = analysis.source_body_facts(function.id, move || cancellation())?
        else {
            self.parameters(analysis, function);
            self.unavailable.push(function.origin.clone());
            return Ok(());
        };
        let locals = facts.locals.clone();
        let callables = facts.callables.clone();
        let properties = facts.properties.clone();
        let local_functions = facts.local_functions.clone();
        self.types(&facts.types);
        self.import_uses
            .extend(facts.imports.iter().map(|reference| {
                (
                    hir::SemanticSourceLocation {
                        source: function.origin.source.clone(),
                        span: reference.span,
                    },
                    reference.imports.clone(),
                )
            }));
        self.parameters(analysis, function);
        for declaration in &local_functions {
            if check() {
                return Err(EditingCancelled);
            }
            self.function(analysis, declaration);
            self.parameters(
                analysis,
                &hir::EditingSourceBody {
                    id: declaration.function.into(),
                    owner: hir::EditingSourceBodyOwner::Callable(hir::SemanticCallable::Current(
                        declaration.function,
                    )),
                    origin: declaration.name_origin.clone(),
                },
            );
        }
        let source = &function.origin.source;
        for local in locals {
            if check() {
                return Err(EditingCancelled);
            }
            let target = self
                .parameter_definitions
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
            match self
                .identities
                .callee(&mut analysis.lowerer, reference.target)
            {
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
            match self
                .identities
                .property(&mut analysis.lowerer, reference.target)
            {
                Some(target) => self.record(Target::Property(target), origin, false),
                None => self.unavailable.push(origin),
            }
        }
        let cancellation = check.clone();
        let V::Known(calls) = analysis.calls(function.id, move || cancellation())? else {
            return Ok(());
        };
        let calls = calls.to_vec();
        for call in calls {
            if check() {
                return Err(EditingCancelled);
            }
            self.call_parameters(&mut analysis.lowerer, source, &call);
        }
        Ok(())
    }

    pub(super) fn call_parameters(
        &mut self,
        lowerer: &mut Lowerer,
        source: &scoop_identity::SourceIdentity,
        call: &hir::EditingAppliedCall,
    ) {
        let Some(callee) = call
            .callee
            .and_then(|callee| self.identities.callee(lowerer, callee))
        else {
            return;
        };
        for argument in &call.arguments {
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

    fn parameters(&mut self, analysis: &mut DeclarationAnalysis, body: &hir::EditingSourceBody) {
        let Some(declaration) = body.callee() else {
            return;
        };
        let parameters = analysis.body_parameters(body);
        let Some(callable) = self.identities.callee(&mut analysis.lowerer, declaration) else {
            return;
        };
        for (index, parameter) in parameters.into_iter().enumerate() {
            let target = Target::Parameter {
                callable,
                index: index as u32,
            };
            self.record(target, parameter, true);
        }
    }
}
