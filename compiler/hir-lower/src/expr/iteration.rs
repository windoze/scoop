use super::*;
use crate::expr::named_calls::imported_dependency::{ImportedMemberReceiver, ImportedProbeCall};

impl Lowerer {
    /// Invoke the checked core slot on an already adapted Iterator<T> receiver.
    pub(crate) fn lower_iteration_next(
        &mut self,
        receiver: hir::Expr,
        result_type: TypeId,
    ) -> Option<hir::Expr> {
        let span = receiver.span;
        match &self.core {
            crate::CoreLoweringAuthority::Defined => {
                let Type::Interface(application) = self.types[receiver.ty] else {
                    unreachable!("iteration uses the checked core interface application")
                };
                let core = self.iteration_core.expect("the defining core was checked");
                let function = self.interface_method_entities[core.next()].function;
                let callable = self.record_method_application(
                    function,
                    hir::MethodOwnerApplication::Interface(application),
                );
                Some(hir::Expr {
                    kind: ExprKind::MethodCall {
                        receiver: Box::new(receiver),
                        callee: hir::MethodCallee::Callable(hir::Callable::Method(callable).into()),
                        args: Vec::new(),
                    },
                    ty: result_type,
                    span,
                    origin: self.expression_origin(span),
                })
            }
            crate::CoreLoweringAuthority::Imported(core) => {
                let protocol = core.iteration();
                let candidate = self
                    .dependencies
                    .as_ref()
                    .expect("an imported core has a dependency view")
                    .callable_for_slot(
                        hir::SourceNominalId::GenericTemplate(protocol.iterator().persistent()),
                        protocol.next_dispatch().persistent(),
                    );
                let candidate = match candidate {
                    Ok(Some(candidate)) => candidate,
                    Ok(None) => {
                        self.error(span, "missing imported Iterator.next declaration".into());
                        return None;
                    }
                    Err(error) => {
                        self.error(span, format!("invalid imported Iterator.next: {error}"));
                        return None;
                    }
                };
                let name = ast::Ident {
                    text: "next".into(),
                    span,
                };
                let probe = match self.probe_imported_member_callable(
                    candidate,
                    ImportedMemberReceiver::Value(receiver),
                    &name,
                    ImportedProbeCall::lowered(&[], span),
                    Some(result_type),
                    false,
                ) {
                    Ok(probe) => probe,
                    Err(failure) => {
                        *self = *failure;
                        return None;
                    }
                };
                self.commit_imported_lowered_callable(probe)
            }
        }
    }
}
