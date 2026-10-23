//! Keep conditional domains even for interfaces with no dispatch slots.

use super::*;

impl Lowerer {
    pub(super) fn imported_interface_domains(
        &mut self,
        ty: hir::TypeId,
        declaration: &hir::ImportedNominalDeclaration,
        roots: &[hir::TypeId],
    ) -> Vec<(hir::TypeId, Option<hir::ConditionalInterfaceApplication>)> {
        let mut domains = Vec::new();
        for root in roots {
            self.append_imported_interface_domain(*root, None, &mut domains);
        }
        let arguments = self.imported_owner_arguments(ty).to_vec();
        let conditional = self
            .declaration_conditional_interfaces(declaration.owner())
            .to_vec();
        for implementation in conditional {
            let interface = self.instantiate_ty(implementation.interface, &arguments);
            let condition = hir::ConditionalInterfaceApplication {
                declaration: implementation.id(),
                owner: declaration.owner(),
                arguments: arguments.clone(),
            };
            self.append_imported_interface_domain(interface, Some(condition), &mut domains);
        }
        if let hir::Type::Class(class) = self.types[ty] {
            let class = self.class_applications[class].clone();
            if let Some(base) = self.class_definition(class.template).base_class {
                let base = self.instantiate_ty(base, &class.arguments);
                let hir::Type::Class(base) = self.types[base] else {
                    unreachable!("checked class parents are classes")
                };
                let base = self.class_applications[base].clone();
                let inherited = self
                    .class_definition(base.template)
                    .interface_implementations
                    .clone();
                for implementation in inherited {
                    let Some(mut condition) = implementation.condition else {
                        continue;
                    };
                    let interface = self.instantiate_ty(implementation.interface, &base.arguments);
                    condition.arguments = condition
                        .arguments
                        .into_iter()
                        .map(|argument| self.instantiate_ty(argument, &base.arguments))
                        .collect();
                    self.append_imported_interface_domain(interface, Some(condition), &mut domains);
                }
            }
        }
        domains
    }

    fn append_imported_interface_domain(
        &mut self,
        root: hir::TypeId,
        condition: Option<hir::ConditionalInterfaceApplication>,
        domains: &mut Vec<(hir::TypeId, Option<hir::ConditionalInterfaceApplication>)>,
    ) {
        let mut interfaces = Vec::new();
        self.append_interface_closure(root, &mut interfaces);
        for interface in interfaces {
            let domain = (interface, condition.clone());
            if !domains.contains(&domain) {
                domains.push(domain);
            }
        }
    }
}
