//! Qualified dependency names use the same static bindings as imports.

use super::*;
use crate::imports::ImportLookupLayer;
use crate::imports::lookup::calls::{
    ExpressionQualifierLookup, ExpressionQualifierTarget, NamedCallBinding, NamedCallOrigin,
    NamedCallTarget,
};
use scoop_identity::BindingNamespace;

#[derive(Clone, Copy)]
pub(in crate::expr) enum ImportedNominalQualifier {
    Applied(TypeId),
    Generic(scoop_identity::PersistentGenericTypeId),
}

impl ImportedNominalQualifier {
    fn applied(self) -> Option<TypeId> {
        match self {
            Self::Applied(ty) => Some(ty),
            Self::Generic(_) => None,
        }
    }
}

impl Lowerer {
    fn imported_qualifier_owner(
        &self,
        qualifier: ImportedNominalQualifier,
    ) -> hir::SourceNominalId {
        match qualifier {
            ImportedNominalQualifier::Applied(ty) => self
                .imported_nominal_owner(ty)
                .expect("a dependency qualifier retains its nominal declaration"),
            ImportedNominalQualifier::Generic(owner) => {
                hir::SourceNominalId::GenericTemplate(owner)
            }
        }
    }

    fn imported_static_bindings(
        &self,
        owner: hir::SourceNominalId,
        namespace: BindingNamespace,
        name: &str,
    ) -> Vec<hir::DirectImportedTargetBinding> {
        self.dependencies
            .as_ref()
            .map_or_else(Vec::new, |dependencies| {
                dependencies
                    .static_bindings(owner, namespace, name)
                    .to_vec()
            })
    }

    fn resolve_imported_nested_type(
        &mut self,
        owner: hir::SourceNominalId,
        name: &ast::Ident,
        arguments: &[ast::TypeRef],
    ) -> Result<Option<TypeId>, ()> {
        let bindings = self.imported_static_bindings(owner, BindingNamespace::Type, &name.text);
        match bindings.as_slice() {
            [] => {
                let declaration = self
                    .dependencies
                    .as_ref()
                    .and_then(|dependencies| dependencies.nested_nominal(owner, &name.text))
                    .cloned();
                let Some(declaration) = declaration else {
                    return Ok(None);
                };
                if !self.access_domain_allows(&self.imported_nominal_access_domain(&declaration)) {
                    self.error(
                        name.span,
                        format!(
                            "type `{}` is not accessible from this source location",
                            name.text
                        ),
                    );
                    return Err(());
                }
                self.resolve_imported_nominal_owner_arguments(declaration.owner(), name, arguments)
                    .map(Some)
                    .ok_or(())
            }
            [binding] => {
                let ty = if arguments.is_empty() {
                    self.resolve_imported_dependency_type_target(binding, name, false)
                } else {
                    self.resolve_imported_generic_type_target(binding, name, arguments)
                };
                ty.map(Some).ok_or(())
            }
            _ => {
                self.error(name.span, format!("ambiguous nested type `{}`", name.text));
                Err(())
            }
        }
    }

    fn resolve_imported_nested_namespace(
        &mut self,
        owner: hir::SourceNominalId,
        name: &ast::Ident,
    ) -> Result<Option<hir::SourceNominalId>, ()> {
        let bindings = self.imported_static_bindings(owner, BindingNamespace::Type, &name.text);
        match bindings.as_slice() {
            [binding] => {
                let qualifier = self
                    .resolve_namespace_type_binding(
                        crate::imports::lookup::TypeLookupTarget::Dependency(binding.clone()),
                        name,
                    )
                    .ok_or(())?;
                match qualifier {
                    crate::types::TypeQualifier::Imported(owner) => Ok(Some(owner)),
                    crate::types::TypeQualifier::Current(_) => {
                        self.error(
                            name.span,
                            format!(
                                "type `{}` does not name a dependency type qualifier",
                                name.text
                            ),
                        );
                        Err(())
                    }
                }
            }
            [] => {
                let declaration = self
                    .dependencies
                    .as_ref()
                    .and_then(|dependencies| dependencies.nested_nominal(owner, &name.text));
                let Some(declaration) = declaration else {
                    return Ok(None);
                };
                if !self.access_domain_allows(&self.imported_nominal_access_domain(declaration)) {
                    self.error(
                        name.span,
                        format!(
                            "type `{}` is not accessible from this source location",
                            name.text
                        ),
                    );
                    return Err(());
                }
                Ok(Some(declaration.owner()))
            }
            _ => {
                self.error(name.span, format!("ambiguous nested type `{}`", name.text));
                Err(())
            }
        }
    }

    pub(crate) fn resolve_imported_qualified_type(
        &mut self,
        mut owner: hir::SourceNominalId,
        path: &[ast::Ident],
        arguments: &[ast::TypeRef],
    ) -> Option<TypeId> {
        let (last, parents) = path
            .split_last()
            .expect("a qualified dependency path has a final type name");
        for name in parents {
            let Some(next) = self.resolve_imported_nested_namespace(owner, name).ok()? else {
                self.imported_missing_nested_type(owner, name);
                return None;
            };
            owner = next;
        }
        let ty = self
            .resolve_imported_nested_type(owner, last, arguments)
            .ok()?;
        if ty.is_none() {
            self.imported_missing_nested_type(owner, last);
        }
        ty
    }

    fn imported_missing_nested_type(&mut self, owner: hir::SourceNominalId, name: &ast::Ident) {
        let declaration = self
            .dependencies
            .as_ref()
            .and_then(|dependencies| dependencies.nominal_declaration(owner))
            .expect("a resolved qualifier retains its actual nominal declaration");
        self.error(
            name.span,
            format!(
                "type `{}` has no accessible nested type `{}`",
                declaration.name(),
                name.text
            ),
        );
    }

    pub(in crate::expr) fn resolve_imported_nominal_qualifier(
        &mut self,
        expression: &ast::Expr,
    ) -> Result<Option<ImportedNominalQualifier>, ()> {
        let owner = match expression {
            ast::Expr::Var(name) => {
                if self.lexical_or_member_value_blocks_type_qualifier(&name.text)
                    || self.lexical_nested_nominal_target(&name.text).is_some()
                {
                    return Ok(None);
                }
                match self.lookup_expression_qualifier(name) {
                    ExpressionQualifierLookup::Unique(
                        ExpressionQualifierTarget::DependencyType(
                            hir::ImportedTarget::GenericType(owner),
                        ),
                    ) => return Ok(Some(ImportedNominalQualifier::Generic(owner.persistent()))),
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
                let bindings = self.imported_static_bindings(
                    self.imported_qualifier_owner(owner),
                    BindingNamespace::Type,
                    &name.text,
                );
                if let [binding] = bindings.as_slice()
                    && let hir::ImportedTarget::GenericType(owner) = binding.target()
                {
                    return Ok(Some(ImportedNominalQualifier::Generic(owner.persistent())));
                }
                return self
                    .resolve_imported_nested_type(self.imported_qualifier_owner(owner), name, &[])
                    .map(|ty| ty.map(ImportedNominalQualifier::Applied));
            }
            _ => return Ok(None),
        };
        Ok(self
            .imported_nominal_owner(owner)
            .map(|_| ImportedNominalQualifier::Applied(owner)))
    }

    pub(in crate::expr) fn lower_imported_qualified_field(
        &mut self,
        owner: ImportedNominalQualifier,
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
        if let Some(ty) = owner.applied()
            && let Some(index) = self.find_imported_variant(ty, &name.text)
        {
            return self.lower_imported_unit_variant(ty, index, name);
        }
        let bindings = self.imported_static_bindings(
            self.imported_qualifier_owner(owner),
            BindingNamespace::Value,
            &name.text,
        );
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
                    return self.lower_imported_variant_binding(binding, name, expected);
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
        if let Some(nested) = self
            .resolve_imported_nested_type(self.imported_qualifier_owner(owner), name, &[])
            .ok()?
            && let Some(value) = self
                .imported_nominal_declaration(nested)
                .and_then(|owner| self.imported_object_value(owner))
        {
            return self.lower_imported_singleton(value, access.span);
        }
        if let Some(value) = owner
            .applied()
            .and_then(|ty| self.imported_nominal_declaration(ty))
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
        let bindings = self.imported_static_bindings(
            self.imported_qualifier_owner(owner),
            BindingNamespace::Value,
            &name.text,
        );
        match bindings.as_slice() {
            [binding] if matches!(binding.target(), hir::ImportedTarget::Property(_)) => {
                Ok(Some(binding.clone()))
            }
            _ => Ok(None),
        }
    }

    pub(in crate::expr) fn lower_imported_qualified_call(
        &mut self,
        owner: ImportedNominalQualifier,
        name: &ast::Ident,
        call: CallSite<'_>,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        if let Some(ty) = owner.applied()
            && let Some(index) = self.find_imported_variant(ty, &name.text)
        {
            return self.lower_imported_variant_construct(ty, index, name, call, sink, Some(ty));
        }
        let mut bindings = self.imported_static_bindings(
            self.imported_qualifier_owner(owner),
            BindingNamespace::Type,
            &name.text,
        );
        if bindings.is_empty()
            && let Some(nested) = self
                .resolve_imported_nested_type(self.imported_qualifier_owner(owner), name, &[])
                .ok()?
        {
            let declaration = self
                .imported_nominal_declaration(nested)
                .expect("a dependency nested type retains its declaration");
            return self.lower_imported_nominal_construct(declaration, name, call, sink, expected);
        }
        if bindings.is_empty()
            && let Some(value) = owner
                .applied()
                .and_then(|ty| self.imported_nominal_declaration(ty))
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
        bindings.extend(self.imported_static_bindings(
            self.imported_qualifier_owner(owner),
            BindingNamespace::Value,
            &name.text,
        ));
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

    fn imported_static_member_error(&mut self, owner: ImportedNominalQualifier, name: &ast::Ident) {
        let declaration = self
            .dependencies
            .as_ref()
            .and_then(|dependencies| {
                dependencies.nominal_declaration(self.imported_qualifier_owner(owner))
            })
            .expect("a static qualifier retains its dependency declaration");
        let owner_name = owner
            .applied()
            .map(|ty| self.type_name(ty))
            .unwrap_or_else(|| declaration.name().to_owned());
        let message = if matches!(
            declaration.interface.source_shape(),
            hir::NominalSourceShapeV1::Enum(_)
        ) {
            format!("enum `{}` has no variant `{}`", owner_name, name.text)
        } else {
            format!(
                "type `{}` has no accessible static member `{}`",
                owner_name, name.text
            )
        };
        self.error(name.span, message);
    }
}
