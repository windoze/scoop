//! Property lookup retains the original receiver and complete declaring application.

use super::*;

impl Lowerer {
    pub(crate) fn find_accessible_nominal_property(
        &mut self,
        receiver_ty: TypeId,
        name: &str,
    ) -> Option<(hir::PropertyId, hir::MethodOwnerApplication, TypeId)> {
        let lookup_ty = match self.types[receiver_ty] {
            hir::Type::Param(parameter) => {
                let parameter = self
                    .type_params_in_scope
                    .iter()
                    .find(|candidate| candidate.id == parameter)?;
                let hir::TypeParamBounds::Nominal(bounds) = &parameter.bounds else {
                    return None;
                };
                bounds.class.as_ref()?.ty
            }
            _ => receiver_ty,
        };
        match self.types[lookup_ty].clone() {
            hir::Type::Class(application) => {
                let (declaring, property, ty) = self.find_accessible_class_application_property(
                    application,
                    name,
                    receiver_ty,
                )?;
                let owner = match self.properties[property].owner {
                    hir::PropertyOwner::Object(object) => {
                        hir::MethodOwnerApplication::Object(self.objects[object].object_type)
                    }
                    _ => hir::MethodOwnerApplication::Class(declaring),
                };
                Some((property, owner, ty))
            }
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
                let ty = self.instantiate_ty(self.properties[property].ty, &value.arguments);
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
                let ty = self.instantiate_ty(self.properties[property].ty, &value.arguments);
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
        if let Some(property) = self.interfaces[self.interface_id(value.template)]
            .properties
            .iter()
            .copied()
            .find(|&property| {
                self.properties[property].name == name
                    && self.property_is_accessible(property, Some(receiver_ty))
            })
        {
            let ty = self.instantiate_ty(self.properties[property].ty, &value.arguments);
            return Some((property, application, ty));
        }
        for parent in self.interfaces[self.interface_id(value.template)]
            .parents
            .clone()
        {
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
