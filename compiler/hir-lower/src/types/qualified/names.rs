use super::*;
use crate::Owner;
use crate::namespace::TopLevelTypeTarget;

mod bindings;

/// A declaration name does not require an application until it is used as a type.
#[derive(Clone, Copy)]
pub(crate) enum ResolvedTypeName {
    Nominal(hir::SourceNominalId),
    Applied(TypeId),
}

impl Lowerer {
    pub(crate) fn resolve_type_name_path(
        &mut self,
        path: &[ast::Ident],
        supplied_type_arguments: bool,
    ) -> Result<Option<ResolvedTypeName>, ()> {
        let Some((binding, start)) = self.type_name_path_head(path)? else {
            return Ok(None);
        };
        let mut target = self
            .resolve_type_name_binding(
                binding,
                &path[start],
                supplied_type_arguments && start + 1 == path.len(),
            )
            .ok_or(())?;
        for (index, name) in path.iter().enumerate().skip(start + 1) {
            let Some(owner) = self.type_name_nominal_owner(target) else {
                let previous = &path[index - 1];
                self.error(
                    previous.span,
                    format!(
                        "typealias `{}` does not name a type qualifier",
                        previous.text,
                    ),
                );
                return Err(());
            };
            let Some(nested) = self.resolve_nested_type_name(
                owner,
                name,
                supplied_type_arguments && index + 1 == path.len(),
            )?
            else {
                let prefix = path[..index]
                    .iter()
                    .map(|name| name.text.as_str())
                    .collect::<Vec<_>>()
                    .join(".");
                self.error(
                    name.span,
                    format!(
                        "type `{prefix}` has no accessible nested type `{}`",
                        name.text,
                    ),
                );
                return Err(());
            };
            target = if let ResolvedTypeName::Applied(host) = target
                && let ResolvedTypeName::Nominal(nominal) = nested
                && self.nominal_is_companion(nominal)
            {
                if supplied_type_arguments && index + 1 == path.len() {
                    self.error(
                        name.span,
                        "companion type arguments belong to its host".into(),
                    );
                    return Err(());
                }
                ResolvedTypeName::Applied(
                    self.apply_companion_type(host, nominal, name.span)
                        .ok_or(())?,
                )
            } else {
                nested
            };
        }
        if !self.resolved_type_name_is_accessible(target) {
            let first = path.first().expect("a type path has an initial name");
            let last = path.last().expect("a type path has a final name");
            let name = path
                .iter()
                .map(|part| part.text.as_str())
                .collect::<Vec<_>>()
                .join(".");
            self.error(
                ast::Span::new(first.span.start, last.span.end),
                format!("type `{name}` is not accessible from this source location"),
            );
            return Err(());
        }
        Ok(Some(target))
    }

    fn type_name_path_head(
        &mut self,
        path: &[ast::Ident],
    ) -> Result<Option<(TypeLookupTarget, usize)>, ()> {
        let first = path.first().expect("a type name path is non-empty");
        if let Some(package) = self.qualified_package_prefix(path) {
            let length = package.consumed();
            let package_name = package.name();
            let Some(name) = path.get(length) else {
                self.error(
                    path.last().expect("a package path is non-empty").span,
                    format!("`{package_name}` names a package, not a type",),
                );
                return Err(());
            };
            let lookup = self.lookup_package_type(&package, &name.text);
            let Some(binding) = self.commit_type_lookup(name, lookup)? else {
                self.error(
                    name.span,
                    format!(
                        "package `{package_name}` has no accessible type `{}`",
                        name.text,
                    ),
                );
                return Err(());
            };
            return Ok(Some((binding, length)));
        }
        if let Some(target) = self.lexical_nested_nominal_target(&first.text) {
            return Ok(Some((
                TypeLookupTarget::Current(TopLevelTypeTarget::Nominal(target)),
                0,
            )));
        }
        Ok(self.resolve_type_lookup(first)?.map(|binding| (binding, 0)))
    }

    pub(in crate::types) fn current_nominal_name_target(
        &self,
        identity: hir::SourceNominalId,
    ) -> Option<NominalTarget> {
        self.nominal_owners
            .get(&identity)
            .map(|owner| match *owner {
                Owner::Class(id) => NominalTarget::Class(id),
                Owner::Interface(id) => NominalTarget::Interface(id),
                Owner::Struct(id) => NominalTarget::Struct(id),
                Owner::Enum(id) => NominalTarget::Enum(id),
                Owner::Object(id) => NominalTarget::Object(id),
            })
    }

    pub(in crate::types) fn type_name_nominal_owner(
        &self,
        target: ResolvedTypeName,
    ) -> Option<hir::SourceNominalId> {
        match target {
            ResolvedTypeName::Nominal(owner) => Some(owner),
            ResolvedTypeName::Applied(ty) => self
                .nominal_target_for_type(ty)
                .map(|target| self.nominal_identity(target.owner()).declaration_id())
                .or_else(|| self.imported_nominal_owner(ty)),
        }
    }

    fn resolved_type_name_is_accessible(&self, target: ResolvedTypeName) -> bool {
        let identity = match target {
            ResolvedTypeName::Nominal(identity) => identity,
            ResolvedTypeName::Applied(ty) => return self.nominal_is_accessible(ty),
        };
        if let Some(owner) = self.nominal_owners.get(&identity) {
            return self.access_domain_allows(self.owner_lookup_domain(*owner));
        }
        let declaration = self
            .dependencies
            .as_ref()
            .and_then(|dependencies| dependencies.nominal_declaration(identity))
            .expect("a resolved nominal name retains its declaration");
        self.access_domain_allows(&self.imported_nominal_access_domain(declaration))
    }
}
