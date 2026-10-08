//! Preserve dependency nominal dispatch targets in typed HIR.

use super::*;
use hir::ImportedCallableSource;

mod selections;

impl Lowerer {
    pub(super) fn resolve_imported_class_dispatch(
        &mut self,
        ty: hir::TypeId,
        declaration: &hir::ImportedNominalDeclaration,
    ) -> Result<
        (
            Vec<hir::ImportedVirtualMethod>,
            Vec<hir::InterfaceImplementation>,
        ),
        ImportedSignatureTypeError,
    > {
        let hir::Type::Class(application) = self.types[ty] else {
            return Err(ImportedSignatureTypeError::Structural);
        };
        let class = self
            .class_definition(self.class_applications[application].template)
            .clone();
        let selections = declaration
            .interface
            .declaration_details()
            .dispatch_selections();
        let mut slots = match class.base_class.map(|base| &self.types[base]) {
            Some(hir::Type::Class(base)) => self.loaded_class_definitions
                [&self.class_applications[*base].template]
                .virtual_methods
                .iter()
                .map(|method| method.slot)
                .collect::<Vec<_>>(),
            Some(_) => return Err(ImportedSignatureTypeError::Structural),
            None => Vec::new(),
        };
        for slot in declaration
            .interface
            .declaration_details()
            .dispatch_order()
            .declared_slots()
        {
            if !slots.contains(&slot) {
                slots.push(slot);
            }
        }
        let mut virtual_methods = Vec::new();
        for (position, slot) in slots.iter().enumerate() {
            let family = if let Some(record) = declaration
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
                .get(&hir::NominalDispatchSelectionRoleV1::ClassVtable, *slot)
                .ok_or(ImportedSignatureTypeError::Structural)?;
            let callable = self.select_imported_dispatch_target(selection, ty)?;
            if let hir::ImportedDispatchCallable::External(callee) = callable {
                let reference = self.imported_dependency_callables[callee].reference();
                let selected = self
                    .dependencies
                    .as_ref()
                    .and_then(|dependencies| dependencies.resolve_callable(reference))
                    .expect("a virtual implementation retains its selected declaration");
                if selected.interface().modality() != hir::CallableModalityV1::Final {
                    let receiver = self.imported_dependency_callables[callee].receiver();
                    self.intern_imported_dependency_callable_use(
                        reference,
                        hir::ImportedDependencyDispatch::Virtual {
                            slot: u32::try_from(position)
                                .map_err(|_| ImportedSignatureTypeError::Structural)?,
                        },
                        receiver,
                    );
                }
            }
            virtual_methods.push(hir::ImportedVirtualMethod {
                slot: *slot,
                family,
                callable,
            });
        }

        let mut interfaces = Vec::new();
        if let Some(base) = class.base_class {
            let hir::Type::Class(base) = self.types[base] else {
                return Err(ImportedSignatureTypeError::Structural);
            };
            let base = self.class_applications[base].clone();
            let inherited = self
                .class_definition(base.template)
                .interface_implementations
                .iter()
                .map(|implementation| implementation.interface)
                .collect::<Vec<_>>();
            interfaces.extend(
                inherited
                    .into_iter()
                    .map(|interface| self.instantiate_ty(interface, &base.arguments)),
            );
        }
        interfaces.extend_from_slice(&class.interfaces);
        let implementations =
            self.resolve_imported_interface_implementations(ty, declaration, &interfaces)?;
        Ok((virtual_methods, implementations))
    }

    pub(in crate::imported_core) fn resolve_imported_interface_implementations(
        &mut self,
        ty: hir::TypeId,
        declaration: &hir::ImportedNominalDeclaration,
        roots: &[hir::TypeId],
    ) -> Result<Vec<hir::InterfaceImplementation>, ImportedSignatureTypeError> {
        let selections = self.imported_interface_dispatch_selections(ty, declaration)?;
        let mut interfaces = Vec::new();
        for root in roots {
            self.append_interface_closure(*root, &mut interfaces);
        }
        let mut implementations = Vec::new();
        for interface_ty in interfaces {
            let Some(interface) = self.dependency_interface_definition(interface_ty).cloned()
            else {
                return Err(ImportedSignatureTypeError::Structural);
            };
            let mut methods = Vec::new();
            for method in &interface.methods {
                let slot = method.slot.id();
                let selection = selections
                    .get(&(interface_ty, slot))
                    .ok_or(ImportedSignatureTypeError::Structural)?;
                if let hir::InheritanceCallableDeclarationV1::DerivedEquality(owner) =
                    selection.selection().declaration()
                {
                    if owner != declaration.owner() {
                        return Err(ImportedSignatureTypeError::Structural);
                    }
                    let target = self
                        .imported_derived_equality(ty)
                        .ok_or(ImportedSignatureTypeError::Structural)?;
                    let hir::MethodCallee::ImportedDerivedEquality { target, .. } =
                        self.imported_equality_callee(target, ty)
                    else {
                        unreachable!()
                    };
                    methods.push(hir::InterfaceMethodImplementation {
                        member: hir::InterfaceMethodReference::Imported {
                            owner: interface_ty,
                            slot,
                        },
                        target: hir::InterfaceImplementationTarget::ImportedDerivedEquality(target),
                    });
                    continue;
                }
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
            implementations.push(hir::InterfaceImplementation {
                interface: interface_ty,
                methods,
            });
        }
        Ok(implementations)
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
            .callable_declaration(
                selection
                    .callable_target()
                    .ok_or(ImportedSignatureTypeError::Structural)?,
            )
            .map_err(|_| ImportedSignatureTypeError::Structural)?;
        let receiver = self.imported_dispatch_receiver(selection.receiver(), receiver)?;
        self.resolve_imported_dispatch_callable(candidate, receiver)
    }

    pub(crate) fn resolve_imported_dispatch_callable(
        &mut self,
        candidate: hir::ImportedCallableDeclaration,
        receiver: hir::TypeId,
    ) -> Result<hir::ImportedDispatchCallable, ImportedSignatureTypeError> {
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
