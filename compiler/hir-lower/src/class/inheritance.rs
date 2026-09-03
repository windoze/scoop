use super::*;
mod interfaces;
mod overrides;
mod signatures;

impl Lowerer {
    /// Inheritance checks (pass 2.75): cycles, property shadowing,
    /// `override` rules and interface implementation — for classes and
    /// for value types implementing interfaces (spec 4.4.3).
    pub(crate) fn check_inheritance(
        &mut self,
        pending_classes: &[(ClassId, &ast::ClassDecl, usize)],
        pending_structs: &[(hir::StructId, &ast::StructDecl, usize)],
        pending_enums: &[(hir::EnumId, &ast::EnumDecl, usize)],
        pending_methods: &[(FunctionId, &ast::FunctionDecl, usize, Owner)],
    ) {
        for &(id, decl, file_index) in pending_classes {
            self.current_file = file_index;
            self.check_inheritance_cycle(id, decl);
            self.check_property_shadowing(id, decl);
        }
        for &(id, decl, file_index, owner) in pending_methods {
            self.current_file = file_index;
            self.check_override_rules(id, decl, owner);
        }
        for &(id, decl, file_index) in pending_classes {
            self.current_file = file_index;
            self.check_interface_implementation(id, decl);
        }
        for &(id, decl, file_index) in pending_structs {
            self.current_file = file_index;
            self.check_value_interface_implementation(Owner::Struct(id), decl.span);
        }
        for &(id, decl, file_index) in pending_enums {
            self.current_file = file_index;
            self.check_value_interface_implementation(Owner::Enum(id), decl.span);
        }
    }

    /// A class may not directly or indirectly inherit from itself.
    fn check_inheritance_cycle(&mut self, id: ClassId, decl: &ast::ClassDecl) {
        let mut seen = vec![id];
        let mut current = id;
        while let Some((base_ty, _)) = self.classes[current].base_class {
            let Type::Class(base_application) = self.types[base_ty] else {
                unreachable!("resolved class bases are class applications")
            };
            let base = self.class_applications[base_application].template;
            if seen.contains(&base) {
                let name = self.classes[id].name.clone();
                self.error(
                    decl.span,
                    format!("class `{name}` directly or indirectly inherits from itself"),
                );
                return;
            }
            seen.push(base);
            current = base;
        }
    }

    /// M6 simplification: a constructor property may not reuse the name
    /// of a base-class property (no field shadowing).
    fn check_property_shadowing(&mut self, id: ClassId, decl: &ast::ClassDecl) {
        let Some((base_ty, _)) = self.classes[id].base_class else {
            return;
        };
        let Type::Class(base_application) = self.types[base_ty] else {
            unreachable!("resolved class bases are class applications")
        };
        let base = self.class_applications[base_application].template;
        for prop in &decl.constructor {
            if let Some((declaring, _, _, _)) = self.find_class_field(base, &prop.name.text) {
                let base_name = self.classes[declaring].name.clone();
                self.error(
                    prop.name.span,
                    format!(
                        "property `{}` of class `{}` shadows a property of base class `{base_name}`",
                        prop.name.text, decl.name.text
                    ),
                );
            }
        }
    }
}
