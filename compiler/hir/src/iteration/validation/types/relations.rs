use std::collections::HashSet;

use super::*;

impl Validator<'_> {
    pub(in super::super) fn checked_interface_application(
        &self,
        id: InterfaceApplicationId,
    ) -> Check<&InterfaceApplication> {
        let application = checked_arena(&self.module.interface_applications, id)
            .ok_or_else(|| invalid("invalid interface application in iteration plan"))?;
        let declaration = checked_arena(&self.module.interfaces, application.template)
            .ok_or_else(|| invalid("interface application has an invalid template"))?;
        if declaration.type_params.len() != application.arguments.len()
            || application
                .arguments
                .iter()
                .any(|argument| !arena_contains(&self.module.types, *argument))
            || !arena_contains(&self.module.types, application.canonical_type)
            || !matches!(self.module.types[application.canonical_type], Type::Interface(found) if found == id)
        {
            return fail("interface application is not canonical");
        }
        Ok(application)
    }

    pub(in super::super) fn exact_interface_applications(
        &self,
        ty: TypeId,
        target: InterfaceId,
    ) -> Check<Vec<InterfaceApplicationId>> {
        if checked_arena(&self.module.interfaces, target).is_none() {
            return fail("iteration target interface is invalid");
        }
        let ty = checked_arena(&self.module.types, ty)
            .ok_or_else(|| invalid("iterator result has an invalid type"))?;
        let mut roots = Vec::new();
        match ty {
            Type::Interface(application) => roots.push(*application),
            Type::Class(application) => {
                self.collect_class_interfaces(*application, &mut roots, &mut HashSet::new())?
            }
            Type::Struct(application) => {
                self.collect_struct_interfaces(*application, &mut roots)?
            }
            Type::Enum(application) => self.collect_enum_interfaces(*application, &mut roots)?,
            Type::Integer(kind) => {
                let owner = self.protocols.fundamental_types.integers.owner(*kind);
                let application = checked_arena(&self.module.structs, owner)
                    .ok_or_else(|| invalid("integer core owner is invalid"))?
                    .self_application;
                self.collect_struct_interfaces(application, &mut roots)?;
            }
            Type::Boolean => {
                let owner = self.protocols.fundamental_types.boolean;
                let application = checked_arena(&self.module.structs, owner)
                    .ok_or_else(|| invalid("Boolean core owner is invalid"))?
                    .self_application;
                self.collect_struct_interfaces(application, &mut roots)?;
            }
            Type::String => {
                let owner = self.protocols.fundamental_types.string;
                let application = checked_arena(&self.module.classes, owner)
                    .ok_or_else(|| invalid("String core owner is invalid"))?
                    .self_application;
                self.collect_class_interfaces(application, &mut roots, &mut HashSet::new())?;
            }
            Type::Param(parameter) => {
                let parameter = self.parameter(*parameter)?;
                if let Some(bound) = parameter.class_bound() {
                    self.collect_class_interfaces(
                        bound.application,
                        &mut roots,
                        &mut HashSet::new(),
                    )?;
                }
                roots.extend(
                    parameter
                        .interface_bounds()
                        .iter()
                        .map(|bound| bound.application),
                );
            }
            Type::ImportedStruct(_)
            | Type::ImportedEnum(_)
            | Type::ImportedClass(_)
            | Type::Unit
            | Type::Any
            | Type::Tuple(_)
            | Type::Function(_)
            | Type::Ptr(_)
            | Type::FunPtr(_) => {}
        }

        let mut closure = Vec::new();
        for root in roots {
            self.append_interface_closure(root, &mut closure)?;
        }
        let mut result = Vec::new();
        for application in closure {
            if self.checked_interface_application(application)?.template == target
                && !result.contains(&application)
            {
                result.push(application);
            }
        }
        Ok(result)
    }

    pub(super) fn collect_class_interfaces(
        &self,
        application: ClassApplicationId,
        out: &mut Vec<InterfaceApplicationId>,
        seen: &mut HashSet<u32>,
    ) -> Check {
        if !seen.insert(application.into_raw().into_u32()) {
            return Ok(());
        }
        let application_value = self.checked_class_application(application)?;
        let declaration = &self.module.classes[application_value.template];
        let bindings = declaration
            .type_params
            .iter()
            .zip(&application_value.arguments)
            .map(|(parameter, argument)| (parameter.id, *argument))
            .collect::<Vec<_>>();
        for interface in &declaration.interfaces {
            out.push(self.interface_from_type(self.instantiate_type(*interface, &bindings)?)?);
        }
        if let Some(base) = declaration.base_class {
            let base = self.instantiate_type(base, &bindings)?;
            let Type::Class(base) = self.module.types[base] else {
                return fail("class base is not a class application");
            };
            self.collect_class_interfaces(base, out, seen)?;
        }
        Ok(())
    }

    fn collect_struct_interfaces(
        &self,
        application: StructApplicationId,
        out: &mut Vec<InterfaceApplicationId>,
    ) -> Check {
        let application_value = self.checked_struct_application(application)?;
        let declaration = &self.module.structs[application_value.template];
        let bindings = declaration
            .type_params
            .iter()
            .zip(&application_value.arguments)
            .map(|(parameter, argument)| (parameter.id, *argument))
            .collect::<Vec<_>>();
        for interface in &declaration.interfaces {
            out.push(self.interface_from_type(self.instantiate_type(*interface, &bindings)?)?);
        }
        Ok(())
    }

    fn collect_enum_interfaces(
        &self,
        application: EnumApplicationId,
        out: &mut Vec<InterfaceApplicationId>,
    ) -> Check {
        let application_value = self.checked_enum_application(application)?;
        let declaration = &self.module.enums[application_value.template];
        let bindings = declaration
            .type_params
            .iter()
            .zip(&application_value.arguments)
            .map(|(parameter, argument)| (parameter.id, *argument))
            .collect::<Vec<_>>();
        for interface in &declaration.interfaces {
            out.push(self.interface_from_type(self.instantiate_type(*interface, &bindings)?)?);
        }
        Ok(())
    }

    pub(super) fn append_interface_closure(
        &self,
        application: InterfaceApplicationId,
        out: &mut Vec<InterfaceApplicationId>,
    ) -> Check {
        if out.contains(&application) {
            return Ok(());
        }
        let application_value = self.checked_interface_application(application)?;
        out.push(application);
        let declaration = &self.module.interfaces[application_value.template];
        let bindings = declaration
            .type_params
            .iter()
            .zip(&application_value.arguments)
            .map(|(parameter, argument)| (parameter.id, *argument))
            .collect::<Vec<_>>();
        for parent in &declaration.parents {
            let parent = self.checked_interface_application(*parent)?.canonical_type;
            let parent = self.instantiate_type(parent, &bindings)?;
            self.append_interface_closure(self.interface_from_type(parent)?, out)?;
        }
        Ok(())
    }

    fn interface_from_type(&self, ty: TypeId) -> Check<InterfaceApplicationId> {
        let Type::Interface(application) = self.module.types[ty] else {
            return fail("declared interface edge does not have interface type");
        };
        self.checked_interface_application(application)?;
        Ok(application)
    }

    pub(in super::super) fn instantiate_type(
        &self,
        source: TypeId,
        bindings: &[(TypeParamId, TypeId)],
    ) -> Check<TypeId> {
        let source_value = checked_arena(&self.module.types, source)
            .ok_or_else(|| invalid("type substitution starts from an invalid type"))?;
        match source_value {
            Type::Param(parameter) => Ok(bindings
                .iter()
                .find_map(|(candidate, argument)| (candidate == parameter).then_some(*argument))
                .unwrap_or(source)),
            Type::Struct(application) => {
                let source = self.checked_struct_application(*application)?;
                let arguments = self.instantiate_types(&source.arguments, bindings)?;
                self.module
                    .struct_applications
                    .iter()
                    .find_map(|(_, candidate)| {
                        (candidate.template == source.template && candidate.arguments == arguments)
                            .then_some(candidate.canonical_type)
                    })
                    .ok_or_else(|| invalid("substituted struct application is not interned"))
            }
            Type::Class(application) => {
                let source = self.checked_class_application(*application)?;
                let arguments = self.instantiate_types(&source.arguments, bindings)?;
                self.module
                    .class_applications
                    .iter()
                    .find_map(|(_, candidate)| {
                        (candidate.template == source.template && candidate.arguments == arguments)
                            .then_some(candidate.canonical_type)
                    })
                    .ok_or_else(|| invalid("substituted class application is not interned"))
            }
            Type::Interface(application) => {
                let source = self.checked_interface_application(*application)?;
                let arguments = self.instantiate_types(&source.arguments, bindings)?;
                self.module
                    .interface_applications
                    .iter()
                    .find_map(|(_, candidate)| {
                        (candidate.template == source.template && candidate.arguments == arguments)
                            .then_some(candidate.canonical_type)
                    })
                    .ok_or_else(|| invalid("substituted interface application is not interned"))
            }
            Type::Enum(application) => {
                let source = self.checked_enum_application(*application)?;
                let arguments = self.instantiate_types(&source.arguments, bindings)?;
                self.module
                    .enum_applications
                    .iter()
                    .find_map(|(_, candidate)| {
                        (candidate.template == source.template && candidate.arguments == arguments)
                            .then_some(candidate.canonical_type)
                    })
                    .ok_or_else(|| invalid("substituted enum application is not interned"))
            }
            Type::Tuple(elements) => {
                let elements = self.instantiate_types(elements, bindings)?;
                self.find_type(
                    |candidate| matches!(candidate, Type::Tuple(found) if *found == elements),
                )
            }
            Type::Function(function) => {
                let function = checked_arena(&self.module.function_types, *function)
                    .ok_or_else(|| invalid("function type is invalid"))?;
                let parameters = self.instantiate_types(&function.parameter_types, bindings)?;
                let result = self.instantiate_type(function.return_type, bindings)?;
                self.module
                    .function_types
                    .iter()
                    .find_map(|(_, candidate)| {
                        (candidate.is_suspend == function.is_suspend
                            && candidate.parameter_types == parameters
                            && candidate.return_type == result)
                            .then_some(candidate.canonical_type)
                    })
                    .ok_or_else(|| invalid("substituted function type is not interned"))
            }
            Type::Ptr(pointee) => {
                let pointee = self.instantiate_type(*pointee, bindings)?;
                self.find_type(
                    |candidate| matches!(candidate, Type::Ptr(found) if *found == pointee),
                )
            }
            Type::FunPtr(function) => {
                let canonical = checked_arena(&self.module.function_types, *function)
                    .ok_or_else(|| invalid("native function type is invalid"))?
                    .canonical_type;
                let Type::Function(_) = self.module.types[canonical] else {
                    return fail("native function signature lacks a canonical managed shape");
                };
                let function_ty = self.instantiate_type(canonical, bindings)?;
                let Type::Function(function) = self.module.types[function_ty] else {
                    return fail("substituted native function signature is invalid");
                };
                self.find_type(
                    |candidate| matches!(candidate, Type::FunPtr(found) if *found == function),
                )
            }
            Type::ImportedStruct(_)
            | Type::ImportedEnum(_)
            | Type::ImportedClass(_)
            | Type::Unit
            | Type::Integer(_)
            | Type::Boolean
            | Type::String
            | Type::Any => Ok(source),
        }
    }

    fn instantiate_types(
        &self,
        source: &[TypeId],
        bindings: &[(TypeParamId, TypeId)],
    ) -> Check<Vec<TypeId>> {
        source
            .iter()
            .map(|ty| self.instantiate_type(*ty, bindings))
            .collect()
    }

    fn find_type(&self, predicate: impl Fn(&Type) -> bool) -> Check<TypeId> {
        self.module
            .types
            .iter()
            .find_map(|(id, candidate)| predicate(candidate).then_some(id))
            .ok_or_else(|| invalid("substituted structural type is not interned"))
    }

    pub(in super::super) fn checked_struct_application(
        &self,
        id: StructApplicationId,
    ) -> Check<&StructApplication> {
        let application = checked_arena(&self.module.struct_applications, id)
            .ok_or_else(|| invalid("invalid struct application"))?;
        let declaration = checked_arena(&self.module.structs, application.template)
            .ok_or_else(|| invalid("struct application has an invalid template"))?;
        if declaration.type_params.len() != application.arguments.len()
            || application
                .arguments
                .iter()
                .any(|argument| !arena_contains(&self.module.types, *argument))
            || !arena_contains(&self.module.types, application.canonical_type)
            || !matches!(self.module.types[application.canonical_type], Type::Struct(found) if found == id)
        {
            return fail("struct application is not canonical");
        }
        Ok(application)
    }

    pub(in super::super) fn checked_class_application(
        &self,
        id: ClassApplicationId,
    ) -> Check<&ClassApplication> {
        let application = checked_arena(&self.module.class_applications, id)
            .ok_or_else(|| invalid("invalid class application"))?;
        let declaration = checked_arena(&self.module.classes, application.template)
            .ok_or_else(|| invalid("class application has an invalid template"))?;
        if declaration.type_params.len() != application.arguments.len()
            || application
                .arguments
                .iter()
                .any(|argument| !arena_contains(&self.module.types, *argument))
            || !arena_contains(&self.module.types, application.canonical_type)
            || !matches!(self.module.types[application.canonical_type], Type::Class(found) if found == id)
        {
            return fail("class application is not canonical");
        }
        Ok(application)
    }

    pub(super) fn checked_enum_application(
        &self,
        id: EnumApplicationId,
    ) -> Check<&EnumApplication> {
        let application = checked_arena(&self.module.enum_applications, id)
            .ok_or_else(|| invalid("invalid enum application"))?;
        let declaration = checked_arena(&self.module.enums, application.template)
            .ok_or_else(|| invalid("enum application has an invalid template"))?;
        if declaration.type_params.len() != application.arguments.len()
            || application
                .arguments
                .iter()
                .any(|argument| !arena_contains(&self.module.types, *argument))
            || !arena_contains(&self.module.types, application.canonical_type)
            || !matches!(self.module.types[application.canonical_type], Type::Enum(found) if found == id)
        {
            return fail("enum application is not canonical");
        }
        Ok(application)
    }

    pub(in super::super) fn parameter(&self, id: TypeParamId) -> Check<&TypeParamDecl> {
        if !self.parameters.contains(&id) {
            return fail("iteration plan references a type parameter outside its body");
        }
        self.module
            .functions
            .iter()
            .flat_map(|(_, function)| function.type_params())
            .chain(
                self.module
                    .structs
                    .iter()
                    .flat_map(|(_, declaration)| declaration.type_params.iter()),
            )
            .chain(
                self.module
                    .enums
                    .iter()
                    .flat_map(|(_, declaration)| declaration.type_params.iter()),
            )
            .chain(
                self.module
                    .classes
                    .iter()
                    .flat_map(|(_, declaration)| declaration.type_params.iter()),
            )
            .chain(
                self.module
                    .interfaces
                    .iter()
                    .flat_map(|(_, declaration)| declaration.type_params.iter()),
            )
            .find(|parameter| parameter.id == id)
            .ok_or_else(|| invalid("iteration plan type parameter has no declaration"))
    }
}
