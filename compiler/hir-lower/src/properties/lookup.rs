//! Property lookup retains the original receiver and complete declaring application.

use super::*;

impl Lowerer {
    pub(crate) fn find_accessible_nominal_property(
        &mut self,
        receiver_ty: TypeId,
        name: &str,
    ) -> Option<(hir::PropertyId, hir::MethodOwnerApplication, TypeId)> {
        let lookup_ty = match self.types[receiver_ty] {
            hir::Type::Param(_) | hir::Type::Companion(_) => self
                .parameter_nominal_bounds(receiver_ty)
                .into_iter()
                .find_map(|bound| match bound {
                    hir::NominalBoundRef::Class(bound) => Some(bound.ty),
                    hir::NominalBoundRef::Interface(_) => None,
                })?,
            _ => receiver_ty,
        };
        self.find_accessible_declared_nominal_property(lookup_ty, receiver_ty, name)
            .or_else(|| self.find_selected_interface_property(lookup_ty, receiver_ty, name))
            .filter(|&(property, owner, _)| {
                if self.property_implementation_condition(property).is_none() {
                    return true;
                }
                let arguments = self.method_owner_arguments(owner).to_vec();
                let getter = self.properties[property].capability.getter();
                let function = match self.property_getters[getter].implementation {
                    hir::PropertyAccessorImplementation::Body(function)
                    | hir::PropertyAccessorImplementation::AbstractSlot(function) => function,
                    _ => unreachable!("conditional properties have no storage"),
                };
                let parameters = self.functions[function]
                    .type_params()
                    .into_iter()
                    .cloned()
                    .collect::<Vec<_>>();
                self.conditional_parameters_hold(&parameters, &arguments)
            })
    }

    fn find_accessible_declared_nominal_property(
        &mut self,
        lookup_ty: TypeId,
        receiver_ty: TypeId,
        name: &str,
    ) -> Option<(hir::PropertyId, hir::MethodOwnerApplication, TypeId)> {
        let class = match self.types[lookup_ty] {
            hir::Type::Class(application) => Some(application),
            hir::Type::String => self
                .intrinsic_type_owners
                .get(&hir::IntrinsicTypeKind::String)
                .and_then(|(owner, _)| match owner {
                    crate::IntrinsicTypeOwner::Class(owner) => {
                        Some(self.classes[*owner].self_application)
                    }
                    crate::IntrinsicTypeOwner::Struct(_) => None,
                }),
            _ => None,
        };
        if let Some(application) = class {
            let (declaring, property, ty) =
                self.find_accessible_class_application_property(application, name, receiver_ty)?;
            let owner = match self.properties[property].owner {
                hir::PropertyOwner::Object(object) => self.method_owner_application(
                    crate::Owner::Object(object),
                    self.class_applications[declaring].arguments.clone(),
                ),
                _ => hir::MethodOwnerApplication::Class(declaring),
            };
            return Some((property, owner, ty));
        }
        match self.types[lookup_ty].clone() {
            hir::Type::Struct(application) => {
                let value = self.struct_applications[application].clone();
                let property = self.structs[self.source_struct_id(value.template)?]
                    .properties
                    .iter()
                    .copied()
                    .find(|&property| {
                        self.properties[property].name == name
                            && self.property_is_accessible(property, Some(receiver_ty))
                    })?;
                let ty = self.infer_property_type(property)?;
                let ty = self.instantiate_ty(ty, &value.arguments);
                Some((
                    property,
                    hir::MethodOwnerApplication::Struct(application),
                    ty,
                ))
            }
            hir::Type::Enum(application) => {
                let value = self.enum_applications[application].clone();
                let property = self.enums[self.source_enum_id(value.template)?]
                    .properties
                    .iter()
                    .copied()
                    .find(|&property| {
                        self.properties[property].name == name
                            && self.property_is_accessible(property, Some(receiver_ty))
                    })?;
                let ty = self.infer_property_type(property)?;
                let ty = self.instantiate_ty(ty, &value.arguments);
                Some((property, hir::MethodOwnerApplication::Enum(application), ty))
            }
            hir::Type::Interface(application) => {
                let (property, declaring, ty) = self
                    .find_accessible_interface_application_property(
                        application,
                        name,
                        receiver_ty,
                        &mut Vec::new(),
                    )?;
                Some((
                    property,
                    hir::MethodOwnerApplication::Interface(declaring),
                    ty,
                ))
            }
            _ => None,
        }
    }

    fn find_selected_interface_property(
        &mut self,
        lookup_ty: TypeId,
        receiver_ty: TypeId,
        name: &str,
    ) -> Option<(hir::PropertyId, hir::MethodOwnerApplication, TypeId)> {
        let mut selected = Vec::new();
        self.collect_selected_interface_members(lookup_ty, &mut selected);
        for (candidate, _, _) in selected {
            let crate::CallableCandidateOwner::Method(owner) = candidate.owner else {
                unreachable!("selected interface members have method owners");
            };
            let hir::MethodOwnerApplication::Interface(application) = owner else {
                unreachable!("selected interface members retain their interface application");
            };
            let application = self.interface_applications[application].clone();
            let interface = self.source_interface_id(application.template)?;
            let property = self.interfaces[interface]
                .properties
                .iter()
                .copied()
                .find(|&id| {
                    let property = &self.properties[id];
                    let getter = &self.property_getters[property.capability.getter()];
                    property.name == name
                        && matches!(
                            getter.implementation,
                            hir::PropertyAccessorImplementation::Body(function)
                                | hir::PropertyAccessorImplementation::StorageBody(function)
                                | hir::PropertyAccessorImplementation::AbstractSlot(function)
                                if function == candidate.function
                        )
                        && self.property_is_accessible(id, Some(receiver_ty))
                });
            if let Some(property) = property {
                let ty = self.infer_property_type(property)?;
                let ty = self.instantiate_ty(ty, &application.arguments);
                return Some((property, owner, ty));
            }
        }
        None
    }

    pub(crate) fn find_accessible_interface_application_property(
        &mut self,
        application: hir::InterfaceApplicationId,
        name: &str,
        receiver_ty: TypeId,
        seen: &mut Vec<hir::InterfaceApplicationId>,
    ) -> Option<(hir::PropertyId, hir::InterfaceApplicationId, TypeId)> {
        if seen.contains(&application) {
            return None;
        }
        seen.push(application);
        let value = self.interface_applications[application].clone();
        let source_id = self.source_interface_id(value.template)?;
        if let Some(property) =
            self.interfaces[source_id]
                .properties
                .iter()
                .copied()
                .find(|&property| {
                    self.properties[property].name == name
                        && self.property_is_accessible(property, Some(receiver_ty))
                })
        {
            let ty = self.infer_property_type(property)?;
            let ty = self.instantiate_ty(ty, &value.arguments);
            return Some((property, application, ty));
        }
        for parent in self.interfaces[source_id].parents.clone() {
            let parent_ty = self.instantiate_ty(parent, &value.arguments);
            let hir::Type::Interface(parent) = self.types[parent_ty] else {
                continue;
            };
            if let Some(property) =
                self.find_accessible_interface_application_property(parent, name, receiver_ty, seen)
            {
                return Some(property);
            }
        }
        None
    }
}
