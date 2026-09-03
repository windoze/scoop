use super::*;

impl Lowerer {
    /// Resolve a class's constructor properties, base-class clause and
    /// interface list (pass 2).
    pub(crate) fn resolve_class(&mut self, id: ClassId, decl: &ast::ClassDecl) {
        self.type_params_in_scope = self.classes[id].type_params.clone();
        if matches!(
            self.classes[id].representation,
            hir::ClassRepresentation::Intrinsic(_)
        ) {
            let interfaces = self.resolve_supertype_interface_list(&decl.supertypes);
            self.classes[id].interfaces = interfaces;
            self.type_params_in_scope.clear();
            return;
        }
        let mut seen = std::collections::HashSet::new();
        let mut props = Vec::new();
        let mut parameter_calling = Vec::new();
        for prop in &decl.constructor {
            if !seen.insert(prop.name.text.clone()) {
                self.error(
                    prop.name.span,
                    format!(
                        "duplicate property `{}` in class `{}`",
                        prop.name.text, decl.name.text
                    ),
                );
                continue;
            }
            let Some((ty, calling)) = self.resolve_parameter(&prop.ty, &prop.syntax) else {
                continue; // diagnostic already recorded
            };
            if prop.property == ast::PrimaryParameterProperty::Plain {
                self.error(
                    prop.span,
                    "ordinary primary constructor parameters require M19 constructor lowering"
                        .to_string(),
                );
                continue;
            }
            props.push(hir::ConstructorField {
                parameter: hir::ConstructorParamId::from_raw(props.len() as u32),
                name: prop.name.text.clone(),
                ty,
                mutable: prop.property.is_mutable(),
            });
            parameter_calling.push(calling);
        }
        self.classes[id].representation = hir::ClassRepresentation::Declared(props);
        self.class_parameter_calling.insert(id, parameter_calling);

        self.resolve_class_supertypes(id, &decl.supertypes);
        self.type_params_in_scope.clear();
    }

    fn resolve_class_supertypes(&mut self, id: ClassId, specs: &[ast::SupertypeSpec]) {
        let mut interfaces = Vec::new();
        let mut base = None;
        for spec in specs {
            let Some(ty) = self.resolve_type_ref(&spec.ty) else {
                continue;
            };
            match self.types[ty] {
                Type::Class(application) => {
                    if base.is_some() {
                        self.error(
                            spec.span,
                            "a class may have only one direct base class".into(),
                        );
                        continue;
                    }
                    let base_id = self.class_applications[application].template;
                    if self.classes[base_id].modifier == hir::ClassModifier::Final {
                        self.error(
                            spec.ty.span,
                            format!(
                                "class `{}` is final and cannot be inherited",
                                self.classes[base_id].name
                            ),
                        );
                        continue;
                    }
                    base = Some((
                        ty,
                        hir::ConstructorDelegation {
                            locals: la_arena::Arena::new(),
                            statements: Vec::new(),
                            args: Vec::new(),
                        },
                    ));
                }
                Type::Interface(_) => {
                    if spec.constructor_arguments.is_some() {
                        self.error(
                            spec.span,
                            "interfaces cannot have constructor arguments".into(),
                        );
                    } else if !interfaces.iter().any(|&other| self.types_equal(other, ty)) {
                        interfaces.push(ty);
                    }
                }
                _ => self.error(
                    spec.ty.span,
                    format!("`{}` is not a class or interface", self.type_name(ty)),
                ),
            }
        }
        self.classes[id].base_class = base;
        self.classes[id].interfaces = interfaces;
    }

    /// Resolve an interface list (`: I1, I2`) on any declaration —
    /// classes, structs and enums share the rules (spec 9.1 / 4.4.3):
    /// every name must be an interface, duplicates are dropped.
    pub(crate) fn resolve_interface_list(&mut self, refs: &[ast::TypeRef]) -> Vec<TypeId> {
        let mut interfaces = Vec::new();
        for ty_ref in refs {
            let Some(ty) = self.resolve_type_ref(ty_ref) else {
                continue;
            };
            if !matches!(self.types[ty], Type::Interface(..)) {
                self.error(
                    ty_ref.span,
                    format!("`{}` is not an interface", self.type_name(ty)),
                );
                continue;
            }
            if !interfaces.iter().any(|&other| self.types_equal(other, ty)) {
                interfaces.push(ty);
            }
        }
        interfaces
    }

    pub(crate) fn resolve_supertype_interface_list(
        &mut self,
        specs: &[ast::SupertypeSpec],
    ) -> Vec<TypeId> {
        let mut interfaces = Vec::new();
        for spec in specs {
            if spec.constructor_arguments.is_some() {
                self.error(
                    spec.span,
                    "interfaces and value types cannot use supertype constructor arguments".into(),
                );
                continue;
            }
            let Some(ty) = self.resolve_type_ref(&spec.ty) else {
                continue;
            };
            if !matches!(self.types[ty], Type::Interface(..)) {
                self.error(
                    spec.ty.span,
                    format!("`{}` is not an interface", self.type_name(ty)),
                );
                continue;
            }
            if !interfaces.iter().any(|&other| self.types_equal(other, ty)) {
                interfaces.push(ty);
            }
        }
        interfaces
    }
}
