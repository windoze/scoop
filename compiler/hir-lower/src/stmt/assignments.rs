use super::*;
use crate::expr::{CallSite, RequiredCallableModifiers};

mod names;

impl Lowerer {
    /// Assignment. A `Local` target names a declared, mutable local; an
    /// `Index` target (`array[index] = value`, spec 10.5) requires a
    /// `MutableArray<T>` receiver, a `Long` index and a value of
    /// exactly the element type `T`; a `Field` target
    /// (`obj.field = value`, M6) requires a class receiver and a `var`
    /// constructor property. In all cases the target type is the
    /// value's expected-type hint.
    pub(super) fn lower_assign(
        &mut self,
        assign: &ast::Assign,
        out: &mut Vec<hir::Statement>,
    ) -> Option<hir::StatementKind> {
        if let ast::AssignmentOp::Compound(op) = assign.op {
            return self.lower_compound_assign(assign, op, out);
        }
        match &assign.target {
            ast::AssignTarget::Name(name) => self.lower_local_assign(assign, name, out),
            ast::AssignTarget::Index {
                receiver, indices, ..
            } => self.lower_index_assign(assign, receiver, indices.as_slice(), out),
            ast::AssignTarget::Field { receiver, name, .. } => {
                if matches!(&**receiver, ast::Expr::This { .. })
                    && self.initialization_context.is_some()
                {
                    return self.lower_initializing_field_assign(assign, name, out);
                }
                if self
                    .qualified_object_const_property(receiver, &name.text)
                    .is_some()
                {
                    self.error(
                        name.span,
                        format!("cannot assign to immutable property `{}`", name.text),
                    );
                    return None;
                }
                if let Some(binding) = self
                    .resolve_imported_qualified_property(receiver, name)
                    .ok()?
                {
                    return self.lower_imported_dependency_property_assignment(
                        &binding,
                        name,
                        &assign.value,
                        out,
                    );
                }
                let mut sink = Vec::new();
                let forwarding = self
                    .nominal_qualifier_target(receiver)
                    .and_then(|host| self.companion_forwarding_property_object(host, &name.text));
                let receiver = match forwarding {
                    Some(companion) => self.lower_singleton_value(companion, receiver.span())?,
                    None => self.lower_expr(receiver, &mut sink, None)?,
                };
                let kind = self.assign_class_field(assign, receiver, name, &mut sink)?;
                out.extend(sink);
                Some(kind)
            }
            ast::AssignTarget::QualifiedInterfaceSuperProperty {
                qualifier,
                name,
                span,
            } => {
                let property =
                    self.resolve_qualified_interface_super_property(qualifier, name, *span)?;
                let mut sink = Vec::new();
                let value = self.lower_expr(&assign.value, &mut sink, Some(property.ty))?;
                if !self.is_subtype(value.ty, property.ty) {
                    self.error(
                        assign.value.span(),
                        format!(
                            "cannot assign value of type {} to property `{}` of type {}",
                            self.type_name(value.ty),
                            name.text,
                            self.type_name(property.ty)
                        ),
                    );
                    return None;
                }
                let expected = property.ty;
                let value = self.adapt_to(value, expected);
                out.extend(sink);
                self.lower_direct_interface_property_write(property, value, *span)
            }
        }
    }

    fn lower_initializing_field_assign(
        &mut self,
        assign: &ast::Assign,
        name: &ast::Ident,
        out: &mut Vec<hir::Statement>,
    ) -> Option<hir::StatementKind> {
        let field = self.initializing_field(name, assign.span)?;
        let Some(target) = field.write else {
            self.error(
                name.span,
                format!("cannot assign to immutable property `{}`", name.text),
            );
            return None;
        };
        let mut sink = Vec::new();
        let value = self.lower_expr(&assign.value, &mut sink, Some(field.read.ty))?;
        if !self.is_subtype(value.ty, field.read.ty) {
            let message = self.with_nominal_invariance_detail(
                format!(
                    "cannot assign value of type {} to property `{}` of type {}",
                    self.type_name(value.ty),
                    name.text,
                    self.type_name(field.read.ty)
                ),
                value.ty,
                field.read.ty,
            );
            self.error(assign.value.span(), message);
            return None;
        }
        out.extend(sink);
        Some(hir::StatementKind::Assign {
            target,
            value: self.adapt_to(value, field.read.ty),
        })
    }

    /// Resolves a member or extension property and preserves receiver-before-value
    /// evaluation. Imported members call the actual provider's setter.
    fn assign_class_field(
        &mut self,
        assign: &ast::Assign,
        receiver: hir::Expr,
        name: &ast::Ident,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::StatementKind> {
        let receiver_ty = receiver.ty;
        if let Some((property, owner, property_ty)) =
            self.find_accessible_nominal_property(receiver_ty, &name.text)
        {
            let value = self.lower_expr(&assign.value, sink, Some(property_ty))?;
            if !self.is_subtype(value.ty, property_ty) {
                let expected = self.type_name(property_ty);
                let found = self.type_name(value.ty);
                let message = self.with_nominal_invariance_detail(
                    format!(
                        "cannot assign value of type {found} to property `{}` of type {expected}",
                        name.text
                    ),
                    value.ty,
                    property_ty,
                );
                self.error(assign.value.span(), message);
                return None;
            }
            let value = self.adapt_to(value, property_ty);
            return self.lower_property_write(
                property,
                Some(owner),
                Some(receiver),
                value,
                name.span,
            );
        }
        if let Some(property) = self
            .resolve_imported_member_property(receiver_ty, name)
            .ok()?
        {
            let receiver = self.materialize_place_expr(receiver, "place", name.span, sink);
            let value = self.lower_expr(&assign.value, sink, Some(property.value_type))?;
            return self.lower_imported_member_property_write(
                property,
                receiver,
                value,
                name,
                assign.span,
            );
        }
        let resolved = match self.resolve_extension_property_write(receiver, name, sink) {
            crate::properties::ExtensionPropertyResolution::Resolved(property) => property,
            crate::properties::ExtensionPropertyResolution::Failed => return None,
            crate::properties::ExtensionPropertyResolution::NoCandidate => {
                match self.types[receiver_ty] {
                    Type::Class(application) => {
                        let class = self.classes[self.class_applications[application].template]
                            .name
                            .clone();
                        self.error(
                            name.span,
                            format!("class `{class}` has no property `{}`", name.text),
                        );
                    }
                    _ if self.is_value_ty(receiver_ty) => self.error(
                        name.span,
                        "field assignment is not supported (value types are immutable)".to_string(),
                    ),
                    _ => {
                        let found = self.type_name(receiver_ty);
                        self.error(name.span, format!("type `{found}` has no fields"));
                    }
                }
                return None;
            }
        };
        let property_ty = resolved.value_type();
        let value = self.lower_expr(&assign.value, sink, Some(property_ty))?;
        if !self.is_subtype(value.ty, property_ty) {
            let expected = self.type_name(property_ty);
            let found = self.type_name(value.ty);
            let message = self.with_nominal_invariance_detail(
                format!(
                    "cannot assign value of type {found} to property `{}` of type {expected}",
                    name.text
                ),
                value.ty,
                property_ty,
            );
            self.error(assign.value.span(), message);
            return None;
        }
        let value = self.adapt_to(value, property_ty);
        self.lower_extension_property_write(*resolved, value, name.span)
    }

    /// Indexed assignment uses the same typed `operator set` resolver as user
    /// types. A winning MutableArray core declaration is normalized to the
    /// dedicated array store expression after applicability succeeds.
    fn lower_index_assign(
        &mut self,
        assign: &ast::Assign,
        receiver: &ast::Expr,
        indices: &[ast::Expr],
        out: &mut Vec<hir::Statement>,
    ) -> Option<hir::StatementKind> {
        let mut sink = Vec::new();
        let array = self.lower_expr(receiver, &mut sink, None)?;
        let mut arguments = indices
            .iter()
            .cloned()
            .map(ast::CallArgument::positional)
            .collect::<Vec<_>>();
        arguments.push(ast::CallArgument::positional(assign.value.clone()));
        let call = self.lower_named_call_on_receiver(
            array,
            &ast::Ident {
                text: "set".to_string(),
                span: assign.span,
            },
            CallSite {
                type_args: &[],
                args: &arguments,
                span: assign.span,
            },
            &mut sink,
            Some(self.unit),
            RequiredCallableModifiers {
                operator: Some(hir::OperatorKind::Set),
                infix: false,
                ..Default::default()
            },
        )?;
        out.extend(sink);
        Some(hir::StatementKind::Expr(call))
    }
}
