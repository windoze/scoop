use scoop_hir as hir;

use crate::Lowerer;

use super::occurrences::{
    collect_body_types, collect_class_constructor_types, collect_constructor_arguments_types,
};
use super::requirements::{extend_default_types, merge_requirements};
use super::{PointeeApplicationOccurrence, PointeeRequirementCallSite, RequirementContext};

impl Lowerer {
    pub(super) fn infer_nominal_gc_free_pointee_requirements(
        &mut self,
        default_occurrences: &[PointeeApplicationOccurrence],
    ) -> bool {
        let mut any_changed = false;
        loop {
            let mut changed = false;

            let structs: Vec<_> = self
                .structs
                .iter()
                .map(|(id, declaration)| {
                    let mut types = declaration
                        .semantic_fields()
                        .iter()
                        .map(|field| field.ty)
                        .collect::<Vec<_>>();
                    types.extend(declaration.interfaces.iter().copied());
                    for constructor in &declaration.constructors {
                        let constructor = &self.struct_constructors[*constructor];
                        types.extend(constructor.parameters.iter().map(|parameter| parameter.ty));
                        if let hir::StructConstructorKind::Secondary { delegation, body } =
                            &constructor.kind
                        {
                            collect_constructor_arguments_types(
                                self,
                                &delegation.arguments,
                                &mut types,
                            );
                            collect_body_types(self, body, &mut types);
                        }
                    }
                    extend_default_types(
                        default_occurrences,
                        RequirementContext::Nominal(crate::Owner::Struct(id)),
                        &mut types,
                    );
                    (
                        id,
                        types,
                        self.owner_callable_pointee_parameters(crate::Owner::Struct(id)),
                    )
                })
                .collect();
            for (id, types, callable_requirements) in structs {
                let mut additions = self.pointee_parameters_in_types(&types);
                additions.extend(callable_requirements);
                changed |= merge_requirements(
                    &mut self.structs[id].gc_free_pointee_requirements,
                    additions,
                );
            }

            let enums: Vec<_> = self
                .enums
                .iter()
                .map(|(id, declaration)| {
                    let mut types = declaration
                        .variants
                        .iter()
                        .flat_map(|variant| variant.fields.iter().map(|field| field.ty))
                        .collect::<Vec<_>>();
                    types.extend(declaration.interfaces.iter().copied());
                    extend_default_types(
                        default_occurrences,
                        RequirementContext::Nominal(crate::Owner::Enum(id)),
                        &mut types,
                    );
                    (
                        id,
                        types,
                        self.owner_callable_pointee_parameters(crate::Owner::Enum(id)),
                    )
                })
                .collect();
            for (id, types, callable_requirements) in enums {
                let mut additions = self.pointee_parameters_in_types(&types);
                additions.extend(callable_requirements);
                changed |=
                    merge_requirements(&mut self.enums[id].gc_free_pointee_requirements, additions);
            }

            let classes: Vec<_> = self
                .classes
                .iter()
                .map(|(id, declaration)| {
                    let mut types = declaration
                        .fields
                        .iter()
                        .map(|field| self.class_fields[*field].ty)
                        .collect::<Vec<_>>();
                    types.extend(declaration.base_class);
                    types.extend(declaration.interfaces.iter().copied());
                    for constructor in &declaration.constructors {
                        let constructor = &self.class_constructors[*constructor];
                        types.extend(constructor.parameters.iter().map(|parameter| parameter.ty));
                        collect_class_constructor_types(self, constructor, &mut types);
                    }
                    extend_default_types(
                        default_occurrences,
                        RequirementContext::Nominal(crate::Owner::Class(id)),
                        &mut types,
                    );
                    (
                        id,
                        types,
                        self.owner_callable_pointee_parameters(crate::Owner::Class(id)),
                    )
                })
                .collect();
            for (id, types, callable_requirements) in classes {
                let mut additions = self.pointee_parameters_in_types(&types);
                additions.extend(callable_requirements);
                changed |= merge_requirements(
                    &mut self.classes[id].gc_free_pointee_requirements,
                    additions,
                );
            }

            let interfaces: Vec<_> = self
                .interfaces
                .iter()
                .map(|(id, declaration)| {
                    let types = declaration
                        .parents
                        .iter()
                        .map(|parent| self.interface_applications[*parent].canonical_type)
                        .collect::<Vec<_>>();
                    (
                        id,
                        types,
                        self.owner_callable_pointee_parameters(crate::Owner::Interface(id)),
                    )
                })
                .collect();
            for (id, types, callable_requirements) in interfaces {
                let mut additions = self.pointee_parameters_in_types(&types);
                additions.extend(callable_requirements);
                changed |= merge_requirements(
                    &mut self.interfaces[id].gc_free_pointee_requirements,
                    additions,
                );
            }

            if !changed {
                break;
            }
            any_changed = true;
        }
        any_changed
    }

    pub(super) fn infer_callable_gc_free_pointee_requirements(
        &mut self,
        default_occurrences: &[PointeeApplicationOccurrence],
    ) -> bool {
        let functions: Vec<_> = self
            .functions
            .iter()
            .map(|(id, function)| {
                let mut types: Vec<_> = function
                    .params
                    .iter()
                    .map(|parameter| parameter.ty)
                    .collect();
                types.push(function.return_ty);
                if let hir::FunctionKind::User(body) = &function.kind {
                    collect_body_types(self, body, &mut types);
                }
                extend_default_types(
                    default_occurrences,
                    RequirementContext::Function(id),
                    &mut types,
                );
                (id, types)
            })
            .collect();
        let mut changed = false;
        for (function, types) in functions {
            let requirements = self.pointee_parameters_in_types(&types);
            changed |= self.merge_function_gc_free_pointee_requirements(function, requirements);
        }
        changed
    }

    pub(super) fn propagate_callable_gc_free_pointee_requirements(
        &mut self,
        call_sites: &[PointeeRequirementCallSite],
    ) -> bool {
        let mut function_additions = Vec::new();
        let mut nominal_additions = Vec::new();
        for call_site in call_sites {
            let requirements = self
                .function_gc_free_pointee_requirements(call_site.callee)
                .to_vec();
            for requirement in requirements {
                let argument = call_site.argument(requirement.type_param);
                let Some(mapped) = self.gc_free_requirements(argument) else {
                    continue;
                };
                for parameter in mapped {
                    match call_site.context {
                        RequirementContext::Function(function)
                            if self.functions[function].type_param_count() != 0 =>
                        {
                            if !self
                                .function_gc_free_pointee_requirements(function)
                                .iter()
                                .any(|requirement| requirement.type_param == parameter)
                            {
                                function_additions.push((function, parameter));
                            }
                        }
                        RequirementContext::Nominal(owner) => {
                            if self.nominal_owns_type_parameter(owner, parameter)
                                && !self
                                    .nominal_gc_free_pointee_requirements(owner)
                                    .iter()
                                    .any(|requirement| requirement.type_param == parameter)
                            {
                                nominal_additions.push((owner, parameter));
                            }
                        }
                        RequirementContext::Function(_) | RequirementContext::Closed => {}
                    }
                }
            }
        }
        let changed = !function_additions.is_empty() || !nominal_additions.is_empty();
        for (function, parameter) in function_additions {
            self.add_function_gc_free_pointee_requirement(function, parameter);
        }
        for (owner, parameter) in nominal_additions {
            self.add_nominal_gc_free_pointee_requirement(owner, parameter);
        }
        changed
    }
}
