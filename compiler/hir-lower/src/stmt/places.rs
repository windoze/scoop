use super::*;
use crate::expr::QualifiedInterfaceProperty;
use crate::expr::{CallSite, RequiredCallableModifiers};

mod names;

pub(super) struct ResolvedPlacePlan {
    pub(super) read: hir::Expr,
    pub(super) write: WriteCapability,
    pub(super) ty: TypeId,
}

pub(super) enum WriteCapability {
    ReadOnly,
    Direct(hir::AssignTarget),
    LocalDelegate {
        storage: hir::Expr,
        binding: hir::BindingId,
    },
    Property {
        property: hir::PropertyId,
        owner: Option<hir::MethodOwnerApplication>,
        receiver: Option<hir::Expr>,
    },
    ExtensionProperty(crate::properties::ResolvedExtensionProperty),
    DirectInterfaceProperty(QualifiedInterfaceProperty),
    ImportedDependencyProperty {
        binding: hir::DirectImportedTargetBinding,
        receiver: Option<hir::Expr>,
    },
    OperatorSet {
        receiver: hir::Expr,
        index_arguments: Vec<ast::CallArgument>,
    },
}

impl Lowerer {
    pub(crate) fn lower_update(
        &mut self,
        place: &ast::PlaceExpr,
        op: ast::UpdateOp,
        notation: ast::UpdateNotation,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let plan = self.resolve_place_plan(place, sink)?;
        if matches!(&plan.write, WriteCapability::ReadOnly) {
            self.error(span, "update operand is not writable".to_string());
            return None;
        }
        let old = self.materialize_place_expr(plan.read, "old", span, sink);
        let (kind, name) = match op {
            ast::UpdateOp::Increment => (hir::OperatorKind::Inc, "inc"),
            ast::UpdateOp::Decrement => (hir::OperatorKind::Dec, "dec"),
        };
        let name = ast::Ident {
            text: name.to_string(),
            span,
        };
        let value = self.lower_named_call_on_receiver(
            old.clone(),
            &name,
            CallSite {
                type_args: &[],
                args: &[],
                span,
            },
            sink,
            Some(plan.ty),
            RequiredCallableModifiers {
                operator: Some(kind),
                infix: false,
                ..Default::default()
            },
        )?;
        if !self.is_subtype(value.ty, plan.ty) {
            self.error(
                span,
                format!(
                    "operator `{name}` returns {}, which cannot be assigned to update operand of type {}",
                    self.type_name(value.ty),
                    self.type_name(plan.ty),
                    name = name.text,
                ),
            );
            return None;
        }
        let value = self.adapt_to(value, plan.ty);
        let new = self.materialize_place_expr(value, "new", span, sink);
        let write = self.lower_place_write(plan.write, new.clone(), span, sink)?;
        sink.push(hir::Statement { kind: write, span });
        Some(match notation {
            ast::UpdateNotation::Prefix => new,
            ast::UpdateNotation::Postfix => old,
        })
    }

    pub(super) fn materialize_place_expr(
        &mut self,
        value: hir::Expr,
        label: &str,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> hir::Expr {
        let ty = value.ty;
        let local = self.alloc_hidden(label, ty);
        sink.push(hir::Statement {
            kind: hir::StatementKind::ValDecl {
                pattern: hir::Pattern::Binding { local },
                init: value,
            },
            span,
        });
        hir::Expr {
            kind: hir::ExprKind::Local(local),
            ty,
            span,
            origin: self.expression_origin(span),
        }
    }

    pub(super) fn resolve_place_plan(
        &mut self,
        place: &ast::PlaceExpr,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<ResolvedPlacePlan> {
        match place {
            ast::PlaceExpr::Name(name) => self.resolve_named_place_plan(name, sink),
            ast::PlaceExpr::Field {
                receiver,
                name,
                span,
            } => {
                if matches!(&**receiver, ast::Expr::This { .. })
                    && self.initialization_context.is_some()
                {
                    let field = self.initializing_field(name, *span)?;
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
                if let Some(property) = self.qualified_object_const_property(receiver, &name.text) {
                    let ty = self.properties[property].ty;
                    let read = self.lower_property_read(property, None, None, ty, *span)?;
                    return Some(ResolvedPlacePlan {
                        read,
                        write: WriteCapability::ReadOnly,
                        ty,
                    });
                }
                let forwarding = self
                    .nominal_qualifier_target(receiver)
                    .and_then(|host| self.companion_forwarding_property_object(host, &name.text));
                let receiver = match forwarding {
                    Some(companion) => self.lower_singleton_value(companion, receiver.span())?,
                    None => self.lower_expr(receiver, sink, None)?,
                };
                let receiver = self.materialize_place_expr(receiver, "place", *span, sink);
                if let Some((property, owner, ty)) =
                    self.find_accessible_nominal_property(receiver.ty, &name.text)
                {
                    let read = self.lower_property_read(
                        property,
                        Some(owner),
                        Some(receiver.clone()),
                        ty,
                        *span,
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
                match self.resolve_extension_property(receiver.clone(), name, sink, true) {
                    crate::properties::ExtensionPropertyResolution::Resolved(property) => {
                        let ty = property.read.ty;
                        let write = if self.properties[property.property]
                            .capability
                            .setter()
                            .is_some()
                        {
                            WriteCapability::ExtensionProperty(*property.clone())
                        } else {
                            WriteCapability::ReadOnly
                        };
                        Some(ResolvedPlacePlan {
                            read: property.read,
                            write,
                            ty,
                        })
                    }
                    crate::properties::ExtensionPropertyResolution::Failed => None,
                    crate::properties::ExtensionPropertyResolution::NoCandidate => {
                        if self.is_value_ty(receiver.ty) {
                            self.error(
                                name.span,
                                "field assignment is not supported (value types are immutable)"
                                    .to_string(),
                            );
                        } else {
                            let found = self.type_name(receiver.ty);
                            self.error(name.span, format!("type `{found}` has no fields"));
                        }
                        None
                    }
                }
            }
            ast::PlaceExpr::Index {
                receiver,
                indices,
                span,
            } => {
                let receiver = self.lower_expr(receiver, sink, None)?;
                let receiver = self.materialize_place_expr(receiver, "place", *span, sink);
                let arguments = indices
                    .iter()
                    .cloned()
                    .map(ast::CallArgument::positional)
                    .collect::<Vec<_>>();
                let read = self.lower_named_call_on_receiver(
                    receiver.clone(),
                    &ast::Ident {
                        text: "get".to_string(),
                        span: *span,
                    },
                    CallSite {
                        type_args: &[],
                        args: &arguments,
                        span: *span,
                    },
                    sink,
                    None,
                    RequiredCallableModifiers {
                        operator: Some(hir::OperatorKind::Get),
                        infix: false,
                        ..Default::default()
                    },
                )?;
                let index_arguments = self.materialized_source_arguments(indices.len(), *span);
                Some(ResolvedPlacePlan {
                    ty: read.ty,
                    read,
                    write: WriteCapability::OperatorSet {
                        receiver,
                        index_arguments,
                    },
                })
            }
            ast::PlaceExpr::QualifiedInterfaceSuperProperty {
                qualifier,
                name,
                span,
            } => {
                let property =
                    self.resolve_qualified_interface_super_property(qualifier, name, *span)?;
                let read = self.lower_direct_interface_property_read(property.clone(), *span)?;
                let write = if self.properties[property.property]
                    .capability
                    .setter()
                    .is_some()
                {
                    WriteCapability::DirectInterfaceProperty(property)
                } else {
                    WriteCapability::ReadOnly
                };
                Some(ResolvedPlacePlan {
                    ty: read.ty,
                    read,
                    write,
                })
            }
        }
    }

    fn materialized_source_arguments(
        &mut self,
        count: usize,
        span: Span,
    ) -> Vec<ast::CallArgument> {
        let locals = (0..count)
            .map(|index| {
                let name = format!("$argument.{index}");
                self.locals
                    .iter()
                    .filter_map(|(local, declaration)| (declaration.name == name).then_some(local))
                    .next_back()
                    .expect("a committed source call materializes every explicit input")
            })
            .collect::<Vec<_>>();
        locals
            .into_iter()
            .map(|local| self.local_source_argument(local, span))
            .collect()
    }

    fn local_source_argument(&mut self, local: hir::LocalId, span: Span) -> ast::CallArgument {
        let name = format!("$place.source.{}", self.hidden_count);
        self.hidden_count += 1;
        self.scopes.declare(name.clone(), local);
        ast::CallArgument::positional(ast::Expr::Var(ast::Ident { text: name, span }))
    }

    pub(super) fn lower_place_write(
        &mut self,
        write: WriteCapability,
        value: hir::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::StatementKind> {
        match write {
            WriteCapability::ReadOnly => {
                self.error(
                    span,
                    "compound-assignment fallback is not writable".to_string(),
                );
                None
            }
            WriteCapability::Direct(target) => Some(hir::StatementKind::Assign { target, value }),
            WriteCapability::LocalDelegate { storage, binding } => self
                .local_delegate_write(storage, binding, value, span)
                .map(hir::StatementKind::Expr),
            WriteCapability::Property {
                property,
                owner,
                receiver,
            } => self.lower_property_write(property, owner, receiver, value, span),
            WriteCapability::ExtensionProperty(property) => {
                self.lower_extension_property_write(property, value, span)
            }
            WriteCapability::DirectInterfaceProperty(property) => {
                self.lower_direct_interface_property_write(property, value, span)
            }
            WriteCapability::ImportedDependencyProperty { binding, receiver } => {
                self.lower_imported_dependency_property_write(&binding, receiver, value, span)
            }
            WriteCapability::OperatorSet {
                receiver,
                mut index_arguments,
            } => {
                let value = self.materialize_place_expr(value, "write", span, sink);
                let hir::ExprKind::Local(value_local) = value.kind else {
                    unreachable!("place values are materialized as local reads")
                };
                index_arguments.push(self.local_source_argument(value_local, span));
                let call = self.lower_named_call_on_receiver(
                    receiver,
                    &ast::Ident {
                        text: "set".to_string(),
                        span,
                    },
                    CallSite {
                        type_args: &[],
                        args: &index_arguments,
                        span,
                    },
                    sink,
                    Some(self.unit),
                    RequiredCallableModifiers {
                        operator: Some(hir::OperatorKind::Set),
                        infix: false,
                        ..Default::default()
                    },
                )?;
                Some(hir::StatementKind::Expr(call))
            }
        }
    }
}
