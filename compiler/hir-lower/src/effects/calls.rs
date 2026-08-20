use scoop_ast::Span;
use scoop_hir as hir;

use crate::Lowerer;

impl Lowerer {
    pub(super) fn callable_target_is_no_gc(&self, callee: hir::CallableTarget) -> bool {
        match callee {
            hir::CallableTarget::Local(callable) => {
                self.functions[self.callable_function_id(callable)]
                    .attributes
                    .gc_effect
                    == hir::GcEffect::NoGc
            }
            hir::CallableTarget::Application(application) => {
                self.imported_generic_templates
                    [self.imported_generic_applications[application].template]
                    .attributes
                    .gc_effect
                    == hir::GcEffect::NoGc
            }
            hir::CallableTarget::Dependency(callee) => {
                let dependencies = self
                    .dependencies
                    .as_ref()
                    .expect("an external call retains its dependency declarations");
                let selected = dependencies
                    .resolve_callable(self.imported_dependency_callables[callee].reference())
                    .expect("a call retains its selected declaration");
                if selected.capability().gc_effect() == scoop_identity::GcEffect::NoGc {
                    return true;
                }
                match (
                    selected.interface().declaration(),
                    selected.interface().owner(),
                ) {
                    (
                        scoop_identity::CallableTemplateOrigin::Constructor(constructor),
                        hir::PublicDeclarationOwnerV1::Nominal(hir::SourceNominalId::Concrete(
                            owner,
                        )),
                    ) => dependencies.nominal(owner).is_some_and(|nominal| {
                        nominal
                            .interface
                            .declaration_details()
                            .primary_value_constructor()
                            == Some(constructor)
                    }),
                    _ => false,
                }
            }
        }
    }

    pub(super) fn check_no_gc_call_target(
        &self,
        callee: hir::CallableTarget,
        span: Span,
        out: &mut Vec<(Span, String)>,
    ) {
        if self.callable_target_is_no_gc(callee) {
            return;
        }
        let message = match callee {
            hir::CallableTarget::Local(callable) => format!(
                "`@NoGC` code may not call managed function `{}`",
                self.functions[self.callable_function_id(callable)].name,
            ),
            hir::CallableTarget::Application(_) => {
                "calling a managed dependency function is not allowed in `@NoGC` code".into()
            }
            hir::CallableTarget::Dependency(_) => {
                "managed dependency calls are not allowed in `@NoGC` code".into()
            }
        };
        out.push((span, message));
    }

    pub(super) fn check_no_gc_callee(
        &self,
        callable: hir::Callable,
        span: Span,
        out: &mut Vec<(Span, String)>,
    ) {
        self.check_no_gc_call_target(callable.into(), span, out);
    }

    pub(super) fn check_no_gc_function(
        &self,
        function: hir::FunctionId,
        span: Span,
        out: &mut Vec<(Span, String)>,
    ) {
        self.check_no_gc_callee(hir::Callable::Function(function), span, out);
    }
}
