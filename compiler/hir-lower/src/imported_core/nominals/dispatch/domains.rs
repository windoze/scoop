//! Keep conditional domains even for interfaces with no dispatch slots.

use super::*;

impl Lowerer {
    pub(super) fn imported_interface_domains(
        &mut self,
        ty: hir::TypeId,
        declaration: &hir::ImportedNominalDeclaration,
        roots: &[hir::TypeId],
    ) -> Result<
        Vec<(hir::TypeId, Option<hir::ConditionalInterfaceApplication>)>,
        ImportedSignatureTypeError,
    > {
        let mut domains = Vec::new();
        for root in roots {
            self.append_imported_interface_domain(*root, None, &mut domains);
        }
        for conformance in declaration
            .interface
            .declaration_details()
            .conditional_conformances()
        {
            let interface = self.imported_dispatch_receiver(&conformance.interface, ty)?;
            let owner_type = self.imported_dispatch_receiver(&conformance.condition.owner, ty)?;
            let owner = self
                .imported_nominal_owner(owner_type)
                .ok_or(ImportedSignatureTypeError::Structural)?;
            if !self
                .declaration_conditional_interfaces(owner)
                .iter()
                .any(|block| block.id() == conformance.condition.declaration)
            {
                return Err(ImportedSignatureTypeError::Structural);
            }
            let condition = hir::ConditionalInterfaceApplication {
                declaration: conformance.condition.declaration,
                owner,
                arguments: self.imported_owner_arguments(owner_type).to_vec(),
            };
            domains.push((interface, Some(condition)));
        }
        Ok(domains)
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
