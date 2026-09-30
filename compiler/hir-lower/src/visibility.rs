use scoop_ast as ast;
use scoop_hir as hir;

use crate::{Lowerer, Owner};

mod access;
mod ancestry;
mod domains;
mod imported;
mod signatures;
mod surface;

pub(crate) enum MemberSlotAccess {
    None,
    Declared,
    Override,
}

impl Lowerer {
    pub(crate) fn visibility_file(&self, file: usize) -> scoop_identity::SourceIdentity {
        self.intrinsic_sources[file].identity.clone()
    }

    pub(crate) fn normalized_visibility(syntax: ast::VisibilitySyntax) -> hir::DeclaredVisibility {
        match syntax {
            ast::VisibilitySyntax::Explicit { visibility, .. } => match visibility {
                ast::DeclaredVisibility::Public => hir::DeclaredVisibility::Public,
                ast::DeclaredVisibility::Internal => hir::DeclaredVisibility::Internal,
                ast::DeclaredVisibility::Private => hir::DeclaredVisibility::Private,
                ast::DeclaredVisibility::Protected => hir::DeclaredVisibility::Protected,
            },
            ast::VisibilitySyntax::Omitted => hir::DeclaredVisibility::Internal,
        }
    }

    pub(crate) fn top_level_domain(
        &self,
        visibility: hir::DeclaredVisibility,
        file: usize,
    ) -> hir::AccessDomain {
        let source = self.visibility_file(file);
        match visibility {
            hir::DeclaredVisibility::Public => hir::AccessDomain::universal(),
            hir::DeclaredVisibility::Internal => {
                hir::AccessDomain::from_constraints([hir::AccessConstraint::Cone(source.cone())])
            }
            hir::DeclaredVisibility::Private => hir::AccessDomain::from_constraints([
                hir::AccessConstraint::Cone(source.cone()),
                hir::AccessConstraint::File(source),
            ]),
            hir::DeclaredVisibility::Protected => {
                unreachable!("top-level protected is rejected before domain construction")
            }
        }
    }

    pub(crate) fn top_level_access(
        &mut self,
        syntax: ast::VisibilitySyntax,
        span: ast::Span,
        declaration_kind: &str,
        file: usize,
    ) -> hir::DeclarationAccess {
        let mut declared = Self::normalized_visibility(syntax);
        if declared == hir::DeclaredVisibility::Protected {
            self.error(
                span,
                format!("top-level {declaration_kind} cannot be protected"),
            );
            declared = hir::DeclaredVisibility::Internal;
        }
        hir::DeclarationAccess {
            declared,
            lookup: hir::EffectiveLookupDomain(self.top_level_domain(declared, file)),
            slot: None,
        }
    }

    pub(crate) fn nominal_access(
        &mut self,
        syntax: ast::VisibilitySyntax,
        span: ast::Span,
        declaration_kind: &str,
        file: usize,
    ) -> hir::NominalAccess {
        let direct = self.top_level_access(syntax, span, declaration_kind, file);
        hir::NominalAccess {
            declared: direct.declared,
            inheritance: hir::InheritanceDomain(direct.lookup.0.clone()),
            lookup: direct.lookup,
        }
    }

    pub(crate) fn nested_nominal_access(
        &mut self,
        syntax: ast::VisibilitySyntax,
        span: ast::Span,
        declaration_kind: &str,
        owner: Owner,
        file: usize,
    ) -> hir::NominalAccess {
        let mut declared = Self::normalized_visibility(syntax);
        if declared == hir::DeclaredVisibility::Protected && !matches!(owner, Owner::Class(_)) {
            self.error(
                span,
                format!(
                    "nested {declaration_kind} on {} cannot be protected",
                    owner.describe(self)
                ),
            );
            declared = hir::DeclaredVisibility::Internal;
        }
        let declared_domain = self.member_declared_domain(declared, owner, file);
        let effective = declared_domain.intersect(self.owner_lookup_domain(owner));
        hir::NominalAccess {
            declared,
            inheritance: hir::InheritanceDomain(effective.clone()),
            lookup: hir::EffectiveLookupDomain(effective),
        }
    }

    fn owner_visibility(&self, owner: Owner) -> hir::VisibilityOwner {
        match owner {
            Owner::Class(id) => hir::VisibilityOwner::Class(id),
            Owner::Interface(id) => hir::VisibilityOwner::Interface(id),
            Owner::Struct(id) => hir::VisibilityOwner::Struct(id),
            Owner::Enum(id) => hir::VisibilityOwner::Enum(id),
            Owner::Object(id) => hir::VisibilityOwner::Object(id),
        }
    }

    pub(crate) fn owner_lookup_domain(&self, owner: Owner) -> &hir::AccessDomain {
        match owner {
            Owner::Class(id) => &self.classes[id].access.lookup.0,
            Owner::Interface(id) => &self.interfaces[id].access.lookup.0,
            Owner::Struct(id) => &self.structs[id].access.lookup.0,
            Owner::Enum(id) => &self.enums[id].access.lookup.0,
            Owner::Object(id) => &self.objects[id].access.lookup.0,
        }
    }

    pub(crate) fn member_declared_domain(
        &self,
        visibility: hir::DeclaredVisibility,
        owner: Owner,
        file: usize,
    ) -> hir::AccessDomain {
        let source = self.visibility_file(file);
        match visibility {
            hir::DeclaredVisibility::Public => hir::AccessDomain::universal(),
            hir::DeclaredVisibility::Internal => {
                hir::AccessDomain::from_constraints([hir::AccessConstraint::Cone(source.cone())])
            }
            hir::DeclaredVisibility::Private => {
                hir::AccessDomain::from_constraints([hir::AccessConstraint::LexicalOwner(
                    self.owner_visibility(owner),
                )])
            }
            hir::DeclaredVisibility::Protected => {
                let Owner::Class(class) = owner else {
                    unreachable!("non-class protected is rejected before domain construction")
                };
                hir::AccessDomain::from_constraints([hir::AccessConstraint::SubclassesOf(
                    self.nominal_identity(Owner::Class(class)).declaration_id(),
                )])
            }
        }
    }

    pub(crate) fn member_access(
        &mut self,
        syntax: ast::VisibilitySyntax,
        span: ast::Span,
        declaration_kind: &str,
        owner: Owner,
        file: usize,
        slot_access: MemberSlotAccess,
    ) -> hir::DeclarationAccess {
        let mut declared = Self::normalized_visibility(syntax);
        if declared == hir::DeclaredVisibility::Protected && !matches!(owner, Owner::Class(_)) {
            self.error(
                span,
                format!(
                    "{declaration_kind} on {} cannot be protected",
                    owner.describe(self)
                ),
            );
            declared = hir::DeclaredVisibility::Internal;
        }
        let declared_domain = self.member_declared_domain(declared, owner, file);
        let effective = declared_domain.intersect(self.owner_lookup_domain(owner));
        let slot = match slot_access {
            MemberSlotAccess::None => None,
            MemberSlotAccess::Declared => Some(hir::SlotContractDomain(effective.clone())),
            MemberSlotAccess::Override => Some(hir::SlotContractDomain(declared_domain)),
        };
        hir::DeclarationAccess {
            declared,
            lookup: hir::EffectiveLookupDomain(effective),
            slot,
        }
    }

    pub(crate) fn fixed_representation_access(&self, owner: Owner) -> hir::DeclarationAccess {
        hir::DeclarationAccess {
            declared: hir::DeclaredVisibility::Public,
            lookup: hir::EffectiveLookupDomain(self.owner_lookup_domain(owner).clone()),
            slot: None,
        }
    }

    pub(crate) fn local_declaration_access(&self) -> hir::DeclarationAccess {
        let source = self.visibility_file(self.current_file);
        hir::DeclarationAccess {
            declared: hir::DeclaredVisibility::Private,
            lookup: hir::EffectiveLookupDomain(hir::AccessDomain::from_constraints([
                hir::AccessConstraint::Cone(source.cone()),
                hir::AccessConstraint::File(source),
            ])),
            slot: None,
        }
    }

    fn owner_parent(&self, owner: Owner) -> Option<Owner> {
        let parent = match owner {
            Owner::Class(id) => self.classes[id].owner,
            Owner::Interface(id) => self.interfaces[id].owner,
            Owner::Struct(id) => self.structs[id].owner,
            Owner::Enum(id) => self.enums[id].owner,
            Owner::Object(id) => self.objects[id].owner,
        }?;
        Some(Owner::from_nominal_owner(parent))
    }

    fn lexical_owner_contains(&self, mut current: Owner, required: hir::VisibilityOwner) -> bool {
        loop {
            if self.owner_visibility(current) == required {
                return true;
            }
            let Some(parent) = self.owner_parent(current) else {
                return false;
            };
            current = parent;
        }
    }

    fn visibility_owner_is_within(
        &self,
        current: hir::VisibilityOwner,
        required: hir::VisibilityOwner,
    ) -> bool {
        let current = match current {
            hir::VisibilityOwner::Class(id) => Owner::Class(id),
            hir::VisibilityOwner::Interface(id) => Owner::Interface(id),
            hir::VisibilityOwner::Struct(id) => Owner::Struct(id),
            hir::VisibilityOwner::Enum(id) => Owner::Enum(id),
            hir::VisibilityOwner::Object(id) => Owner::Object(id),
        };
        self.lexical_owner_contains(current, required)
    }

    fn owner_definition_file(&self, owner: hir::VisibilityOwner) -> scoop_identity::SourceIdentity {
        let file = match owner {
            hir::VisibilityOwner::Class(id) => self.class_files[&id],
            hir::VisibilityOwner::Interface(id) => self.interface_files[&id],
            hir::VisibilityOwner::Struct(id) => self.struct_files[&id],
            hir::VisibilityOwner::Enum(id) => self.enum_files[&id],
            hir::VisibilityOwner::Object(id) => self.object_files[&id],
        };
        self.visibility_file(file)
    }

    fn constraint_implies(
        &self,
        narrower: &hir::AccessConstraint,
        wider: &hir::AccessConstraint,
    ) -> bool {
        if narrower == wider {
            return true;
        }
        match (narrower, wider) {
            (hir::AccessConstraint::File(source), hir::AccessConstraint::Cone(cone)) => {
                source.cone() == *cone
            }
            (hir::AccessConstraint::LexicalOwner(owner), hir::AccessConstraint::Cone(cone)) => {
                self.owner_definition_file(*owner).cone() == *cone
            }
            (hir::AccessConstraint::LexicalOwner(owner), hir::AccessConstraint::File(file)) => {
                self.owner_definition_file(*owner) == *file
            }
            (
                hir::AccessConstraint::LexicalOwner(current),
                hir::AccessConstraint::LexicalOwner(required),
            ) => self.visibility_owner_is_within(*current, *required),
            (
                hir::AccessConstraint::LexicalOwner(owner),
                hir::AccessConstraint::SubclassesOf(base),
            ) => self.lexical_scope_implies_subclass(*owner, *base),
            (
                hir::AccessConstraint::SubclassesOf(derived),
                hir::AccessConstraint::SubclassesOf(base),
            ) => self.class_declaration_is_same_or_subclass_of(*derived, *base),
            _ => false,
        }
    }

    /// Set containment used by override, signature and default-access proofs.
    /// `narrower` is a subset of `wider` when every wider constraint is
    /// implied by at least one constraint of the narrower region.
    pub(crate) fn access_domain_is_subset(
        &self,
        narrower: &hir::AccessDomain,
        wider: &hir::AccessDomain,
    ) -> bool {
        if narrower.is_empty() {
            return true;
        }
        if wider.is_empty() {
            return false;
        }
        wider.constraints().iter().all(|wider_constraint| {
            narrower.constraints().iter().any(|narrower_constraint| {
                self.constraint_implies(narrower_constraint, wider_constraint)
            })
        })
    }
}
