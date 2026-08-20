//! A default carries its lexical type declarations along with its body.

use super::*;

impl Lowerer {
    pub(in crate::defaults) fn default_has_local_type(&self, ty: hir::TypeId) -> bool {
        let Some(application) = self.nominal_application(ty) else {
            return false;
        };
        if let Some(&owner) = self.nominal_owners.get(&application.template) {
            return self.local_type_root(owner).is_some();
        }
        self.dependencies
            .as_ref()
            .and_then(|dependencies| dependencies.nominal_declaration(application.template))
            .is_some_and(|declaration| {
                matches!(
                    declaration.identity.declaration().scope(),
                    scoop_identity::DeclarationScope::LexicalScoped { .. }
                )
            })
    }

    pub(in crate::defaults) fn default_type_access_domain(
        &self,
        ty: hir::TypeId,
    ) -> hir::AccessDomain {
        let mut dependencies = Vec::new();
        self.collect_type_dependencies(ty, &mut dependencies);
        dependencies
            .into_iter()
            .filter(|(ty, _)| !self.default_has_local_type(*ty))
            .fold(hir::AccessDomain::universal(), |domain, (_, dependency)| {
                domain.intersect(&dependency)
            })
    }
}
