use super::*;
use crate::argument_materialization::PlaceIndexInputs;

impl Lowerer {
    pub(super) fn resolve_index_place(
        &mut self,
        receiver: &ast::Expr,
        indices: &ast::NonEmptyVec<ast::Expr>,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<ResolvedPlacePlan> {
        let receiver = self.lower_expr(receiver, sink, None)?;
        let receiver = self.materialize_place_expr(receiver, "place", span, sink);
        let arguments = indices
            .iter()
            .cloned()
            .map(ast::CallArgument::positional)
            .collect::<Vec<_>>();
        let outer = self.place_index_inputs.replace(PlaceIndexInputs {
            call_span: span,
            locals: Vec::new(),
        });
        let read = self.lower_named_call_on_receiver(
            receiver.clone(),
            &ast::Ident {
                text: "get".to_string(),
                span,
            },
            CallSite {
                type_args: &[],
                args: &arguments,
                span,
            },
            sink,
            None,
            RequiredCallableModifiers {
                operator: Some(hir::OperatorKind::Get),
                infix: false,
                ..Default::default()
            },
        );
        let inputs = std::mem::replace(&mut self.place_index_inputs, outer)
            .expect("index lowering retains its source-input context");
        let read = read?;
        assert_eq!(inputs.locals.len(), indices.len());
        let index_arguments = inputs
            .locals
            .into_iter()
            .map(|local| self.local_source_argument(local, span))
            .collect();
        Some(ResolvedPlacePlan {
            ty: read.ty,
            read,
            write: WriteCapability::OperatorSet {
                receiver,
                index_arguments,
            },
        })
    }
}
