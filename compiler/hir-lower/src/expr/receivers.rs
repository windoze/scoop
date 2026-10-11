use super::*;
use crate::{Owner, ReceiverBinding, ReceiverScope};

impl Lowerer {
    pub(crate) fn check_initializing_receiver_label(
        &mut self,
        label: Option<&ast::Ident>,
    ) -> Option<()> {
        let Some(label) = label else { return Some(()) };
        if self.loop_label_is_active(&label.text) {
            self.error(
                label.span,
                format!("label `{}` has no receiver", label.text),
            );
            return None;
        }
        if self
            .current_owner
            .is_some_and(|owner| self.receiver_owner_label(owner) == label.text)
        {
            return Some(());
        }
        self.error(
            label.span,
            format!("unknown receiver label `{}`", label.text),
        );
        None
    }

    pub(crate) fn push_receiver_scope(
        &mut self,
        label: Option<String>,
        receiver: Option<ReceiverBinding>,
    ) {
        self.receiver_scopes.push(ReceiverScope { label, receiver });
    }

    pub(crate) fn bind_callable_receiver(
        &mut self,
        ty: TypeId,
        label: Option<String>,
        span: Span,
    ) -> hir::Param {
        let local = self.alloc_this_local(ty, span);
        self.scopes.declare("this".to_owned(), local);
        self.current_this = Some((local, ty));
        self.push_receiver_scope(
            label,
            Some(ReceiverBinding {
                binding: self.locals[local].binding,
                ty,
            }),
        );
        hir::Param {
            name: "this".to_owned(),
            ty,
            local,
        }
    }

    pub(crate) fn receiver_owner_label(&self, owner: Owner) -> String {
        match owner {
            Owner::Class(id) => self.classes[id].name.clone(),
            Owner::Interface(id) => self.interfaces[id].name.clone(),
            Owner::Struct(id) => self.structs[id].name.clone(),
            Owner::Enum(id) => self.enums[id].name.clone(),
            Owner::Object(id) => self.objects[id].name.clone(),
        }
    }

    pub(crate) fn implicit_receivers(&self) -> Vec<ReceiverBinding> {
        let mut result = Vec::new();
        for receiver in self
            .receiver_scopes
            .iter()
            .rev()
            .filter_map(|scope| scope.receiver)
        {
            if !result
                .iter()
                .any(|previous: &ReceiverBinding| previous.binding == receiver.binding)
            {
                result.push(receiver);
            }
        }
        if result.is_empty() {
            if let Some((local, ty)) = self.current_this {
                result.push(ReceiverBinding {
                    binding: self.locals[local].binding,
                    ty,
                });
            } else if let Some(capture) = self.available_capture("this") {
                result.push(ReceiverBinding {
                    binding: capture.binding,
                    ty: capture.ty,
                });
            }
        }
        result
    }

    pub(crate) fn lower_receiver_binding(
        &mut self,
        receiver: ReceiverBinding,
        span: Span,
    ) -> Option<hir::Expr> {
        let local = self
            .locals
            .iter()
            .find_map(|(id, local)| (local.binding == receiver.binding).then_some(id));
        if let Some(local) = local {
            return Some(hir::Expr {
                kind: ExprKind::Local(local),
                ty: receiver.ty,
                span,
                origin: self.expression_origin(span),
            });
        }
        if !self.capture_contexts.is_empty() {
            return self.lower_capture_binding(receiver.binding, "this", span);
        }
        self.lower_local_type_capture(&crate::CaptureKey::Receiver(receiver.binding), span)
    }

    pub(super) fn lower_labelled_this(
        &mut self,
        label: &ast::Ident,
        span: Span,
    ) -> Option<hir::Expr> {
        let scope = self
            .receiver_scopes
            .iter()
            .rev()
            .find(|scope| scope.label.as_deref() == Some(&label.text));
        if self.loop_label_is_active(&label.text)
            || scope.is_some_and(|scope| scope.receiver.is_none())
        {
            self.error(
                label.span,
                format!("label `{}` has no receiver", label.text),
            );
            return None;
        }
        let Some(receiver) = scope.and_then(|scope| scope.receiver) else {
            self.error(
                label.span,
                format!("unknown receiver label `{}`", label.text),
            );
            return None;
        };
        self.lower_receiver_binding(receiver, span)
    }

    pub(crate) fn lower_current_this(&mut self, span: Span) -> Option<hir::Expr> {
        if let Some((local, ty)) = self.current_this {
            return Some(hir::Expr {
                kind: ExprKind::Local(local),
                ty,
                span,
                origin: self.expression_origin(span),
            });
        }
        if self.available_capture("this").is_some() {
            return self.lower_capture(&ast::Ident {
                text: "this".to_string(),
                span,
            });
        }
        None
    }

    pub(crate) fn current_this_ty(&self) -> Option<TypeId> {
        self.current_this
            .map(|(_, ty)| ty)
            .or_else(|| self.available_capture("this").map(|capture| capture.ty))
    }
}
