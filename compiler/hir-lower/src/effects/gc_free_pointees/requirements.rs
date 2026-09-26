use std::collections::HashSet;

use scoop_hir as hir;

use crate::Lowerer;

use super::{PointeeApplicationOccurrence, RequirementContext};

impl Lowerer {
    pub(super) fn pointee_parameters_in_types(
        &self,
        types: &[hir::TypeId],
    ) -> Vec<hir::TypeParamId> {
        let mut requirements = HashSet::new();
        for &ty in types {
            self.collect_pointee_parameters(ty, &mut HashSet::new(), &mut requirements);
        }
        let mut requirements = requirements.into_iter().collect::<Vec<_>>();
        requirements.sort_by_key(|parameter| parameter.into_raw());
        requirements
    }

    pub(super) fn collect_pointee_parameters(
        &self,
        ty: hir::TypeId,
        visiting: &mut HashSet<hir::TypeId>,
        out: &mut HashSet<hir::TypeParamId>,
    ) {
        if !visiting.insert(ty) {
            return;
        }
        match &self.types[ty] {
            hir::Type::Ptr(pointee) => {
                if let Some(requirements) = self.gc_free_requirements(*pointee) {
                    out.extend(requirements);
                }
                self.collect_pointee_parameters(*pointee, visiting, out);
            }
            hir::Type::Struct(application) => {
                let application = &self.struct_applications[*application];
                self.collect_application_pointee_parameters(
                    &self.structs[application.template].type_params,
                    &self.structs[application.template].gc_free_pointee_requirements,
                    &application.arguments,
                    out,
                );
                for &argument in &application.arguments {
                    self.collect_pointee_parameters(argument, visiting, out);
                }
            }
            hir::Type::Class(application) => {
                let application = &self.class_applications[*application];
                self.collect_application_pointee_parameters(
                    &self.classes[application.template].type_params,
                    &self.classes[application.template].gc_free_pointee_requirements,
                    &application.arguments,
                    out,
                );
                for &argument in &application.arguments {
                    self.collect_pointee_parameters(argument, visiting, out);
                }
            }
            hir::Type::Interface(application) => {
                let application = &self.interface_applications[*application];
                self.collect_application_pointee_parameters(
                    &self.interfaces[application.template].type_params,
                    &self.interfaces[application.template].gc_free_pointee_requirements,
                    &application.arguments,
                    out,
                );
                for &argument in &application.arguments {
                    self.collect_pointee_parameters(argument, visiting, out);
                }
            }
            hir::Type::Enum(application) => {
                let application = &self.enum_applications[*application];
                self.collect_application_pointee_parameters(
                    &self.enums[application.template].type_params,
                    &self.enums[application.template].gc_free_pointee_requirements,
                    &application.arguments,
                    out,
                );
                for &argument in &application.arguments {
                    self.collect_pointee_parameters(argument, visiting, out);
                }
            }
            hir::Type::Tuple(elements) => {
                for &element in elements {
                    self.collect_pointee_parameters(element, visiting, out);
                }
            }
            hir::Type::Function(function) | hir::Type::FunPtr(function) => {
                let function = &self.function_types[*function];
                for &parameter in &function.parameter_types {
                    self.collect_pointee_parameters(parameter, visiting, out);
                }
                self.collect_pointee_parameters(function.return_type, visiting, out);
            }
            hir::Type::ImportedStruct(_)
            | hir::Type::Unit
            | hir::Type::Integer(_)
            | hir::Type::Boolean
            | hir::Type::String
            | hir::Type::Any
            | hir::Type::Param(_) => {}
        }
        visiting.remove(&ty);
    }

    fn collect_application_pointee_parameters(
        &self,
        parameters: &[hir::TypeParamDecl],
        requirements: &[hir::RequiresGcFreePointee],
        arguments: &[hir::TypeId],
        out: &mut HashSet<hir::TypeParamId>,
    ) {
        for requirement in requirements {
            let index = parameters
                .iter()
                .position(|parameter| parameter.id == requirement.type_param)
                .expect("a nominal pointee requirement names its own parameter");
            if let Some(mapped) = self.gc_free_requirements(arguments[index]) {
                out.extend(mapped);
            }
        }
    }

    fn set_function_gc_free_pointee_requirements(
        &mut self,
        function: hir::FunctionId,
        parameters: Vec<hir::TypeParamId>,
    ) {
        let requirements = parameters
            .into_iter()
            .map(|type_param| hir::RequiresGcFreePointee { type_param })
            .collect();
        match self.functions[function].genericity.clone() {
            hir::FunctionGenericity::Plain => {}
            hir::FunctionGenericity::Generic { definition, .. } => {
                self.generic_functions[definition].gc_free_pointee_requirements = requirements;
            }
            hir::FunctionGenericity::OwnerParameterizedMethod { .. } => {
                let hir::FunctionGenericity::OwnerParameterizedMethod {
                    gc_free_pointee_requirements,
                    ..
                } = &mut self.functions[function].genericity
                else {
                    unreachable!("the matched function genericity remains stable")
                };
                *gc_free_pointee_requirements = requirements;
            }
            hir::FunctionGenericity::GenericMethod { definition, .. } => {
                self.generic_methods[definition].gc_free_pointee_requirements = requirements;
            }
        }
    }

    pub(super) fn merge_function_gc_free_pointee_requirements(
        &mut self,
        function: hir::FunctionId,
        additions: Vec<hir::TypeParamId>,
    ) -> bool {
        let mut parameters = self
            .function_gc_free_pointee_requirements(function)
            .iter()
            .map(|requirement| requirement.type_param)
            .collect::<Vec<_>>();
        let original = parameters.len();
        parameters.extend(additions);
        parameters.sort_by_key(|parameter| parameter.into_raw());
        parameters.dedup();
        if parameters.len() == original {
            return false;
        }
        self.set_function_gc_free_pointee_requirements(function, parameters);
        true
    }

    pub(super) fn function_gc_free_pointee_requirements(
        &self,
        function: hir::FunctionId,
    ) -> &[hir::RequiresGcFreePointee] {
        match &self.functions[function].genericity {
            hir::FunctionGenericity::Plain => &[],
            hir::FunctionGenericity::Generic { definition, .. } => {
                &self.generic_functions[*definition].gc_free_pointee_requirements
            }
            hir::FunctionGenericity::OwnerParameterizedMethod {
                gc_free_pointee_requirements,
                ..
            } => gc_free_pointee_requirements,
            hir::FunctionGenericity::GenericMethod { definition, .. } => {
                &self.generic_methods[*definition].gc_free_pointee_requirements
            }
        }
    }

    pub(super) fn add_function_gc_free_pointee_requirement(
        &mut self,
        function: hir::FunctionId,
        parameter: hir::TypeParamId,
    ) {
        let mut parameters = self
            .function_gc_free_pointee_requirements(function)
            .iter()
            .map(|requirement| requirement.type_param)
            .collect::<Vec<_>>();
        if !parameters.contains(&parameter) {
            parameters.push(parameter);
            parameters.sort_by_key(|parameter| parameter.into_raw());
            self.set_function_gc_free_pointee_requirements(function, parameters);
        }
    }

    pub(super) fn add_nominal_gc_free_pointee_requirement(
        &mut self,
        owner: crate::Owner,
        parameter: hir::TypeParamId,
    ) {
        let requirements = match owner {
            crate::Owner::Struct(id) => &mut self.structs[id].gc_free_pointee_requirements,
            crate::Owner::Enum(id) => &mut self.enums[id].gc_free_pointee_requirements,
            crate::Owner::Class(id) => &mut self.classes[id].gc_free_pointee_requirements,
            crate::Owner::Interface(id) => &mut self.interfaces[id].gc_free_pointee_requirements,
            crate::Owner::Object(_) => return,
        };
        merge_requirements(requirements, vec![parameter]);
    }

    pub(super) fn nominal_owns_type_parameter(
        &self,
        owner: crate::Owner,
        parameter: hir::TypeParamId,
    ) -> bool {
        match owner {
            crate::Owner::Struct(id) => &self.structs[id].type_params,
            crate::Owner::Enum(id) => &self.enums[id].type_params,
            crate::Owner::Class(id) => &self.classes[id].type_params,
            crate::Owner::Interface(id) => &self.interfaces[id].type_params,
            crate::Owner::Object(_) => return false,
        }
        .iter()
        .any(|candidate| candidate.id == parameter)
    }

    pub(super) fn allowed_pointee_parameters(
        &self,
        context: RequirementContext,
    ) -> HashSet<hir::TypeParamId> {
        match context {
            RequirementContext::Function(function) => self
                .function_gc_free_pointee_requirements(function)
                .iter()
                .map(|requirement| requirement.type_param)
                .collect(),
            RequirementContext::Nominal(owner) => self
                .nominal_gc_free_pointee_requirements(owner)
                .iter()
                .map(|requirement| requirement.type_param)
                .collect(),
            RequirementContext::Closed => HashSet::new(),
        }
    }

    pub(super) fn owner_callable_pointee_parameters(
        &self,
        owner: crate::Owner,
    ) -> Vec<hir::TypeParamId> {
        let owner_parameters = match owner {
            crate::Owner::Struct(id) => &self.structs[id].type_params,
            crate::Owner::Enum(id) => &self.enums[id].type_params,
            crate::Owner::Class(id) => &self.classes[id].type_params,
            crate::Owner::Interface(id) => &self.interfaces[id].type_params,
            crate::Owner::Object(_) => return Vec::new(),
        };
        let owner_parameters = owner_parameters
            .iter()
            .map(|parameter| parameter.id)
            .collect::<HashSet<_>>();
        let mut requirements = self
            .function_owner
            .iter()
            .filter_map(|(function, candidate)| (*candidate == owner).then_some(*function))
            .flat_map(|function| {
                self.function_gc_free_pointee_requirements(function)
                    .iter()
                    .map(|requirement| requirement.type_param)
            })
            .filter(|parameter| owner_parameters.contains(parameter))
            .collect::<Vec<_>>();
        requirements.sort_by_key(|parameter| parameter.into_raw());
        requirements.dedup();
        requirements
    }

    pub(super) fn nominal_gc_free_pointee_requirements(
        &self,
        owner: crate::Owner,
    ) -> &[hir::RequiresGcFreePointee] {
        match owner {
            crate::Owner::Struct(id) => &self.structs[id].gc_free_pointee_requirements,
            crate::Owner::Enum(id) => &self.enums[id].gc_free_pointee_requirements,
            crate::Owner::Class(id) => &self.classes[id].gc_free_pointee_requirements,
            crate::Owner::Interface(id) => &self.interfaces[id].gc_free_pointee_requirements,
            crate::Owner::Object(_) => &[],
        }
    }
}

pub(super) fn extend_default_types(
    occurrences: &[PointeeApplicationOccurrence],
    context: RequirementContext,
    out: &mut Vec<hir::TypeId>,
) {
    out.extend(
        occurrences
            .iter()
            .filter_map(|occurrence| (occurrence.context == context).then_some(occurrence.ty)),
    );
}

pub(super) fn merge_requirements(
    target: &mut Vec<hir::RequiresGcFreePointee>,
    additions: Vec<hir::TypeParamId>,
) -> bool {
    let original = target.len();
    for type_param in additions {
        if !target
            .iter()
            .any(|requirement| requirement.type_param == type_param)
        {
            target.push(hir::RequiresGcFreePointee { type_param });
        }
    }
    target.sort_by_key(|requirement| requirement.type_param.into_raw());
    target.len() != original
}
