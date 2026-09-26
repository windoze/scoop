use super::*;
use crate::imports::lookup::calls::{ExpressionQualifierLookup, ExpressionQualifierTarget};

impl Lowerer {
    pub(in crate::expr) fn resolve_imported_enum_qualifier(
        &mut self,
        expression: &ast::Expr,
    ) -> Result<Option<TypeId>, ()> {
        let ast::Expr::Var(name) = expression else {
            return Ok(None);
        };
        if self.lexical_or_member_value_blocks_type_qualifier(&name.text)
            || self.lexical_nested_nominal_target(&name.text).is_some()
        {
            return Ok(None);
        }
        let owner = match self.lookup_expression_qualifier(name) {
            ExpressionQualifierLookup::Unique(ExpressionQualifierTarget::DependencyType(_)) => self
                .resolve_type_ref(&ast::TypeRef {
                    kind: ast::TypeRefKind::Named(name.clone()),
                    span: name.span,
                })
                .ok_or(())?,
            ExpressionQualifierLookup::Unique(ExpressionQualifierTarget::Type(
                crate::namespace::TopLevelTypeTarget::Alias(alias),
            ))
            | ExpressionQualifierLookup::Inaccessible(ExpressionQualifierTarget::Type(
                crate::namespace::TopLevelTypeTarget::Alias(alias),
            )) => self
                .resolve_type_alias_id_reference(alias, name, false)
                .ok_or(())?,
            _ => return Ok(None),
        };
        Ok(matches!(self.types[owner], Type::ImportedEnum(_)).then_some(owner))
    }
}
