use super::*;
use crate::expr::QualifiedInterfaceProperty;

impl Lowerer {
    pub(crate) fn lower_qualified_interface_super_method_call(
        &mut self,
        qualifier: &ast::TypeRef,
        name: &ast::Ident,
        call: CallSite<'_>,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        let (receiver, qualifier_ty, _) =
            self.resolve_direct_interface_super(qualifier, call.span)?;
        let candidates = self.methods_by_name(qualifier_ty, &name.text);
        if candidates.is_empty() {
            self.error(
                name.span,
                format!(
                    "direct superinterface `{}` has no method `{}`",
                    self.type_name(qualifier_ty),
                    name.text
                ),
            );
            return None;
        }
        self.finish_super_method_call(candidates, &name.text, receiver, call, sink, expected)
    }

    pub(crate) fn resolve_qualified_interface_super_property(
        &mut self,
        qualifier: &ast::TypeRef,
        name: &ast::Ident,
        span: Span,
    ) -> Option<QualifiedInterfaceProperty> {
        let (receiver, qualifier_ty, application) =
            self.resolve_direct_interface_super(qualifier, span)?;
        let Some((property, owner, ty)) = self.find_accessible_interface_application_property(
            application,
            &name.text,
            qualifier_ty,
            &mut Vec::new(),
        ) else {
            self.error(
                name.span,
                format!(
                    "direct superinterface `{}` has no property `{}`",
                    self.type_name(qualifier_ty),
                    name.text
                ),
            );
            return None;
        };
        Some(QualifiedInterfaceProperty {
            property,
            owner,
            receiver,
            ty,
        })
    }

    pub(crate) fn lower_qualified_interface_super_property_read(
        &mut self,
        qualifier: &ast::TypeRef,
        name: &ast::Ident,
        span: Span,
    ) -> Option<hir::Expr> {
        let property = self.resolve_qualified_interface_super_property(qualifier, name, span)?;
        self.lower_direct_interface_property_read(property, span)
    }

    pub(crate) fn lower_direct_interface_property_read(
        &mut self,
        property: QualifiedInterfaceProperty,
        span: Span,
    ) -> Option<hir::Expr> {
        let declaration = self.properties[property.property].clone();
        let getter = self.property_getters[declaration.capability.getter()].clone();
        if !self.property_accessor_is_accessible(
            property.property,
            &getter.access,
            Some(property.receiver.ty),
        ) {
            self.error(
                span,
                format!(
                    "getter of property `{}` is not accessible",
                    declaration.name
                ),
            );
            return None;
        }
        let hir::PropertyAccessorImplementation::Body(function) = getter.implementation else {
            self.error(
                span,
                format!(
                    "abstract interface getter `{}` cannot be called with qualified `super`",
                    declaration.name
                ),
            );
            return None;
        };
        self.check_call_effects(hir::Callable::Function(function), span);
        let application = self.record_method_application(
            function,
            hir::MethodOwnerApplication::Interface(property.owner),
        );
        Some(hir::Expr {
            kind: ExprKind::DirectSuperMethodCall {
                receiver: Box::new(property.receiver),
                callee: hir::MethodCallee::Callable(hir::Callable::Method(application)),
                args: Vec::new(),
            },
            ty: property.ty,
            span,
            origin: self.expression_origin(span),
        })
    }

    pub(crate) fn lower_direct_interface_property_write(
        &mut self,
        property: QualifiedInterfaceProperty,
        value: hir::Expr,
        span: Span,
    ) -> Option<hir::StatementKind> {
        let declaration = self.properties[property.property].clone();
        let Some(setter) = declaration.capability.setter() else {
            self.error(
                span,
                format!("cannot assign to immutable property `{}`", declaration.name),
            );
            return None;
        };
        let setter = self.property_setters[setter].clone();
        if !self.property_accessor_is_accessible(
            property.property,
            &setter.access,
            Some(property.receiver.ty),
        ) {
            self.error(
                span,
                format!(
                    "setter of property `{}` is not accessible",
                    declaration.name
                ),
            );
            return None;
        }
        let hir::PropertyAccessorImplementation::Body(function) = setter.implementation else {
            self.error(
                span,
                format!(
                    "abstract interface setter `{}` cannot be called with qualified `super`",
                    declaration.name
                ),
            );
            return None;
        };
        self.check_call_effects(hir::Callable::Function(function), span);
        let application = self.record_method_application(
            function,
            hir::MethodOwnerApplication::Interface(property.owner),
        );
        Some(hir::StatementKind::Expr(hir::Expr {
            kind: ExprKind::DirectSuperMethodCall {
                receiver: Box::new(property.receiver),
                callee: hir::MethodCallee::Callable(hir::Callable::Method(application)),
                args: vec![value],
            },
            ty: self.unit,
            span,
            origin: self.expression_origin(span),
        }))
    }

    fn resolve_direct_interface_super(
        &mut self,
        qualifier: &ast::TypeRef,
        span: Span,
    ) -> Option<(hir::Expr, TypeId, hir::InterfaceApplicationId)> {
        if self.initialization_context.is_some() {
            self.error(
                span,
                "qualified interface `super` is not allowed during initialization".into(),
            );
            return None;
        }
        if self.current_this.is_none() {
            self.error(
                span,
                "qualified interface `super` is only allowed directly inside a member or accessor body"
                    .into(),
            );
            return None;
        }
        let Some(owner) = self.current_owner else {
            self.error(
                span,
                "qualified interface `super` requires a nominal member owner".into(),
            );
            return None;
        };
        let qualifier_ty = self.resolve_type_ref(qualifier)?;
        let Type::Interface(application) = self.types[qualifier_ty] else {
            self.error(
                qualifier.span,
                "qualified `super` type must be an interface".into(),
            );
            return None;
        };
        let direct = match owner {
            crate::Owner::Class(owner) => self.classes[owner].interfaces.clone(),
            crate::Owner::Struct(owner) => self.structs[owner].interfaces.clone(),
            crate::Owner::Enum(owner) => self.enums[owner].interfaces.clone(),
            crate::Owner::Interface(owner) => self.interfaces[owner]
                .parents
                .iter()
                .map(|parent| self.interface_applications[*parent].canonical_type)
                .collect(),
            crate::Owner::Object(owner) => self.classes[self.objects[owner].backing_class]
                .interfaces
                .clone(),
        };
        if !direct
            .iter()
            .any(|candidate| self.types_equal(*candidate, qualifier_ty))
        {
            self.error(
                qualifier.span,
                format!(
                    "interface `{}` is not a direct superinterface of {}",
                    self.type_name(qualifier_ty),
                    owner.describe(self)
                ),
            );
            return None;
        }
        let mut receiver = self
            .lower_current_this(span)
            .expect("a direct member body has a current receiver");
        receiver.ty = qualifier_ty;
        Some((receiver, qualifier_ty, application))
    }
}
