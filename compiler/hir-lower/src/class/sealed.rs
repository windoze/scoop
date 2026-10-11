//! Declaration-side sealed inheritance rules and complete direct families.

use super::*;
use scoop_ast::Span;

impl Lowerer {
    pub(crate) fn sealed_subtypes(&self, ty: TypeId) -> Option<&[hir::SourceNominalId]> {
        match self.types[ty] {
            Type::Class(id) => self
                .class_definition(self.class_applications[id].template)
                .inheritance_family
                .subtypes(),
            Type::Interface(id) => self
                .interface_definition(self.interface_applications[id].template)
                .inheritance_family
                .subtypes(),
            _ => None,
        }
    }

    pub(crate) fn check_sealed_supertype(&mut self, ty: TypeId, span: Span) -> bool {
        if self.sealed_subtypes(ty).is_none() {
            return true;
        }
        let parent = self
            .nominal_application(ty)
            .expect("a sealed type is nominal")
            .template;
        let owner = self
            .current_owner
            .expect("supertypes belong to declarations");
        let child = self.nominal_identity(owner);
        let local = self.local_type_root(owner).is_none()
            && child.source().is_some_and(|source| {
                matches!(
                    source.declaration().name(),
                    scoop_identity::DeclarationName::Named(_)
                ) && source.declaration().owners().owners().iter().all(|owner| {
                    matches!(
                        owner,
                        scoop_identity::DefinitionOwnerAtom::Type(_)
                            | scoop_identity::DefinitionOwnerAtom::GenericType(_)
                    )
                })
            });
        let parent_key = match self.nominal_owners.get(&parent) {
            Some(parent) => self
                .nominal_identity(*parent)
                .source()
                .map(|source| source.declaration()),
            None => self
                .dependencies
                .as_ref()
                .and_then(|dependencies| dependencies.nominal_declaration(parent))
                .map(|declaration| declaration.identity.declaration()),
        }
        .expect("sealed declarations retain source identities");
        let allowed = local
            && child.source().is_some_and(|source| {
                source.declaration().origin() == parent_key.origin()
                    && source.declaration().package() == parent_key.package()
            });
        if !allowed {
            self.error(span, format!("a direct subtype of sealed `{}` must be a named non-local declaration in the same origin Cone and package", self.type_name(ty)));
            return false;
        }
        let child = child.declaration_id();
        let source = self.nominal_owners[&parent];
        let family = match source {
            Owner::Class(id) => &mut self.classes[id].definition.inheritance_family,
            Owner::Interface(id) => &mut self.interfaces[id].definition.inheritance_family,
            _ => unreachable!("only classes and interfaces are sealed"),
        };
        let hir::InheritanceFamily::Sealed(subtypes) = family else {
            unreachable!("the parent was checked sealed")
        };
        if let Err(index) = subtypes.binary_search(&child) {
            subtypes.insert(index, child);
        }
        true
    }

    pub(super) fn class_constructor_visibility(
        &mut self,
        class: ClassId,
        visibility: ast::VisibilitySyntax,
        span: Span,
    ) -> ast::VisibilitySyntax {
        if self.classes[class].inheritance_family.subtypes().is_none() {
            return visibility;
        }
        match visibility {
            ast::VisibilitySyntax::Omitted => ast::VisibilitySyntax::Explicit {
                visibility: ast::DeclaredVisibility::Protected,
                span,
            },
            ast::VisibilitySyntax::Explicit {
                visibility: ast::DeclaredVisibility::Protected | ast::DeclaredVisibility::Private,
                ..
            } => visibility,
            ast::VisibilitySyntax::Explicit { span, .. } => {
                self.error(
                    span,
                    "a sealed class constructor must be protected or private".into(),
                );
                visibility
            }
        }
    }
}
