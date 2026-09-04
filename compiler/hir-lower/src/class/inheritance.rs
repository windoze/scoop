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
            self.current_owner = Some(Owner::Class(id));
            self.check_inheritance_cycle(id, decl);
            self.check_property_shadowing(id, decl);
        }
        self.current_owner = None;
        for &(id, decl, file_index, owner) in pending_methods {
            self.current_file = file_index;
            self.current_owner = Some(owner);
            self.check_member_access_contract(id, decl, owner);
            self.check_override_rules(id, decl, owner);
        }
        self.current_owner = None;
        for &(id, decl, file_index) in pending_classes {
            self.current_file = file_index;
            self.current_owner = Some(Owner::Class(id));
            self.check_interface_implementation(id, decl);
        }
        for &(id, decl, file_index) in pending_structs {
            self.current_file = file_index;
            self.current_owner = Some(Owner::Struct(id));
            self.check_value_interface_implementation(Owner::Struct(id), decl.span);
        }
        for &(id, decl, file_index) in pending_enums {
            self.current_file = file_index;
            self.current_owner = Some(Owner::Enum(id));
            self.check_value_interface_implementation(Owner::Enum(id), decl.span);
        }
        self.current_owner = None;
    }

    /// A class may not directly or indirectly inherit from itself.
    fn check_inheritance_cycle(&mut self, id: ClassId, decl: &ast::ClassDecl) {
        let mut seen = vec![id];
        let mut current = id;
        while let Some(base_ty) = self.classes[current].base_class {
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
        let Some(base_ty) = self.classes[id].base_class else {
            return;
        };
        let Type::Class(base_application) = self.types[base_ty] else {
            unreachable!("resolved class bases are class applications")
        };
        let base = self.class_applications[base_application].template;
        for &field in &self.classes[id].fields.clone() {
            let field = self.class_fields[field].clone();
            let base_application = self.classes[base].self_application;
            let receiver_ty =
                self.class_applications[self.classes[id].self_application].canonical_type;
            if let Some((declaring_application, _, _, _)) = self
                .find_accessible_class_application_field(base_application, &field.name, receiver_ty)
            {
                let declaring = self.class_applications[declaring_application].template;
                let base_name = self.classes[declaring].name.clone();
                self.error(
                    field.span,
                    format!(
                        "property `{}` of class `{}` shadows a property of base class `{base_name}`",
                        field.name, decl.name.text
                    ),
                );
            }
        }
    }
}
