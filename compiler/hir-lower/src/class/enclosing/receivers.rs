use super::*;

impl Lowerer {
    pub(crate) fn enclosing_receiver_class(&mut self, receiver: hir::Expr) -> Option<hir::Expr> {
        let ty = if let Type::Param(parameter) = self.types[receiver.ty] {
            let parameter = self
                .type_params_in_scope
                .iter()
                .find(|value| value.id == parameter)?;
            let hir::TypeParamBounds::Nominal(bounds) = &parameter.bounds else {
                return None;
            };
            bounds.class.as_ref()?.ty
        } else {
            receiver.ty
        };
        matches!(self.types[ty], Type::Class(_)).then(|| self.adapt_to(receiver, ty))
    }

    pub(crate) fn enclosing_instance_expression(
        &mut self,
        receiver: hir::Expr,
    ) -> Option<hir::Expr> {
        let Type::Class(application) = self.types[receiver.ty] else {
            return None;
        };
        let application_value = self.class_applications[application].clone();
        let (field, ty) = match self
            .nominal_owners
            .get(&application_value.template)
            .copied()
        {
            Some(Owner::Class(class)) => {
                let field = self.enclosing_class_field(class)?;
                let ty = self.class_field_definition(field).ty;
                (self.class_field_reference(application, field), ty)
            }
            Some(_) => return None,
            None => {
                let definition = &self.loaded_class_definitions[&application_value.template];
                let index = definition
                    .declaration
                    .field_sources
                    .iter()
                    .position(|field| {
                        field.storage == hir::NominalFieldStorage::EnclosingInstance
                    })?;
                (
                    hir::FieldRef::ClassField {
                        owner: application_value.canonical_type,
                        field: definition.field_identity(index),
                    },
                    definition.definition.fields[index].ty,
                )
            }
        };
        let ty = self.instantiate_ty(ty, &application_value.arguments);
        Some(hir::Expr {
            span: receiver.span,
            origin: receiver.origin,
            kind: hir::ExprKind::FieldAccess {
                receiver: Box::new(receiver),
                field,
            },
            ty,
        })
    }

    pub(crate) fn bind_enclosing_receivers(
        &mut self,
        mut outer: hir::Expr,
        sink: &mut Vec<hir::Statement>,
    ) {
        let mut scopes = Vec::new();
        loop {
            let span = outer.span;
            let value = self.materialize_temporary("$outer".into(), outer, span, sink);
            let hir::ExprKind::Local(local) = value.kind else {
                unreachable!("a saved outer instance is a local")
            };
            let application = self
                .nominal_application(value.ty)
                .expect("an enclosing instance is a class application");
            let label = match self.nominal_owners.get(&application.template).copied() {
                Some(owner) => self.receiver_owner_label(owner),
                None => self.loaded_class_definitions[&application.template]
                    .declaration
                    .name()
                    .into(),
            };
            scopes.push(crate::ReceiverScope {
                label: Some(label),
                receiver: Some(crate::ReceiverBinding {
                    binding: self.locals[local].binding,
                    ty: value.ty,
                }),
            });
            let Some(enclosing) = self.enclosing_instance_expression(value) else {
                break;
            };
            outer = enclosing;
        }
        self.receiver_scopes.extend(scopes.into_iter().rev());
    }

    pub(crate) fn enclosing_receiver_application(
        &mut self,
        receiver: hir::Expr,
        required: hir::SourceNominalId,
    ) -> Option<hir::Expr> {
        let mut receiver = self.enclosing_receiver_class(receiver)?;
        let mut seen = Vec::new();
        loop {
            let Type::Class(application) = self.types[receiver.ty] else {
                return None;
            };
            if seen.contains(&application) {
                return None;
            }
            seen.push(application);
            let value = self.class_applications[application].clone();
            if value.template == required {
                return Some(receiver);
            }
            if matches!(
                self.nominal_owners.get(&value.template),
                Some(Owner::Object(_))
            ) {
                return None;
            }
            let base = self.class_definition(value.template).base_class?;
            let base = self.instantiate_ty(base, &value.arguments);
            receiver = self.adapt_to(receiver, base);
        }
    }

    pub(crate) fn implicit_enclosing_instance(
        &mut self,
        required: hir::SourceNominalId,
        span: ast::Span,
    ) -> Option<hir::Expr> {
        for binding in self.implicit_receivers() {
            let receiver = self.lower_receiver_binding(binding, span)?;
            if let Some(receiver) = self.enclosing_receiver_application(receiver, required) {
                return Some(receiver);
            }
        }
        self.error(
            span,
            "inner constructor requires a matching outer instance".into(),
        );
        None
    }

    pub(crate) fn is_initializing_this_label(&self, label: Option<&ast::Ident>) -> bool {
        self.initialization_context.is_some()
            && label.is_none_or(|label| {
                self.current_owner
                    .is_some_and(|owner| self.receiver_owner_label(owner) == label.text)
            })
    }
}
