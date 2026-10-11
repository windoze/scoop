//! Inferred types retain local identities only inside their lexical scopes.

use super::*;

impl Lowerer {
    pub(crate) fn local_type_root(&self, mut owner: Owner) -> Option<Owner> {
        loop {
            if self.local_types.contains_key(&owner) {
                return Some(owner);
            }
            owner = self.nominal_parent(owner)?;
        }
    }

    pub(crate) fn check_local_type_result(
        &mut self,
        ty: hir::TypeId,
        span: ast::Span,
        scopes: &Scopes,
        allow_anonymous: bool,
    ) -> bool {
        let mut dependencies = Vec::new();
        self.collect_type_dependencies(ty, &mut dependencies);
        for (ty, _) in dependencies {
            let Some(owner) = self
                .nominal_application(ty)
                .and_then(|application| self.nominal_owners.get(&application.template).copied())
                .and_then(|owner| self.local_type_root(owner))
            else {
                continue;
            };
            let local = &self.local_types[&owner];
            if local.anonymous {
                if allow_anonymous {
                    continue;
                }
                self.error(span, "an inferred declaration signature cannot expose an anonymous object type; specify a visible base type".into());
                return false;
            }
            if !scopes.contains_nominal(owner) {
                self.error(span, format!("local type `{}` cannot escape its lexical scope through an inferred result", owner.describe_name(self)));
                return false;
            }
        }
        true
    }

    pub(crate) fn check_local_declaration_result(
        &mut self,
        owner: Option<Owner>,
        ty: hir::TypeId,
        span: ast::Span,
    ) -> bool {
        let scopes = owner
            .and_then(|owner| self.local_type_root(owner))
            .map_or_else(Scopes::new, |owner| self.local_types[&owner].scopes.clone());
        self.check_local_type_result(ty, span, &scopes, false)
    }
}
