use super::*;
use crate::aliases::AliasTemplate;
use crate::imports::lookup::TypeLookupTarget;
use crate::namespace::TopLevelTypeTarget;

impl Lowerer {
    pub(in crate::expr) fn expression_alias_template(
        &mut self,
        expression: &ast::Expr,
    ) -> Result<Option<AliasTemplate>, ()> {
        match expression {
            ast::Expr::Var(name) => {
                if self.lexical_or_member_value_blocks_type_qualifier(&name.text) {
                    return Ok(None);
                }
                match self.lookup_expression_qualifier(name) {
                    ExpressionQualifierLookup::Unique(ExpressionQualifierTarget::Type(
                        TopLevelTypeTarget::Alias(id),
                    ))
                    | ExpressionQualifierLookup::Inaccessible(ExpressionQualifierTarget::Type(
                        TopLevelTypeTarget::Alias(id),
                    )) => self.source_alias_template(id, name).map(Some).ok_or(()),
                    ExpressionQualifierLookup::Unique(
                        ExpressionQualifierTarget::DependencyType(hir::ImportedTarget::TypeAlias(
                            _,
                        )),
                    ) => {
                        let Some(TypeLookupTarget::Dependency(binding)) =
                            self.resolve_type_lookup(name)?
                        else {
                            unreachable!("a selected dependency alias retains its type binding");
                        };
                        self.imported_alias_template(&binding, name)
                            .map(Some)
                            .ok_or(())
                    }
                    _ => Ok(None),
                }
            }
            ast::Expr::FieldAccess(access) if access.navigation == ast::Navigation::Direct => {
                let ast::FieldSelector::Name(name) = &access.selector else {
                    return Ok(None);
                };
                self.nested_constructor_alias(&access.receiver, name)
            }
            _ => Ok(None),
        }
    }

    pub(in crate::expr) fn nested_constructor_alias(
        &mut self,
        receiver: &ast::Expr,
        name: &ast::Ident,
    ) -> Result<Option<AliasTemplate>, ()> {
        if let ast::Expr::Var(name) = receiver
            && self.lexical_or_member_value_blocks_type_qualifier(&name.text)
        {
            return Ok(None);
        }
        let (owner, mut host) = if let Some(alias) = self.expression_alias_template(receiver)? {
            let owner = self
                .nominal_target_for_type(alias.target)
                .map(|target| self.nominal_identity(target.owner()).declaration_id())
                .or_else(|| self.imported_nominal_owner(alias.target));
            let Some(owner) = owner else {
                return Ok(None);
            };
            (owner, alias.parameters.is_empty().then_some(alias.target))
        } else if let Some(target) = self.nominal_qualifier_target(receiver) {
            (self.nominal_identity(target.owner()).declaration_id(), None)
        } else if let Some((ty, target)) = self.resolve_applied_qualifier(receiver)? {
            (
                self.nominal_identity(target.owner()).declaration_id(),
                Some(ty),
            )
        } else if let Some(qualifier) = self.resolve_imported_nominal_qualifier(receiver)? {
            (
                self.imported_qualifier_owner(qualifier),
                qualifier.applied(),
            )
        } else {
            return Ok(None);
        };
        let mut alias = if let Some(owner) = self.nominal_owners.get(&owner).copied() {
            let Some(id) = self
                .nested_aliases_by_owner
                .get(&(owner, name.text.clone()))
                .copied()
            else {
                return Ok(None);
            };
            self.source_alias_template(id, name).ok_or(())?
        } else {
            let bindings = self.imported_static_bindings(
                owner,
                scoop_identity::BindingNamespace::Type,
                &name.text,
            );
            let [binding] = bindings.as_slice() else {
                return Ok(None);
            };
            if !matches!(binding.target(), hir::ImportedTarget::TypeAlias(_)) {
                return Ok(None);
            }
            self.imported_alias_template(binding, name).ok_or(())?
        };
        if alias.outer_parameter_count != 0 && host.is_none() {
            host = self.resolve_applied_qualifier(receiver)?.map(|(ty, _)| ty);
        }
        alias.host = host;
        Ok(Some(alias))
    }
}
