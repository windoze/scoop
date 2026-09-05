//! Generic GC-free precondition inference and call-site validation.

use scoop_ast::Span;
use scoop_hir as hir;

use crate::Lowerer;
mod call_sites;

#[derive(Debug, Clone)]
pub(super) struct GenericCallSite {
    pub(super) caller: hir::FunctionId,
    pub(super) callee: hir::FunctionId,
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
    pub(super) callee: hir::FunctionId,
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
                if self.functions[call_site.caller].type_param_count() == 0 {
                    continue;
                }
                let callee_requirements =
                    self.function_no_gc_requirements(call_site.callee).to_vec();
                for parameter in callee_requirements {
                    let argument = call_site.argument(parameter);
                    let Some(mapped) = self.gc_free_requirements(argument) else {
                        continue;
                    };
                    for mapped_parameter in mapped {
                        if !self
                            .function_no_gc_requirements(call_site.caller)
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
                self.add_function_no_gc_requirement(function, parameter);
            }
        }

        // A ref-bound parameter can never satisfy a GC-free precondition.
        // Diagnose this on the exact callable declaration even if no concrete
        // caller has instantiated it yet.
        let impossible_requirements: Vec<_> = self
            .functions
            .iter()
            .flat_map(|(function_id, function)| {
                self.function_no_gc_requirements(function_id)
                    .iter()
                    .copied()
                    .filter(|parameter| {
                        function.type_param(*parameter).kind() == hir::TypeParamKind::Ref
                    })
                    .map(|parameter| {
                        (
                            function_id,
                            parameter,
                            function.span,
                            function.name.clone(),
                            function.type_param(parameter).name.clone(),
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        for (function, _, span, function_name, parameter_name) in impossible_requirements {
            self.current_file = self
                .function_files
                .get(&function)
                .copied()
                .unwrap_or(self.user_file_index);
            self.error(
                span,
                format!(
                    "generic function `{function_name}` cannot require ref-bound type parameter `{parameter_name}` to be GC-free"
                ),
            );
        }

        for call_site in call_sites {
            let requirements = self.function_no_gc_requirements(call_site.callee).to_vec();
            if requirements.is_empty() {
                continue;
            }
            let callee = self.functions[call_site.callee].clone();
            let caller_requirements = (self.functions[call_site.caller].type_param_count() != 0)
                .then(|| self.function_no_gc_requirements(call_site.caller).to_vec());
            self.current_file = self
                .function_files
                .get(&call_site.caller)
                .copied()
                .unwrap_or(self.user_file_index);

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
                            "generic function `{}` requires type argument {} for `{}` to be GC-free",
                            callee.name,
                            self.type_name(argument),
                            callee.type_param(parameter).name
                        ),
                    );
                }
            }
        }
    }

    pub(super) fn set_function_no_gc_requirements(
        &mut self,
        function: hir::FunctionId,
        mut requirements: Vec<hir::TypeParamId>,
    ) {
        requirements.sort_by_key(|parameter| parameter.into_raw());
        requirements.dedup();
        match self.functions[function].genericity.clone() {
            hir::FunctionGenericity::Plain => {
                assert!(
                    requirements.is_empty(),
                    "a parameter-free function cannot have type-parameter requirements"
                );
            }
            hir::FunctionGenericity::Generic { definition, .. } => {
                self.generic_functions[definition].no_gc_type_params = requirements;
            }
            hir::FunctionGenericity::OwnerParameterizedMethod { .. } => {
                let hir::FunctionGenericity::OwnerParameterizedMethod {
                    no_gc_type_params, ..
                } = &mut self.functions[function].genericity
                else {
                    unreachable!("the matched genericity remains stable")
                };
                *no_gc_type_params = requirements;
            }
            hir::FunctionGenericity::GenericMethod { definition, .. } => {
                self.generic_methods[definition].no_gc_type_params = requirements;
            }
        }
    }

    fn function_no_gc_requirements(&self, function: hir::FunctionId) -> &[hir::TypeParamId] {
        match &self.functions[function].genericity {
            hir::FunctionGenericity::Plain => &[],
            hir::FunctionGenericity::Generic { definition, .. } => {
                &self.generic_functions[*definition].no_gc_type_params
            }
            hir::FunctionGenericity::OwnerParameterizedMethod {
                no_gc_type_params, ..
            } => no_gc_type_params,
            hir::FunctionGenericity::GenericMethod { definition, .. } => {
                &self.generic_methods[*definition].no_gc_type_params
            }
        }
    }

    fn add_function_no_gc_requirement(
        &mut self,
        function: hir::FunctionId,
        parameter: hir::TypeParamId,
    ) {
        let mut requirements = self.function_no_gc_requirements(function).to_vec();
        if !requirements.contains(&parameter) {
            requirements.push(parameter);
            self.set_function_no_gc_requirements(function, requirements);
        }
    }
}
