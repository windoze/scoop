//! Qualified dependency names use the same static bindings as imports.

use super::*;
use crate::imports::ImportLookupLayer;
use crate::imports::lookup::calls::{
    ExpressionQualifierLookup, ExpressionQualifierTarget, NamedCallBinding, NamedCallOrigin,
    NamedCallTarget,
};
use scoop_identity::BindingNamespace;

impl Lowerer {
    fn imported_static_bindings(
        &self,
        owner: TypeId,
        namespace: BindingNamespace,
        name: &str,
    ) -> Vec<hir::DirectImportedTargetBinding> {
        let Some(owner) = self.imported_nominal_declaration(owner) else {
            return Vec::new();
        };
        self.dependencies
            .as_ref()
            .map_or_else(Vec::new, |dependencies| {
                dependencies
                    .static_bindings(hir::SourceNominalId::Concrete(owner), namespace, name)
                    .to_vec()
            })
    }

    fn resolve_imported_nested_type(
        &mut self,
        owner: TypeId,
        name: &ast::Ident,
        supplied_type_arguments: bool,
    ) -> Result<Option<TypeId>, ()> {
        let bindings = self.imported_static_bindings(owner, BindingNamespace::Type, &name.text);
        match bindings.as_slice() {
            [] => {
                let declaration = self
                    .imported_nominal_declaration(owner)
                    .and_then(|owner| {
                        self.dependencies
                            .as_ref()?
                            .nested_nominal(owner, &name.text)
                    })
                    .cloned();
                let Some(declaration) = declaration else {
                    return Ok(None);
                };
                if supplied_type_arguments {
                    self.error(name.span, format!("type `{}` is not generic", name.text));
                    return Err(());
                }
                let ty = self
                    .imported_signature_type(&scoop_identity::SignatureTypeKey::Nominal(
                        declaration.identity.id(),
                    ))
                    .map_err(|error| {
                        self.error(
                            name.span,
                            format!("invalid dependency nested type: {error:?}"),
                        )
                    })?;
                if !self.nominal_is_accessible(ty) {
                    self.error(
                        name.span,
                        format!(
                            "type `{}` is not accessible from this source location",
                            self.type_name(ty)
                        ),
                    );
                    return Err(());
                }
                Ok(Some(ty))
            }
            [binding] => self
                .resolve_imported_dependency_type_target(binding, name, supplied_type_arguments)
                .map(Some)
                .ok_or(()),
            _ => {
                self.error(name.span, format!("ambiguous nested type `{}`", name.text));
                Err(())
            }
        }
    }

    pub(crate) fn resolve_imported_qualified_type(
        &mut self,
        mut owner: TypeId,
        path: &[ast::Ident],
        arguments: &[ast::TypeRef],
    ) -> Option<TypeId> {
        for (index, name) in path.iter().enumerate() {
            let Some(nested) = self
                .resolve_imported_nested_type(
                    owner,
                    name,
                    index + 1 == path.len() && !arguments.is_empty(),
                )
                .ok()?
            else {
                self.error(
                    name.span,
                    format!(
                        "type `{}` has no accessible nested type `{}`",
                        self.type_name(owner),
                        name.text
                    ),
                );
                return None;
            };
            owner = nested;
        }
        Some(owner)
    }

    pub(in crate::expr) fn resolve_imported_nominal_qualifier(
        &mut self,
        expression: &ast::Expr,
    ) -> Result<Option<TypeId>, ()> {
        let owner = match expression {
            ast::Expr::Var(name) => {
                if self.lexical_or_member_value_blocks_type_qualifier(&name.text)
                    || self.lexical_nested_nominal_target(&name.text).is_some()
                {
                    return Ok(None);
                }
                match self.lookup_expression_qualifier(name) {
                    ExpressionQualifierLookup::Unique(
                        ExpressionQualifierTarget::DependencyType(_),
                    ) => self
                        .resolve_type_ref(&ast::TypeRef {
                            kind: ast::TypeRefKind::Named(name.clone()),
                            span: name.span,
                        })
                        .ok_or(())?,
                    ExpressionQualifierLookup::Unique(
                        ExpressionQualifierTarget::DependencyObject(value),
                    ) => self.imported_singleton_type(value).map_err(|error| {
                        self.error(
                            name.span,
                            format!("invalid dependency singleton: {error:?}"),
                        )
                    })?,
                    ExpressionQualifierLookup::Unique(ExpressionQualifierTarget::Type(
                        crate::namespace::TopLevelTypeTarget::Alias(alias),
                    ))
                    | ExpressionQualifierLookup::Inaccessible(ExpressionQualifierTarget::Type(
                        crate::namespace::TopLevelTypeTarget::Alias(alias),
                    )) => self
                        .resolve_type_alias_id_reference(alias, name, false)
                        .ok_or(())?,
                    _ => return Ok(None),
                }
            }
            ast::Expr::FieldAccess(access) if access.navigation == ast::Navigation::Direct => {
                let ast::FieldSelector::Name(name) = &access.selector else {
                    return Ok(None);
                };
                let Some(owner) = self.resolve_imported_nominal_qualifier(&access.receiver)? else {
                    return Ok(None);
                };
                return self.resolve_imported_nested_type(owner, name, false);
            }
            _ => return Ok(None),
        };
        Ok(self.imported_nominal_declaration(owner).map(|_| owner))
    }

    pub(in crate::expr) fn lower_imported_qualified_field(
        &mut self,
        owner: TypeId,
        access: &ast::FieldAccess,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        let ast::FieldSelector::Name(name) = &access.selector else {
            self.error(
                access.span,
                "a type qualifier requires a named member".into(),
            );
            return None;
        };
        let bindings = self.imported_static_bindings(owner, BindingNamespace::Value, &name.text);
        match bindings.as_slice() {
            [binding] => match binding.target() {
                hir::ImportedTarget::ObjectValue(value) => {
                    return self.lower_imported_singleton(value.persistent(), access.span);
                }
                hir::ImportedTarget::Property(_) => {
                    return self
                        .lower_imported_dependency_property_read(binding, None, access.span)
                        .map(|read| read.expression);
                }
                hir::ImportedTarget::EnumVariant(_) => {
                    return self.lower_imported_variant_binding(binding, name);
                }
                _ => {}
            },
            [] => {}
            _ => {
                self.error(
                    name.span,
                    format!("static member `{}` is not an unambiguous value", name.text),
                );
                return None;
            }
        }
        if let Some(index) = self.find_imported_variant(owner, &name.text) {
            return self.lower_imported_unit_variant(owner, index, name);
        }
        if let Some(nested) = self.resolve_imported_nested_type(owner, name, false).ok()?
            && let Some(value) = self
                .imported_nominal_declaration(nested)
                .and_then(|owner| self.imported_object_value(owner))
        {
            return self.lower_imported_singleton(value, access.span);
        }
        if let Some(value) = self
            .imported_nominal_declaration(owner)
            .and_then(|owner| self.imported_object_value(owner))
        {
            let receiver = self.lower_imported_singleton(value, access.receiver.span())?;
            if let Some(value) = self
                .lower_imported_member_property_read(&receiver, name, access.span, expected)
                .ok()?
            {
                return Some(value);
            }
        }
        self.imported_static_member_error(owner, name);
        None
    }

    pub(crate) fn resolve_imported_qualified_property(
        &mut self,
        receiver: &ast::Expr,
        name: &ast::Ident,
    ) -> Result<Option<hir::DirectImportedTargetBinding>, ()> {
        let Some(owner) = self.resolve_imported_nominal_qualifier(receiver)? else {
            return Ok(None);
        };
        let bindings = self.imported_static_bindings(owner, BindingNamespace::Value, &name.text);
        match bindings.as_slice() {
            [binding] if matches!(binding.target(), hir::ImportedTarget::Property(_)) => {
                Ok(Some(binding.clone()))
            }
            _ => Ok(None),
        }
    }

    pub(in crate::expr) fn lower_imported_qualified_call(
        &mut self,
        owner: TypeId,
        name: &ast::Ident,
        call: CallSite<'_>,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        if let Some(index) = self.find_imported_variant(owner, &name.text) {
            return self.lower_imported_variant_construct(owner, index, name, call, sink, expected);
        }
        let mut bindings = self.imported_static_bindings(owner, BindingNamespace::Type, &name.text);
        if bindings.is_empty()
            && let Some(nested) = self.resolve_imported_nested_type(owner, name, false).ok()?
        {
            let declaration = self
                .imported_nominal_declaration(nested)
                .expect("a dependency nested type retains its declaration");
            return self.lower_imported_nominal_construct(declaration, name, call, sink, expected);
        }
        if bindings.is_empty()
            && let Some(value) = self
                .imported_nominal_declaration(owner)
                .and_then(|owner| self.imported_object_value(owner))
        {
            let receiver = self.lower_imported_singleton(value, name.span)?;
            return self.lower_explicit_named_call(
                receiver,
                name,
                call,
                sink,
                expected,
                RequiredCallableModifiers::default(),
            );
        }
        bindings.extend(self.imported_static_bindings(owner, BindingNamespace::Value, &name.text));
        if bindings.is_empty() {
            self.imported_static_member_error(owner, name);
            return None;
        }
        let targets = bindings
            .into_iter()
            .map(|binding| NamedCallBinding {
                target: NamedCallTarget::ImportedDependency(binding.target()),
                origin: NamedCallOrigin::Dependency(binding),
            })
            .collect::<Vec<_>>();
        self.lower_named_function_partition(
            &targets,
            ImportLookupLayer::Exact,
            &ast::CallExpr {
                callee: name.clone(),
                type_args: call.type_args.to_vec(),
                args: call.args.to_vec(),
                span: call.span,
            },
            sink,
            expected,
        )
        .ok()
        .flatten()
    }

    fn imported_static_member_error(&mut self, owner: TypeId, name: &ast::Ident) {
        let message = if matches!(self.types[owner], Type::ImportedEnum(_)) {
            format!(
                "enum `{}` has no variant `{}`",
                self.type_name(owner),
                name.text
            )
        } else {
            format!(
                "type `{}` has no accessible static member `{}`",
                self.type_name(owner),
                name.text
            )
        };
        self.error(name.span, message);
    }
}
