use super::*;
use crate::NominalTarget;
use crate::imports::lookup::calls::{ExpressionQualifierLookup, ExpressionQualifierTarget};

mod applied;

impl Lowerer {
    /// A direct receiver spelling is still an expression name first. Keep
    /// this guard shared by every qualifier consumer so none of them can
    /// bypass a higher lexical/member value while probing a lower type.
    pub(in crate::expr) fn lexical_or_member_value_blocks_type_qualifier(
        &self,
        name: &str,
    ) -> bool {
        let initializing_field = self.initialization_context.is_some() && {
            let mut state = self.clone();
            state.initializing_receiver_has_field(name)
        };
        let object_const = match self.current_owner {
            Some(crate::Owner::Object(object)) => self.classes[self.objects[object].backing_class]
                .properties
                .iter()
                .any(|property| {
                    self.properties[*property].name == name
                        && matches!(
                            self.properties[*property].representation,
                            hir::PropertyRepresentation::Const { .. }
                        )
                }),
            _ => false,
        };
        (name == "field" && self.backing_field_context.is_some())
            || self.scopes.lookup(name).is_some()
            || !self.local_function_scopes.lookup(name).is_empty()
            || self.available_capture(name).is_some()
            || self.constructor_params_in_scope.contains_key(name)
            || initializing_field
            || object_const
            || self.host_has_property(name)
    }

    pub(crate) fn nominal_qualifier_target(&self, expression: &ast::Expr) -> Option<NominalTarget> {
        match expression {
            ast::Expr::Var(name)
                if !self.lexical_or_member_value_blocks_type_qualifier(&name.text) =>
            {
                self.lexical_nested_nominal_target(&name.text).or_else(|| {
                    match self.lookup_expression_qualifier(name) {
                        ExpressionQualifierLookup::Unique(ExpressionQualifierTarget::Object(
                            object,
                        ))
                        | ExpressionQualifierLookup::Inaccessible(
                            ExpressionQualifierTarget::Object(object),
                        ) => Some(NominalTarget::Object(object)),
                        ExpressionQualifierLookup::Unique(ExpressionQualifierTarget::Type(
                            crate::namespace::TopLevelTypeTarget::Nominal(target),
                        ))
                        | ExpressionQualifierLookup::Inaccessible(
                            ExpressionQualifierTarget::Type(
                                crate::namespace::TopLevelTypeTarget::Nominal(target),
                            ),
                        ) => Some(target),
                        ExpressionQualifierLookup::Unique(ExpressionQualifierTarget::Type(
                            crate::namespace::TopLevelTypeTarget::Alias(alias),
                        ))
                        | ExpressionQualifierLookup::Inaccessible(
                            ExpressionQualifierTarget::Type(
                                crate::namespace::TopLevelTypeTarget::Alias(alias),
                            ),
                        ) => self
                            .resolved_type_alias_target(alias)
                            .and_then(|target| self.nominal_target_for_type(target)),
                        ExpressionQualifierLookup::Unique(
                            ExpressionQualifierTarget::DependencyObject(_),
                        )
                        | ExpressionQualifierLookup::Inaccessible(
                            ExpressionQualifierTarget::DependencyObject(_),
                        )
                        | ExpressionQualifierLookup::Missing
                        | ExpressionQualifierLookup::Value
                        | ExpressionQualifierLookup::Ambiguous
                        | ExpressionQualifierLookup::Unique(
                            ExpressionQualifierTarget::DependencyType(_),
                        )
                        | ExpressionQualifierLookup::Inaccessible(
                            ExpressionQualifierTarget::DependencyType(_),
                        ) => None,
                    }
                })
            }
            ast::Expr::FieldAccess(access) if access.navigation == ast::Navigation::Direct => {
                let ast::FieldSelector::Name(name) = &access.selector else {
                    return None;
                };
                let owner = self.nominal_qualifier_target(&access.receiver)?.owner();
                self.nested_nominal_target(owner, &name.text)
            }
            _ => None,
        }
    }

    /// Resolve a direct alias qualifier after value bindings have had their
    /// normal shadowing opportunity. This uses the resolver-owned API so the
    /// access diagnostic is identical in type and expression positions.
    pub(in crate::expr) fn resolve_direct_alias_qualifier(
        &mut self,
        expression: &ast::Expr,
    ) -> Result<Option<(AliasExpansion, NominalTarget)>, ()> {
        let Some((target, nominal)) = self.resolve_direct_type_alias_qualifier(expression)? else {
            return Ok(None);
        };
        let ast::Expr::Var(name) = expression else {
            unreachable!("a direct alias qualifier is a source name")
        };
        Ok(Some((
            AliasExpansion {
                name: name.clone(),
                target,
            },
            nominal,
        )))
    }

    pub(crate) fn resolve_direct_type_alias_qualifier(
        &mut self,
        expression: &ast::Expr,
    ) -> Result<Option<(TypeId, NominalTarget)>, ()> {
        let ast::Expr::Var(name) = expression else {
            return Ok(None);
        };
        if self.lexical_or_member_value_blocks_type_qualifier(&name.text)
            || self.lexical_nested_nominal_target(&name.text).is_some()
        {
            return Ok(None);
        }
        let alias = match self.lookup_expression_qualifier(name) {
            ExpressionQualifierLookup::Unique(ExpressionQualifierTarget::Type(
                crate::namespace::TopLevelTypeTarget::Alias(alias),
            ))
            | ExpressionQualifierLookup::Inaccessible(ExpressionQualifierTarget::Type(
                crate::namespace::TopLevelTypeTarget::Alias(alias),
            )) => alias,
            _ => return Ok(None),
        };
        let Some(target) = self.resolve_type_alias_id_reference(alias, name, false) else {
            return Err(());
        };
        let Some(nominal) = self.nominal_target_for_type(target) else {
            self.error(
                name.span,
                format!("typealias `{}` does not name a type qualifier", name.text),
            );
            return Err(());
        };
        Ok(Some((target, nominal)))
    }

    pub(super) fn lower_static_nested_constructor(
        &mut self,
        target: NominalTarget,
        name: &ast::Ident,
        call: CallSite<'_>,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        match target {
            NominalTarget::Struct(struct_id) => {
                let application = self.structs[struct_id].self_application;
                let ty = self.struct_applications[application].canonical_type;
                if !self.nominal_is_accessible(ty) {
                    self.error(
                        name.span,
                        format!("struct `{}` is not accessible here", name.text),
                    );
                    return None;
                }
                self.lower_struct_init(struct_id, ty, call, sink, expected)
            }
            NominalTarget::Class(class_id) => {
                let application = self.classes[class_id].self_application;
                let ty = self.class_applications[application].canonical_type;
                if !self.nominal_is_accessible(ty) {
                    self.error(
                        name.span,
                        format!("class `{}` is not accessible here", name.text),
                    );
                    return None;
                }
                self.lower_class_construct(class_id, call, sink, expected)
            }
            NominalTarget::Enum(_) => {
                self.error(
                    name.span,
                    format!(
                        "enum `{}` cannot be constructed without a variant",
                        name.text
                    ),
                );
                None
            }
            NominalTarget::Interface(_) => {
                self.error(
                    name.span,
                    format!("interface `{}` cannot be constructed", name.text),
                );
                None
            }
            NominalTarget::Object(object) => {
                let kind = match self.objects[object].kind {
                    hir::ObjectKind::Standalone => "object",
                    hir::ObjectKind::Companion(_) => "companion object",
                };
                self.error(
                    name.span,
                    format!("{kind} `{}` cannot be constructed", name.text),
                );
                None
            }
        }
    }
}
