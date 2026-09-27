use super::*;

impl Lowerer {
    pub(crate) fn imported_callable_slot_domain(
        &self,
        callable: &hir::CallableDeclarationRecordV1,
    ) -> Option<hir::SlotContractDomain> {
        use hir::ImportedCallableSource;
        let slot = *callable.slot_relations().values().first()?;
        let record = self
            .virtual_method_roots
            .values()
            .find_map(|root| match root {
                crate::persistent_dispatch::VirtualMethodRoot::Imported(record)
                    if record.id() == slot =>
                {
                    Some(record)
                }
                _ => None,
            })
            .expect("resolved dependency virtual families retain their declaration records");
        let target = match record.key().owner() {
            scoop_identity::DispatchDeclarationOwner::Function(id) => {
                scoop_identity::CallableTemplateOrigin::Function(id)
            }
            scoop_identity::DispatchDeclarationOwner::Accessor(id) => {
                scoop_identity::CallableTemplateOrigin::Accessor(id)
            }
        };
        let root = self
            .dependencies
            .as_ref()
            .expect("dependency slot has a catalog")
            .callable_declaration(target)
            .expect("dependency slot root has its actual declaration");
        let domain = match root.interface().declared_visibility() {
            hir::DeclaredVisibilityV1::Public => hir::AccessDomain::universal(),
            hir::DeclaredVisibilityV1::Protected => {
                let hir::PublicDeclarationOwnerV1::Nominal(hir::SourceNominalId::Concrete(owner)) =
                    root.interface().owner()
                else {
                    unreachable!("protected dependency slots belong to classes")
                };
                hir::AccessDomain::from_constraints([hir::AccessConstraint::ImportedSubclassesOf(
                    owner,
                )])
            }
            hir::DeclaredVisibilityV1::Internal | hir::DeclaredVisibilityV1::Private => {
                hir::AccessDomain::empty()
            }
        };
        Some(hir::SlotContractDomain(domain))
    }

    pub(crate) fn imported_callable_is_accessible(
        &self,
        declaration: &hir::CallableDeclarationRecordV1,
        receiver: Option<hir::TypeId>,
    ) -> bool {
        match declaration.declared_visibility() {
            hir::DeclaredVisibilityV1::Public => true,
            hir::DeclaredVisibilityV1::Internal | hir::DeclaredVisibilityV1::Private => false,
            hir::DeclaredVisibilityV1::Protected => {
                let hir::PublicDeclarationOwnerV1::Nominal(hir::SourceNominalId::Concrete(base)) =
                    declaration.owner()
                else {
                    return false;
                };
                let Some(scope) = self.current_owner else {
                    return false;
                };
                self.protected_scope_classes(scope).any(|class| {
                    self.class_inherits_imported(class, base)
                        && receiver.is_none_or(|receiver| {
                            self.receiver_class(receiver).is_some_and(|receiver| {
                                self.class_is_same_or_subclass_of(receiver, class)
                            })
                        })
                })
            }
        }
    }

    pub(crate) fn class_inherits_imported(
        &self,
        class: hir::ClassId,
        base: scoop_identity::PersistentTypeId,
    ) -> bool {
        let mut current = self.classes[class].base_class;
        let mut seen = Vec::new();
        while let Some(ty) = current {
            if seen.contains(&ty) {
                return false;
            }
            seen.push(ty);
            current = match &self.types[ty] {
                hir::Type::Class(application) => {
                    self.classes[self.class_applications[*application].template].base_class
                }
                hir::Type::ImportedClass(class) => {
                    if class.declaration.identity.id() == base {
                        return true;
                    }
                    class.base_class
                }
                _ => unreachable!("a resolved class base has a class type"),
            };
        }
        false
    }
}
