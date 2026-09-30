//! Storage reads and writes retain their selected field or global identity.

use super::*;

impl Lowerer {
    pub(super) fn property_storage_read(
        &mut self,
        declaration: &hir::Property,
        owner_application: Option<hir::MethodOwnerApplication>,
        receiver: Option<hir::Expr>,
        ty: TypeId,
        span: ast::Span,
    ) -> Option<hir::Expr> {
        let kind = match &declaration.representation {
            hir::PropertyRepresentation::Stored(stored) => match stored.backing {
                hir::PropertyBacking::TopLevelGlobal { storage, .. } => {
                    debug_assert!(receiver.is_none());
                    hir::ExprKind::GlobalRead(storage)
                }
                hir::PropertyBacking::ClassField { field, .. } => {
                    let receiver = receiver.expect("class storage access has a receiver");
                    let application = match owner_application {
                        Some(hir::MethodOwnerApplication::Class(application)) => application,
                        Some(hir::MethodOwnerApplication::Object(object)) => {
                            self.object_types[object].representation
                        }
                        _ => {
                            unreachable!(
                                "reference storage access has a reference owner application"
                            )
                        }
                    };
                    hir::ExprKind::FieldAccess {
                        receiver: Box::new(receiver),
                        field: self.class_field_reference(application, field),
                    }
                }
                hir::PropertyBacking::StructField { owner: _, index } => {
                    let receiver = receiver.expect("struct storage access has a receiver");
                    let Some(hir::MethodOwnerApplication::Struct(application)) = owner_application
                    else {
                        unreachable!("struct storage access has a struct owner application")
                    };
                    let field = self.struct_field_reference(application, index);
                    hir::ExprKind::FieldAccess {
                        receiver: Box::new(receiver),
                        field,
                    }
                }
            },
            hir::PropertyRepresentation::NativeStorage { storage } => {
                debug_assert!(receiver.is_none());
                if matches!(
                    self.globals[*storage].storage,
                    hir::GlobalStorage::Extern { .. }
                ) {
                    self.require_unsafe_operation(span, "reading an extern global");
                }
                hir::ExprKind::GlobalRead(*storage)
            }
            hir::PropertyRepresentation::AccessorOnly
            | hir::PropertyRepresentation::Delegated { .. }
            | hir::PropertyRepresentation::GenericDelegated { .. }
            | hir::PropertyRepresentation::Const { .. } => {
                unreachable!("only stored and native properties have storage accessors")
            }
        };
        Some(hir::Expr {
            kind,
            ty,
            span,
            origin: self.expression_origin(span),
        })
    }

    pub(super) fn property_storage_write(
        &mut self,
        declaration: &hir::Property,
        owner_application: Option<hir::MethodOwnerApplication>,
        receiver: Option<hir::Expr>,
        value: hir::Expr,
    ) -> Option<hir::StatementKind> {
        let target = match &declaration.representation {
            hir::PropertyRepresentation::Stored(stored) => match stored.backing {
                hir::PropertyBacking::TopLevelGlobal { storage, .. } => {
                    debug_assert!(receiver.is_none());
                    hir::AssignTarget::Global(storage)
                }
                hir::PropertyBacking::ClassField { field, .. } => {
                    let receiver = receiver.expect("class storage write has a receiver");
                    let application = match owner_application {
                        Some(hir::MethodOwnerApplication::Class(application)) => application,
                        Some(hir::MethodOwnerApplication::Object(object)) => {
                            self.object_types[object].representation
                        }
                        _ => {
                            unreachable!(
                                "reference storage write has a reference owner application"
                            )
                        }
                    };
                    hir::AssignTarget::Field {
                        receiver: Box::new(receiver),
                        field: self.class_field_reference(application, field),
                    }
                }
                hir::PropertyBacking::StructField { .. } => {
                    unreachable!("value-type stored properties are immutable")
                }
            },
            hir::PropertyRepresentation::NativeStorage { storage } => {
                debug_assert!(receiver.is_none());
                if matches!(
                    self.globals[*storage].storage,
                    hir::GlobalStorage::Extern { .. }
                ) {
                    self.require_unsafe_operation(value.span, "writing an extern global");
                }
                hir::AssignTarget::Global(*storage)
            }
            hir::PropertyRepresentation::AccessorOnly
            | hir::PropertyRepresentation::Delegated { .. }
            | hir::PropertyRepresentation::GenericDelegated { .. }
            | hir::PropertyRepresentation::Const { .. } => {
                unreachable!("only stored and native properties have storage setters")
            }
        };
        Some(hir::StatementKind::Assign { target, value })
    }
}
