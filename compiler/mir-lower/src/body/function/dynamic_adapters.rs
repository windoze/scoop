use super::*;

impl BodyLowerer<'_> {
    pub(in crate::body) fn adapt_checked_function_value(
        &mut self,
        value: smir::Expr,
        target: mir::FunctionTypeId,
        _span: Span,
    ) -> smir::Expr {
        let adapter = request_dynamic_adapter(
            self.module,
            self.shell,
            target,
            DynamicAdapterDefinitions {
                functions: self.functions,
                top_level: self.top_level,
                classes: self.closure_classes,
                invokes: self.closure_invokes,
                adapters: self.dynamic_closure_adapters,
                by_target: self.dynamic_adapter_by_target,
            },
        );
        smir::Expr::new(
            mir::Type::Function(target),
            smir::ExprKind::ClosureAlloc {
                class: self.dynamic_closure_adapters[adapter].class(),
                captures: vec![smir::ClosureCaptureInit::new(0, value)],
            },
        )
    }
}
