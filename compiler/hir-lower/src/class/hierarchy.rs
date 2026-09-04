use super::*;

impl Lowerer {
    pub(crate) fn check_interface_inheritance_cycles(
        &mut self,
        pending: &[(hir::InterfaceId, &ast::InterfaceDecl, usize)],
    ) {
        for &(interface, declaration, file) in pending {
            self.current_file = file;
            let mut visiting = Vec::new();
            if self.interface_reaches(interface, interface, &mut visiting) {
                self.error(
                    declaration.span,
                    format!(
                        "interface `{}` directly or indirectly inherits from itself",
                        declaration.name.text
                    ),
                );
            }
        }
    }

    fn interface_reaches(
        &self,
        current: hir::InterfaceId,
        target: hir::InterfaceId,
        visiting: &mut Vec<hir::InterfaceId>,
    ) -> bool {
        if visiting.contains(&current) {
            return false;
        }
        visiting.push(current);
        let reaches = self.interfaces[current].parents.iter().any(|parent| {
            let parent = self.interface_applications[*parent].template;
            parent == target || self.interface_reaches(parent, target, visiting)
        });
        visiting.pop();
        reaches
    }

    // --- member lookup helpers ---

    pub(crate) fn direct_base_class(&self, class: ClassId) -> Option<ClassId> {
        let base = self.classes[class].base_class.as_ref()?;
        let Type::Class(application) = self.types[*base] else {
            unreachable!("resolved class bases are class applications")
        };
        Some(self.class_applications[application].template)
    }

    /// Every interface implemented by class `c` or its base classes,
    /// deduplicated, own list first (cycle-safe).
    pub(crate) fn class_interfaces_all(&mut self, c: ClassId) -> Vec<TypeId> {
        self.class_interfaces_for_application(self.classes[c].self_application)
    }

    /// The methods of the base classes of `c`, nearest base first
    /// (cycle-safe).
    pub(super) fn base_chain_methods(&mut self, c: ClassId) -> Vec<crate::CallableCandidate> {
        let mut result = Vec::new();
        let mut current = self.classes[c].self_application;
        let mut seen = vec![current];
        loop {
            let application = self.class_applications[current].clone();
            let Some(base) = self.classes[application.template].base_class else {
                break;
            };
            let base = self.instantiate_ty(base, &application.arguments);
            let Type::Class(base_application) = self.types[base] else {
                unreachable!("class bases are resolved class applications")
            };
            if seen.contains(&base_application) {
                break;
            }
            seen.push(base_application);
            let base = self.class_applications[base_application].clone();
            result.extend(
                self.classes[base.template]
                    .methods
                    .iter()
                    .copied()
                    .map(|function| {
                        crate::CallableCandidate::inheritance_method(
                            function,
                            hir::MethodOwnerApplication::Class(base_application),
                        )
                    }),
            );
            current = base_application;
        }
        result
    }

    /// Field lookup on a complete class application. The declaration/layout
    /// identity remains the declaring `ClassId`, while the returned field type
    /// is fully substituted through every generic base application.
    pub(crate) fn find_class_application_field(
        &mut self,
        application: hir::ClassApplicationId,
        name: &str,
    ) -> Option<(hir::ClassApplicationId, hir::ClassFieldId, TypeId, bool)> {
        let application_value = self.class_applications[application].clone();
        let class = application_value.template;
        if let Some(&field_id) = self.classes[class]
            .fields
            .iter()
            .find(|field| self.class_fields[**field].name == name)
        {
            let field = self.class_fields[field_id].clone();
            let field_ty = field.ty;
            let ty = self.instantiate_ty(field_ty, &application_value.arguments);
            let mutable = field.mutable;
            return Some((application, field_id, ty, mutable));
        }
        let base = self.classes[class].base_class?;
        let base = self.instantiate_ty(base, &application_value.arguments);
        let Type::Class(base_application) = self.types[base] else {
            unreachable!("resolved class bases are class applications")
        };
        self.find_class_application_field(base_application, name)
    }

    /// Name lookup variant used by ordinary source access. Inaccessible
    /// declarations remain observable to diagnostics but do not block a
    /// visible declaration with the same name in a lower inheritance layer.
    pub(crate) fn find_accessible_class_application_field(
        &mut self,
        mut application: hir::ClassApplicationId,
        name: &str,
        receiver_ty: TypeId,
    ) -> Option<(hir::ClassApplicationId, hir::ClassFieldId, TypeId, bool)> {
        let mut seen = Vec::new();
        loop {
            if seen.contains(&application) {
                return None;
            }
            seen.push(application);
            let application_value = self.class_applications[application].clone();
            let class = application_value.template;
            if let Some(&field_id) = self.classes[class]
                .fields
                .iter()
                .find(|field| self.class_fields[**field].name == name)
                && self.class_field_is_accessible(field_id, receiver_ty)
            {
                let field = self.class_fields[field_id].clone();
                let ty = self.instantiate_ty(field.ty, &application_value.arguments);
                return Some((application, field_id, ty, field.mutable));
            }
            let base = self.classes[class].base_class?;
            let base = self.instantiate_ty(base, &application_value.arguments);
            let Type::Class(base_application) = self.types[base] else {
                unreachable!("resolved class bases are class applications")
            };
            application = base_application;
        }
    }
}
