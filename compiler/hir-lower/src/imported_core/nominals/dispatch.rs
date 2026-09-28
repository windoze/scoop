//! Preserve a dependency class's resolved dispatch targets in typed HIR.

use super::*;
use hir::ImportedCallableSource;

impl Lowerer {
    pub(super) fn resolve_imported_class_dispatch(
        &mut self,
        ty: hir::TypeId,
        class: &mut hir::ImportedClassType,
    ) -> Result<(), ImportedSignatureTypeError> {
        let selections = class
            .declaration
            .interface
            .declaration_details()
            .dispatch_selections();
        let mut slots = match class.base_class.map(|base| &self.types[base]) {
            Some(hir::Type::ImportedClass(base)) => base
                .virtual_methods
                .iter()
                .map(|method| method.slot)
                .collect::<Vec<_>>(),
            Some(_) => return Err(ImportedSignatureTypeError::Structural),
            None => Vec::new(),
        };
        for slot in class
            .declaration
            .interface
            .declaration_details()
            .dispatch_order()
            .declared_slots()
        {
            if !slots.contains(&slot) {
                slots.push(slot);
            }
        }
        for slot in &slots {
            let family = if let Some(record) = class
                .declaration
                .dispatch_slots
                .iter()
                .find(|record| record.id() == *slot)
            {
                self.imported_virtual_method(record)
            } else {
                self.imported_virtual_family(*slot)
                    .ok_or(ImportedSignatureTypeError::Structural)?
            };
            let selection = selections
                .records()
                .iter()
                .find(|selection| selection.slot() == *slot)
                .ok_or(ImportedSignatureTypeError::Structural)?;
            let callable = self.select_imported_dispatch_target(selection, ty)?;
            class.virtual_methods.push(hir::ImportedVirtualMethod {
                slot: *slot,
                family,
                callable,
            });
        }

        let mut interfaces = Vec::new();
        if let Some(base) = class.base_class {
            let hir::Type::ImportedClass(base) = &self.types[base] else {
                return Err(ImportedSignatureTypeError::Structural);
            };
            interfaces.extend(
                base.interface_implementations
                    .iter()
                    .map(|implementation| implementation.interface),
            );
        }
        for interface in &class.interfaces {
            self.append_interface_closure(*interface, &mut interfaces);
        }
        for interface_ty in interfaces {
            let hir::Type::ImportedInterface(interface) = self.types[interface_ty].clone() else {
                return Err(ImportedSignatureTypeError::Structural);
            };
            let mut methods = Vec::new();
            for method in &interface.methods {
                let slot = method.slot.id();
                let selection = selections
                    .records()
                    .iter()
                    .find(|selection| selection.slot() == slot)
                    .ok_or(ImportedSignatureTypeError::Structural)?;
                let callable = self.select_imported_dispatch_target(selection, ty)?;
                let hir::PublicDeclarationOwnerV1::Nominal(owner) = method.declaration.owner()
                else {
                    return Err(ImportedSignatureTypeError::Structural);
                };
                let owner = self
                    .imported_member_owner_type(interface_ty, owner)
                    .ok_or(ImportedSignatureTypeError::Structural)?;
                methods.push(hir::InterfaceMethodImplementation {
                    member: hir::InterfaceMethodReference::Imported { owner, slot },
                    target: match (selection.selection(), callable) {
                        (
                            hir::InheritanceSourceSlotSelectionV1::Abstract(_),
                            hir::ImportedDispatchCallable::External(callable),
                        ) => hir::InterfaceImplementationTarget::ImportedAbstract(callable),
                        (_, hir::ImportedDispatchCallable::External(callable)) => {
                            hir::InterfaceImplementationTarget::Imported(callable)
                        }
                        (
                            hir::InheritanceSourceSlotSelectionV1::Abstract(_),
                            hir::ImportedDispatchCallable::Template(application),
                        ) => hir::InterfaceImplementationTarget::ImportedAbstractTemplate(
                            application,
                        ),
                        (_, hir::ImportedDispatchCallable::Template(application)) => {
                            hir::InterfaceImplementationTarget::ImportedTemplate(application)
                        }
                    },
                });
            }
            class
                .interface_implementations
                .push(hir::InterfaceImplementation {
                    interface: interface_ty,
                    methods,
                });
        }
        Ok(())
    }

    fn select_imported_dispatch_target(
        &mut self,
        selection: &hir::NominalDispatchSelectionV1,
        receiver: hir::TypeId,
    ) -> Result<hir::ImportedDispatchCallable, ImportedSignatureTypeError> {
        let candidate = self
            .dependencies
            .as_ref()
            .expect("dependency dispatch retains its declaration catalog")
            .callable_declaration(selection.callable_target())
            .map_err(|_| ImportedSignatureTypeError::Structural)?;
        if let hir::PublicDeclarationOwnerV1::Nominal(
            owner @ hir::SourceNominalId::GenericTemplate(_),
        ) = candidate.interface().owner()
        {
            let owner = self
                .imported_member_owner_type(receiver, owner)
                .ok_or(ImportedSignatureTypeError::Structural)?;
            let template = self
                .request_imported_generic_template(candidate)
                .map_err(|_| ImportedSignatureTypeError::Structural)?;
            let application =
                self.imported_generic_applications
                    .alloc(hir::ImportedGenericCallableApplication {
                        template,
                        arguments: hir::ImportedCallableArguments::Method {
                            owner,
                            method_arguments: Vec::new(),
                        },
                    });
            return Ok(hir::ImportedDispatchCallable::Template(application));
        }
        for parameter in candidate.interface().parameters().parameters() {
            self.imported_signature_type(parameter.value_type())?;
        }
        self.imported_signature_type(candidate.interface().result())?;
        self.select_imported_callable_definition_use(candidate)
            .map(hir::ImportedDispatchCallable::External)
            .map_err(|_| ImportedSignatureTypeError::Structural)
    }
}
