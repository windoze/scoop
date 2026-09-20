use super::*;

impl Lowerer {
    pub(super) fn protected_scope_classes(
        &self,
        owner: Owner,
    ) -> impl Iterator<Item = hir::ClassId> + '_ {
        std::iter::successors(Some(owner), |owner| self.owner_parent(*owner)).filter_map(|owner| {
            match owner {
                Owner::Class(class) => Some(class),
                Owner::Object(object) => Some(self.objects[object].backing_class),
                Owner::Interface(_) | Owner::Struct(_) | Owner::Enum(_) => None,
            }
        })
    }

    pub(crate) fn protected_access_class(&self, base: hir::ClassId) -> Option<hir::ClassId> {
        self.protected_scope_classes(self.current_owner?)
            .find(|class| self.class_is_same_or_subclass_of(*class, base))
    }

    pub(super) fn lexical_scope_implies_subclass(
        &self,
        owner: hir::VisibilityOwner,
        base: hir::ClassId,
    ) -> bool {
        let owner = match owner {
            hir::VisibilityOwner::Class(id) => Owner::Class(id),
            hir::VisibilityOwner::Interface(id) => Owner::Interface(id),
            hir::VisibilityOwner::Struct(id) => Owner::Struct(id),
            hir::VisibilityOwner::Enum(id) => Owner::Enum(id),
            hir::VisibilityOwner::Object(id) => Owner::Object(id),
        };
        self.protected_scope_classes(owner)
            .any(|class| self.class_is_same_or_subclass_of(class, base))
    }

    pub(crate) fn class_is_same_or_subclass_of(
        &self,
        mut class: hir::ClassId,
        base: hir::ClassId,
    ) -> bool {
        let mut seen = Vec::new();
        loop {
            if class == base {
                return true;
            }
            if seen.contains(&class) {
                return false;
            }
            seen.push(class);
            let Some(base_ty) = self.classes[class].base_class else {
                return false;
            };
            let hir::Type::Class(application) = self.types[base_ty] else {
                unreachable!("resolved class bases are class applications")
            };
            class = self.class_applications[application].template;
        }
    }

    pub(crate) fn receiver_class(&self, ty: hir::TypeId) -> Option<hir::ClassId> {
        match self.types[ty] {
            hir::Type::Class(application) => Some(self.class_applications[application].template),
            _ => None,
        }
    }

    pub(crate) fn access_domain_allows(
        &self,
        domain: &hir::AccessDomain,
        explicit_receiver: Option<hir::TypeId>,
    ) -> bool {
        if domain.is_empty() {
            return false;
        }
        let site = self.visibility_file(self.current_file);
        domain
            .constraints()
            .iter()
            .all(|constraint| match constraint {
                hir::AccessConstraint::Cone(cone) => site.cone() == *cone,
                hir::AccessConstraint::File(file) => site == *file,
                hir::AccessConstraint::LexicalOwner(owner) => self
                    .current_owner
                    .is_some_and(|current| self.lexical_owner_contains(current, *owner)),
                hir::AccessConstraint::SubclassesOf(base) => {
                    self.current_owner.is_some_and(|owner| {
                        self.protected_scope_classes(owner).any(|current| {
                            self.class_is_same_or_subclass_of(current, *base)
                                && explicit_receiver.is_none_or(|receiver| {
                                    self.receiver_class(receiver).is_some_and(|receiver| {
                                        self.class_is_same_or_subclass_of(receiver, current)
                                    })
                                })
                        })
                    })
                }
            })
    }
}
