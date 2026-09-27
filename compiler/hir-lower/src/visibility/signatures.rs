//! Declaration-site checks for types exposed by lookup and dispatch slots.

use super::*;

impl Lowerer {
    fn type_parameter_signature_types(
        &self,
        parameters: &[hir::TypeParamDecl],
    ) -> Vec<hir::TypeId> {
        let mut result = Vec::new();
        for parameter in parameters {
            for bound in parameter.nominal_bounds_in_source_order() {
                let ty = match bound {
                    hir::NominalBoundRef::Class(bound) => {
                        self.class_applications[bound.application].canonical_type
                    }
                    hir::NominalBoundRef::Interface(bound) => {
                        self.interface_applications[bound.application].canonical_type
                    }
                };
                result.push(ty);
            }
        }
        result
    }

    pub(crate) fn check_signature_exposure(
        &mut self,
        access: &hir::DeclarationAccess,
        signature_types: &[hir::TypeId],
        span: ast::Span,
        declaration: &str,
    ) {
        let mut requirements = vec![access.lookup.0.clone()];
        if let Some(slot) = &access.slot
            && !requirements.contains(&slot.0)
        {
            requirements.push(slot.0.clone());
        }
        let mut dependencies = Vec::new();
        for &ty in signature_types {
            self.collect_type_dependencies(ty, &mut dependencies);
        }
        for (dependency, provided) in dependencies {
            for required in &requirements {
                if !self.access_domain_is_subset(required, &provided) {
                    let dependency_name = self.type_name(dependency);
                    self.error(
                        span,
                        format!(
                            "signature of {declaration} exposes type `{dependency_name}` outside its access domain"
                        ),
                    );
                }
            }
        }
    }

    pub(crate) fn validate_signature_exposure(&mut self) {
        let functions = self
            .signatures
            .iter()
            .map(|(&id, signature)| {
                let mut types = self.type_parameter_signature_types(&signature.type_params);
                types.extend(signature.params.iter().map(|parameter| parameter.ty));
                types.push(signature.return_ty);
                (id, types)
            })
            .collect::<Vec<_>>();
        for (id, types) in functions {
            self.current_file = self.function_files[&id];
            let access = self.functions[id].access.clone();
            let name = self.functions[id].name.clone();
            self.check_signature_exposure(
                &access,
                &types,
                self.functions[id].span,
                &format!("function `{name}`"),
            );
        }

        let properties = self
            .properties
            .iter()
            .map(|(id, value)| {
                (
                    id,
                    value.ty,
                    value.span,
                    value.name.clone(),
                    value.access.clone(),
                    value.owner,
                )
            })
            .collect::<Vec<_>>();
        for (id, ty, span, name, access, owner) in properties {
            self.current_file = match owner {
                hir::PropertyOwner::TopLevel => self.property_files[&id],
                hir::PropertyOwner::Extension(_) => self.property_files[&id],
                hir::PropertyOwner::Class(owner) => self.class_files[&owner],
                hir::PropertyOwner::Struct(owner) => self.struct_files[&owner],
                hir::PropertyOwner::Enum(owner) => self.enum_files[&owner],
                hir::PropertyOwner::Interface(owner) => self.interface_files[&owner],
                hir::PropertyOwner::Object(owner) => self.object_files[&owner],
            };
            let mut signature_types = vec![ty];
            if let hir::PropertyOwner::Extension(extension) = owner {
                signature_types.insert(0, self.extension_properties[extension].receiver_ty);
            }
            self.check_signature_exposure(
                &access,
                &signature_types,
                span,
                &format!("property `{name}`"),
            );

            let getter = self.properties[id].capability.getter();
            let getter_access = self.property_getters[getter].access.clone();
            self.check_signature_exposure(
                &getter_access,
                &[ty],
                self.property_getters[getter].span,
                &format!("getter of property `{name}`"),
            );

            if let Some(setter) = self.properties[id].capability.setter() {
                let setter_access = self.property_setters[setter].access.clone();
                self.check_signature_exposure(
                    &setter_access,
                    &[ty],
                    self.property_setters[setter].span,
                    &format!("setter of property `{name}`"),
                );
            }
        }

        let class_constructors = self
            .class_constructors
            .iter()
            .map(|(id, value)| {
                (
                    id,
                    value
                        .parameters
                        .iter()
                        .map(|parameter| parameter.ty)
                        .collect::<Vec<_>>(),
                    value.span,
                    self.classes[value.owner].name.clone(),
                    value.access.clone(),
                )
            })
            .collect::<Vec<_>>();
        for (id, types, span, owner, access) in class_constructors {
            self.current_file = self.class_files[&self.class_constructors[id].owner];
            self.check_signature_exposure(
                &access,
                &types,
                span,
                &format!("constructor of class `{owner}`"),
            );
        }

        let struct_constructors = self
            .struct_constructors
            .iter()
            .map(|(id, value)| {
                (
                    id,
                    value
                        .parameters
                        .iter()
                        .map(|parameter| parameter.ty)
                        .collect::<Vec<_>>(),
                    value.span,
                    self.structs[value.owner].name.clone(),
                    value.access.clone(),
                )
            })
            .collect::<Vec<_>>();
        for (id, types, span, owner, access) in struct_constructors {
            self.current_file = self.struct_files[&self.struct_constructors[id].owner];
            self.check_signature_exposure(
                &access,
                &types,
                span,
                &format!("constructor of struct `{owner}`"),
            );
        }

        self.validate_nominal_signature_exposure();
    }

    fn validate_nominal_signature_exposure(&mut self) {
        let structs = self
            .structs
            .iter()
            .map(|(id, value)| {
                let mut types = self.type_parameter_signature_types(&value.type_params);
                types.extend(value.semantic_fields().iter().map(|field| field.ty));
                types.extend(value.interfaces.iter().copied());
                (
                    id,
                    types,
                    value.span,
                    value.name.clone(),
                    value.access.clone(),
                )
            })
            .collect::<Vec<_>>();
        for (id, types, span, name, access) in structs {
            self.current_file = self.struct_files[&id];
            let declaration = hir::DeclarationAccess {
                declared: access.declared,
                lookup: access.lookup.clone(),
                slot: None,
            };
            self.check_signature_exposure(&declaration, &types, span, &format!("struct `{name}`"));
        }

        let enums = self
            .enums
            .iter()
            .map(|(id, value)| {
                let mut types = self.type_parameter_signature_types(&value.type_params);
                types.extend(
                    value
                        .variants
                        .iter()
                        .flat_map(|variant| variant.fields.iter().map(|field| field.ty)),
                );
                types.extend(value.interfaces.iter().copied());
                (
                    id,
                    types,
                    value.span,
                    value.name.clone(),
                    value.access.clone(),
                )
            })
            .collect::<Vec<_>>();
        for (id, types, span, name, access) in enums {
            self.current_file = self.enum_files[&id];
            let declaration = hir::DeclarationAccess {
                declared: access.declared,
                lookup: access.lookup.clone(),
                slot: None,
            };
            self.check_signature_exposure(&declaration, &types, span, &format!("enum `{name}`"));
        }

        let classes = self
            .classes
            .iter()
            .map(|(id, value)| {
                let mut types = self.type_parameter_signature_types(&value.type_params);
                types.extend(value.base_class);
                types.extend(value.interfaces.iter().copied());
                (
                    id,
                    types,
                    value.span,
                    value.name.clone(),
                    value.access.clone(),
                )
            })
            .collect::<Vec<_>>();
        for (id, types, span, name, access) in classes {
            self.current_file = self.class_files[&id];
            let declaration = hir::DeclarationAccess {
                declared: access.declared,
                lookup: access.lookup.clone(),
                slot: None,
            };
            self.check_signature_exposure(&declaration, &types, span, &format!("class `{name}`"));
        }

        let interfaces = self
            .interfaces
            .iter()
            .map(|(id, value)| {
                let mut types = self.type_parameter_signature_types(&value.type_params);
                types.extend(value.parents.iter().copied());
                (
                    id,
                    types,
                    value.span,
                    value.name.clone(),
                    value.access.clone(),
                )
            })
            .collect::<Vec<_>>();
        for (id, types, span, name, access) in interfaces {
            self.current_file = self.interface_files[&id];
            let declaration = hir::DeclarationAccess {
                declared: access.declared,
                lookup: access.lookup.clone(),
                slot: None,
            };
            self.check_signature_exposure(
                &declaration,
                &types,
                span,
                &format!("interface `{name}`"),
            );
        }
    }
}
