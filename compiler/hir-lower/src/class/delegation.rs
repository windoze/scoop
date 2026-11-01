//! Interface delegates are ordinary fields initialized before body properties.

use super::*;

#[derive(Clone)]
pub(crate) struct InterfaceDelegateSource {
    pub field: hir::ClassFieldId,
    pub interface: TypeId,
    expression: ast::Expr,
    pub span: ast::Span,
}

impl InterfaceDelegateSource {
    pub(crate) fn expression_span(&self) -> ast::Span {
        self.expression.span()
    }
}

impl Lowerer {
    pub(super) fn declare_interface_delegate(
        &mut self,
        owner: ClassId,
        interface: TypeId,
        ordinal: u32,
        expression: &ast::Expr,
        span: ast::Span,
    ) {
        let field = self.allocate_class_field(
            owner,
            hir::Field {
                name: format!("$interfaceDelegate.{ordinal}"),
                ty: interface,
            },
            hir::ClassFieldSource::InterfaceDelegate { ordinal },
            span,
        );
        self.classes[owner].fields.push(field);
        self.interface_delegates
            .entry(owner)
            .or_default()
            .push(InterfaceDelegateSource {
                field,
                interface,
                expression: expression.clone(),
                span,
            });
    }

    pub(in crate::class) fn lower_interface_delegate_initialization(
        &mut self,
        owner: ClassId,
        constructor: hir::ClassConstructorId,
        primary_parameters_visible: bool,
    ) -> Vec<hir::ClassInitializationStep> {
        let delegates = self
            .interface_delegates
            .get(&owner)
            .cloned()
            .unwrap_or_default();
        let mut steps = Vec::new();
        for delegate in &delegates {
            let initializer = self.with_constructor_source_body(
                constructor,
                hir::EditingConstructionBody::InterfaceDelegate(delegate.field),
                "interface delegate initializer",
                hir::Safety::Safe,
                |this, sink| {
                    if !primary_parameters_visible {
                        this.constructor_params_in_scope.clear();
                    }
                    let value =
                        this.lower_expr(&delegate.expression, sink, Some(delegate.interface))?;
                    if !this.is_subtype(value.ty, delegate.interface) {
                        this.error(
                            delegate.expression.span(),
                            format!(
                                "interface delegate must have type {}, found {}",
                                this.type_name(delegate.interface),
                                this.type_name(value.ty),
                            ),
                        );
                        return None;
                    }
                    Some(this.adapt_to(value, delegate.interface))
                },
            );
            if let Some(initializer) = initializer {
                steps.push(hir::ClassInitializationStep::Field {
                    field: self.initializing_class_field_reference(
                        self.classes[owner].self_application,
                        delegate.field,
                    ),
                    initializer: hir::ConstructorExpression {
                        locals: initializer.locals,
                        statements: initializer.statements,
                        value: initializer.value,
                    },
                    span: delegate.span,
                });
                if let Some(crate::InitializationContext {
                    receiver: crate::InitializingReceiver::Class { initialized, .. },
                    ..
                }) = &mut self.initialization_context
                {
                    initialized.insert(delegate.field);
                }
            }
        }
        steps
    }
}
