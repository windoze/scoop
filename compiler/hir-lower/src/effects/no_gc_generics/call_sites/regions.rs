use super::*;

impl Lowerer {
    pub(in crate::effects) fn generic_calls_in_body(&self, body: &hir::Body) -> Vec<GenericCall> {
        let mut out = Vec::new();
        self.collect_generic_calls_in_statements(&body.statements, &mut out);
        out
    }

    pub(in crate::effects) fn generic_calls_in_default(
        &self,
        default: &hir::ExportDefaultExpr,
    ) -> Vec<GenericCall> {
        let mut out = Vec::new();
        self.collect_generic_calls_in_statements(&default.statements, &mut out);
        self.collect_generic_calls_in_expr(&default.value, &mut out);
        out
    }

    pub(in crate::effects) fn generic_calls_in_struct_constructor(
        &self,
        constructor: &hir::StructConstructor,
    ) -> Vec<GenericCall> {
        let mut out = Vec::new();
        if let hir::StructConstructorKind::Secondary {
            delegation, body, ..
        } = &constructor.kind
        {
            out.extend(self.generic_struct_constructor_call(delegation.target, constructor.span));
            self.collect_generic_calls_in_constructor_arguments(&delegation.arguments, &mut out);
            self.collect_generic_calls_in_statements(&body.statements, &mut out);
        }
        out
    }

    pub(in crate::effects) fn generic_calls_in_class_constructor(
        &self,
        constructor: &hir::ClassConstructor,
    ) -> Vec<GenericCall> {
        let mut out = Vec::new();
        match &constructor.kind {
            hir::ClassConstructorKind::Primary {
                base,
                common_initialization,
                ..
            } => {
                self.collect_generic_calls_in_base_initialization(base, constructor.span, &mut out);
                self.collect_generic_calls_in_class_initialization(common_initialization, &mut out);
            }
            hir::ClassConstructorKind::Secondary { delegation, body } => {
                match delegation {
                    hir::ClassSecondaryDelegation::This { target, arguments } => {
                        out.extend(self.generic_class_constructor_call(*target, constructor.span));
                        self.collect_generic_calls_in_constructor_arguments(arguments, &mut out)
                    }
                    hir::ClassSecondaryDelegation::Terminal {
                        base,
                        common_initialization,
                    } => {
                        self.collect_generic_calls_in_base_initialization(
                            base,
                            constructor.span,
                            &mut out,
                        );
                        self.collect_generic_calls_in_class_initialization(
                            common_initialization,
                            &mut out,
                        );
                    }
                }
                self.collect_generic_calls_in_statements(&body.statements, &mut out);
            }
        }
        out
    }

    fn collect_generic_calls_in_constructor_arguments(
        &self,
        arguments: &hir::ConstructorArguments,
        out: &mut Vec<GenericCall>,
    ) {
        self.collect_generic_calls_in_statements(&arguments.statements, out);
        for argument in &arguments.args {
            self.collect_generic_calls_in_expr(argument, out);
        }
    }

    fn collect_generic_calls_in_base_initialization(
        &self,
        base: &hir::BaseInitialization,
        span: Span,
        out: &mut Vec<GenericCall>,
    ) {
        if let hir::BaseInitialization::Super { target, arguments } = base {
            if let hir::BaseInitializerTarget::Local(target) = target {
                out.extend(self.generic_class_constructor_call(*target, span));
            }
            self.collect_generic_calls_in_constructor_arguments(arguments, out);
        }
    }

    fn collect_generic_calls_in_class_initialization(
        &self,
        initialization: &[hir::ClassInitializationStep],
        out: &mut Vec<GenericCall>,
    ) {
        for step in initialization {
            match step {
                hir::ClassInitializationStep::StoredProperty { initializer, .. }
                | hir::ClassInitializationStep::DelegatedProperty { initializer, .. } => {
                    self.collect_generic_calls_in_statements(&initializer.statements, out);
                    self.collect_generic_calls_in_expr(&initializer.value, out);
                }
                hir::ClassInitializationStep::InitBlock { body, .. } => {
                    self.collect_generic_calls_in_statements(&body.statements, out);
                }
            }
        }
    }
}
