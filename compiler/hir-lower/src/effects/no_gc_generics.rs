//! Generic GC-free precondition inference and call-site validation.

use scoop_ast::Span;
use scoop_hir as hir;

use crate::Lowerer;
mod call_sites;
mod callables;
mod requirements;
pub(super) use callables::GenericCallable;

#[derive(Debug, Clone)]
pub(super) struct GenericCallSite {
    pub(super) caller: GenericCallable,
    pub(super) callee: GenericCallable,
    /// Complete declaration-parameter to application-argument relation. It
    /// is built from the exact callable application variants; consumers do
    /// not split or reconstruct an argument vector.
    pub(super) arguments: Vec<(hir::TypeParamId, hir::TypeId)>,
    pub(super) span: Span,
}

/// A fully resolved parameterized callable use before attaching the lexical
/// region that owns the expression. Keeping this edge caller-independent lets
/// predicate validation reuse the same typed walk for constructor/default
/// regions as for ordinary function bodies.
#[derive(Debug, Clone)]
pub(super) struct GenericCall {
    pub(super) callee: GenericCallable,
    /// Complete declaration-parameter to application-argument relation.
    pub(super) arguments: Vec<(hir::TypeParamId, hir::TypeId)>,
    pub(super) span: Span,
}

impl GenericCallSite {
    pub(in crate::effects) fn argument(&self, parameter: hir::TypeParamId) -> hir::TypeId {
        self.arguments
            .iter()
            .find_map(|(candidate, argument)| (*candidate == parameter).then_some(*argument))
            .expect("every parameterized call application binds every declaration parameter")
    }
}

impl Lowerer {
    pub(super) fn validate_no_gc_instantiations(&mut self) {
        let call_sites = self.generic_call_sites();

        // A parameterized caller inherits the concrete GC-free preconditions
        // of every parameterized callee. Iterate to a fixed point so wrappers
        // and mutually recursive call graphs retain the full condition.
        loop {
            let mut additions = Vec::new();
            for call_site in &call_sites {
                if self.effect_callable_parameters(call_site.caller).is_empty() {
                    continue;
                }
                let callee_requirements =
                    self.callable_no_gc_requirements(call_site.callee).to_vec();
                for parameter in callee_requirements {
                    let argument = call_site.argument(parameter);
                    let Some(mapped) = self.gc_free_requirements(argument) else {
                        continue;
                    };
                    for mapped_parameter in mapped {
                        if !self
                            .callable_no_gc_requirements(call_site.caller)
                            .contains(&mapped_parameter)
                        {
                            additions.push((call_site.caller, mapped_parameter));
                        }
                    }
                }
            }
            if additions.is_empty() {
                break;
            }
            for (function, parameter) in additions {
                self.add_callable_no_gc_requirement(function, parameter);
            }
        }

        // A ref-bound parameter can never satisfy a GC-free precondition.
        // Diagnose this on the exact callable declaration even if no concrete
        // caller has instantiated it yet.
        for callable in self.effect_callable_ids() {
            let impossible = self
                .callable_no_gc_requirements(callable)
                .iter()
                .copied()
                .filter(|p| {
                    self.effect_callable_parameter(callable, *p).kind() == hir::TypeParamKind::Ref
                })
                .collect::<Vec<_>>();
            for parameter in impossible {
                self.current_file = self.effect_callable_file(callable);
                self.error(self.effect_callable_span(callable), format!(
                    "generic {} `{}` cannot require ref-bound type parameter `{}` to be GC-free",
                    callable.kind(), self.effect_callable_name(callable),
                    self.effect_callable_parameter(callable, parameter).name,
                ));
            }
        }

        for call_site in call_sites {
            let requirements = self.callable_no_gc_requirements(call_site.callee).to_vec();
            if requirements.is_empty() {
                continue;
            }
            let caller_requirements =
                (!self.effect_callable_parameters(call_site.caller).is_empty())
                    .then(|| self.callable_no_gc_requirements(call_site.caller).to_vec());
            self.current_file = self.effect_callable_file(call_site.caller);

            for parameter in requirements {
                let argument = call_site.argument(parameter);
                let valid = match self.gc_free_requirements(argument) {
                    Some(mapped) if mapped.is_empty() => true,
                    Some(mapped) => caller_requirements
                        .as_ref()
                        .is_some_and(|caller| mapped.iter().all(|item| caller.contains(item))),
                    None => false,
                };
                if !valid {
                    self.error(
                        call_site.span,
                        format!(
                            "generic {} `{}` requires type argument {} for `{}` to be GC-free",
                            call_site.callee.kind(),
                            self.effect_callable_name(call_site.callee),
                            self.type_name(argument),
                            self.effect_callable_parameter(call_site.callee, parameter)
                                .name
                        ),
                    );
                }
            }
        }
    }
}
