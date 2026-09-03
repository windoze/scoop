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
            let interfaces = self.resolve_interface_list(&decl.interfaces);
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
            props.push(hir::ConstructorField {
                parameter: hir::ConstructorParamId::from_raw(props.len() as u32),
                name: prop.name.text.clone(),
                ty,
                mutable: prop.mutable,
            });
            parameter_calling.push(calling);
        }
        self.classes[id].representation = hir::ClassRepresentation::Declared(props);
        self.class_parameter_calling.insert(id, parameter_calling);

        if let Some((base_ref, _)) = &decl.base_class
            && let Some(base_ty) = self.resolve_type_ref(base_ref)
        {
            let Type::Class(base_application) = self.types[base_ty] else {
                self.error(
                    base_ref.span,
                    format!("`{}` is not a class", self.type_name(base_ty)),
                );
                self.type_params_in_scope.clear();
                return;
            };
            let base_id = self.class_applications[base_application].template;
            if self.classes[base_id].modifier == hir::ClassModifier::Final {
                self.error(
                    base_ref.span,
                    format!(
                        "class `{}` is final and cannot be inherited",
                        self.classes[base_id].name
                    ),
                );
            } else {
                // Constructor arguments are lowered in pass 3.
                self.classes[id].base_class = Some((base_ty, Vec::new()));
            }
        }

        let interfaces = self.resolve_interface_list(&decl.interfaces);
        self.classes[id].interfaces = interfaces;
        self.type_params_in_scope.clear();
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
}
