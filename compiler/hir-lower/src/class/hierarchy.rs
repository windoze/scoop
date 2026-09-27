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
        let reaches =
            self.interfaces[current]
                .parents
                .iter()
                .any(|parent| match self.types[*parent] {
                    Type::Interface(application) => {
                        let parent = self.interface_applications[application].template;
                        parent == target || self.interface_reaches(parent, target, visiting)
                    }
                    Type::ImportedInterface(_) => false,
                    _ => unreachable!("resolved interface parents are interface types"),
                });
        visiting.pop();
        reaches
    }

    // --- member lookup helpers ---

    pub(crate) fn direct_base_class(&self, class: ClassId) -> Option<ClassId> {
        let base = self.classes[class].base_class.as_ref()?;
        let application = match self.types[*base] {
            Type::Class(application) => application,
            Type::ImportedClass(_) => return None,
            _ => unreachable!("resolved class bases have class types"),
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
            let base_application = match self.types[base] {
                Type::Class(application) => application,
                Type::ImportedClass(_) => break,
                _ => unreachable!("resolved class bases have class types"),
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
                        crate::CallableCandidate::method(
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
        if let Some(&property_id) = self.classes[class]
            .properties
            .iter()
            .find(|property| self.properties[**property].name == name)
        {
            let property = self.properties[property_id].clone();
            let hir::PropertyRepresentation::Stored(stored) = property.representation else {
                return None;
            };
            let hir::PropertyBacking::ClassField {
                field: field_id, ..
            } = stored.backing
            else {
                return None;
            };
            let field_ty = property.ty;
            let ty = self.instantiate_ty(field_ty, &application_value.arguments);
            let mutable = property.capability.setter().is_some();
            return Some((application, field_id, ty, mutable));
        }
        let base = self.classes[class].base_class?;
        let base = self.instantiate_ty(base, &application_value.arguments);
        let base_application = match self.types[base] {
            Type::Class(application) => application,
            Type::ImportedClass(_) => return None,
            _ => unreachable!("resolved class bases have class types"),
        };
        self.find_class_application_field(base_application, name)
    }

    pub(crate) fn find_accessible_class_application_property(
        &mut self,
        mut application: hir::ClassApplicationId,
        name: &str,
        receiver_ty: TypeId,
    ) -> Option<(hir::ClassApplicationId, hir::PropertyId, TypeId)> {
        let mut seen = Vec::new();
        loop {
            if seen.contains(&application) {
                return None;
            }
            seen.push(application);
            let application_value = self.class_applications[application].clone();
            let class = application_value.template;
            if let Some(&property) = self.classes[class]
                .properties
                .iter()
                .find(|property| self.properties[**property].name == name)
                && self.property_is_accessible(property, Some(receiver_ty))
            {
                let ty =
                    self.instantiate_ty(self.properties[property].ty, &application_value.arguments);
                return Some((application, property, ty));
            }
            let base = self.classes[class].base_class?;
            let base = self.instantiate_ty(base, &application_value.arguments);
            let base_application = match self.types[base] {
                Type::Class(application) => application,
                Type::ImportedClass(_) => return None,
                _ => unreachable!("resolved class bases have class types"),
            };
            application = base_application;
        }
    }
}
