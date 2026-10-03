use super::*;

impl Lowerer {
    pub(super) fn resolve_named_place_plan(
        &mut self,
        name: &ast::Ident,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<ResolvedPlacePlan> {
        if self.reject_release_field_write(name) {
            return None;
        }
        if name.text == "field" && self.backing_field_context.is_some() {
            let (read, write) = self.contextual_backing_field(name.span)?;
            let ty = read.ty;
            return Some(ResolvedPlacePlan {
                read,
                write: write
                    .map(WriteCapability::Direct)
                    .unwrap_or(WriteCapability::ReadOnly),
                ty,
            });
        }
        if let Some(local) = self.scopes.lookup(&name.text) {
            let binding = self.locals[local].binding;
            if let Some(plan) = self.local_delegate_plans.get(&binding).cloned() {
                let storage = hir::Expr {
                    kind: hir::ExprKind::Local(local),
                    ty: self.locals[local].ty,
                    span: name.span,
                    origin: self.expression_origin(name.span),
                };
                let read = self.local_delegate_read(storage.clone(), binding, name.span)?;
                let write = if plan.mutable {
                    WriteCapability::LocalDelegate { storage, binding }
                } else {
                    WriteCapability::ReadOnly
                };
                return Some(ResolvedPlacePlan {
                    read,
                    write,
                    ty: plan.property_ty,
                });
            }
            let read = self.lower_var(name, sink, None)?;
            let write = if self.locals[local].mutable {
                WriteCapability::Direct(hir::AssignTarget::Local(local))
            } else {
                WriteCapability::ReadOnly
            };
            return Some(ResolvedPlacePlan {
                ty: read.ty,
                read,
                write,
            });
        }
        if let Some(capture) = self.available_capture(&name.text) {
            if let Some(plan) = self.local_delegate_plans.get(&capture.binding).cloned() {
                let storage = self.lower_capture(name)?;
                let read = self.local_delegate_read(storage.clone(), capture.binding, name.span)?;
                let write = if plan.mutable {
                    WriteCapability::LocalDelegate {
                        storage,
                        binding: capture.binding,
                    }
                } else {
                    WriteCapability::ReadOnly
                };
                return Some(ResolvedPlacePlan {
                    read,
                    write,
                    ty: plan.property_ty,
                });
            }
            let read = self.lower_var(name, sink, None)?;
            return Some(ResolvedPlacePlan {
                ty: read.ty,
                read,
                write: WriteCapability::ReadOnly,
            });
        }
        if self.constructor_params_in_scope.contains_key(&name.text) {
            let read = self.lower_var(name, sink, None)?;
            return Some(ResolvedPlacePlan {
                ty: read.ty,
                read,
                write: WriteCapability::ReadOnly,
            });
        }
        if self.initialization_context.is_some() && self.initializing_receiver_has_field(&name.text)
        {
            let field = self.initializing_field(name, name.span)?;
            let write = field
                .write
                .map(WriteCapability::Direct)
                .unwrap_or(WriteCapability::ReadOnly);
            return Some(ResolvedPlacePlan {
                ty: field.read.ty,
                read: field.read,
                write,
            });
        }
        if let Some(receiver_ty) = self.current_this_ty()
            && let Some((property, owner, ty)) =
                self.find_accessible_nominal_property(receiver_ty, &name.text)
        {
            let receiver = self.lower_current_this(name.span)?;
            let read = self.lower_property_read(
                property,
                Some(owner),
                Some(receiver.clone()),
                ty,
                name.span,
            )?;
            let write = if self.properties[property].capability.setter().is_some() {
                WriteCapability::Property {
                    property,
                    owner: Some(owner),
                    receiver: Some(receiver),
                }
            } else {
                WriteCapability::ReadOnly
            };
            return Some(ResolvedPlacePlan { read, write, ty });
        }
        if let Some(receiver_ty) = self.current_this_ty()
            && let Some(property) = self
                .resolve_imported_member_property(receiver_ty, name)
                .ok()?
        {
            let receiver = self.lower_current_this(name.span)?;
            return self.imported_member_place(property, receiver, name, name.span);
        }
        let mut selected_value = None;
        if self.initialization_context.is_none()
            && let Some(receiver) = self.lower_current_this(name.span)
        {
            match self.resolve_implicit_value_read(receiver, name, sink) {
                crate::properties::ImplicitValueResolution::ExtensionProperty(property) => {
                    let ty = property.read.ty;
                    let write = if property.write.has_setter() {
                        WriteCapability::ExtensionProperty(property.write.clone())
                    } else {
                        WriteCapability::ReadOnly
                    };
                    return Some(ResolvedPlacePlan {
                        read: property.read,
                        write,
                        ty,
                    });
                }
                crate::properties::ImplicitValueResolution::Value { target, .. } => {
                    selected_value = Some(target);
                }
                crate::properties::ImplicitValueResolution::NoApplicable(failure) => {
                    self.commit_layer_diagnostics(*failure);
                    return None;
                }
                crate::properties::ImplicitValueResolution::Failed => return None,
                crate::properties::ImplicitValueResolution::NoCandidate => {}
            }
        }
        let selected_value = match selected_value {
            Some(value) => Some(value),
            None => self.resolve_value_name(name).ok()?,
        };
        if let Some(value) = selected_value {
            if let crate::imports::lookup::values::ResolvedValueTarget::Materialized(
                crate::imports::lookup::values::ValueTarget::Property(property),
            ) = value
            {
                let ty = self.properties[property].ty;
                let (owner, receiver) = self.named_property_receiver(property, name.span)?.parts();
                return Some(ResolvedPlacePlan {
                    read: self.lower_property_read(
                        property,
                        owner,
                        receiver.clone(),
                        ty,
                        name.span,
                    )?,
                    write: if self.properties[property].capability.setter().is_some() {
                        WriteCapability::Property {
                            property,
                            owner,
                            receiver,
                        }
                    } else {
                        WriteCapability::ReadOnly
                    },
                    ty,
                });
            }
            let read = match value {
                crate::imports::lookup::values::ResolvedValueTarget::Materialized(value) => {
                    self.lower_named_value_target(name, value, None)?
                }
                crate::imports::lookup::values::ResolvedValueTarget::Dependency(binding) => {
                    return self.resolve_imported_property_place(binding, name, sink);
                }
            };
            return Some(ResolvedPlacePlan {
                ty: read.ty,
                read,
                write: WriteCapability::ReadOnly,
            });
        }
        let read = self.lower_var(name, sink, None)?;
        Some(ResolvedPlacePlan {
            ty: read.ty,
            read,
            write: WriteCapability::ReadOnly,
        })
    }
}
