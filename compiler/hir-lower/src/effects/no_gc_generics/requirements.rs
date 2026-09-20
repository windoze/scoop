use super::*;

impl Lowerer {
    pub(in crate::effects) fn set_function_no_gc_requirements(
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

    pub(in crate::effects) fn callable_no_gc_requirements(
        &self,
        callable: GenericCallable,
    ) -> &[hir::TypeParamId] {
        match callable {
            GenericCallable::Function(id) => self.function_no_gc_requirements(id),
            GenericCallable::ClassConstructor(id) => &self.class_constructors[id].no_gc_type_params,
            GenericCallable::StructConstructor(id) => {
                &self.struct_constructors[id].no_gc_type_params
            }
        }
    }

    pub(in crate::effects) fn add_callable_no_gc_requirement(
        &mut self,
        callable: GenericCallable,
        parameter: hir::TypeParamId,
    ) {
        self.effect_callable_parameter(callable, parameter);
        let mut requirements = self.callable_no_gc_requirements(callable).to_vec();
        if requirements.contains(&parameter) {
            return;
        }
        requirements.push(parameter);
        requirements.sort_by_key(|p| p.into_raw());
        match callable {
            GenericCallable::Function(id) => self.set_function_no_gc_requirements(id, requirements),
            GenericCallable::ClassConstructor(id) => {
                self.class_constructors[id].no_gc_type_params = requirements
            }
            GenericCallable::StructConstructor(id) => {
                self.struct_constructors[id].no_gc_type_params = requirements
            }
        }
    }
}
