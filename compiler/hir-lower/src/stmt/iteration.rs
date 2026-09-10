use super::*;
use crate::expr::{CallSite, RequiredCallableModifiers};

impl Lowerer {
    pub(super) fn lower_for(&mut self, source: &ast::For) -> Option<hir::ForIterationPlan> {
        self.with_pattern_transaction(|state| state.lower_for_inner(source))
    }

    fn lower_for_inner(&mut self, source: &ast::For) -> Option<hir::ForIterationPlan> {
        let core = self
            .iteration_core
            .expect("source iteration is lowered only after core validation");
        let mut source_setup = Vec::new();
        let source_init = self.lower_expr(&source.iterable, &mut source_setup, None)?;
        let source_temporary = hir::BindingTemporary {
            local: self.alloc_desugared_iterator_hidden("for.source", source_init.ty),
            ty: source_init.ty,
        };
        let source_receiver = hir::Expr {
            kind: hir::ExprKind::Local(source_temporary.local),
            ty: source_temporary.ty,
            span: source.iterable.span(),
            origin: self.expression_origin(source.iterable.span()),
        };

        let mut iterator_setup = Vec::new();
        let iterator_call = self.lower_named_call_on_receiver(
            source_receiver,
            &ast::Ident {
                text: "iterator".to_string(),
                span: source.iterable.span(),
            },
            CallSite {
                type_args: &[],
                args: &[],
                span: source.iterable.span(),
            },
            &mut iterator_setup,
            None,
            RequiredCallableModifiers {
                operator: Some(hir::OperatorKind::Iterator),
                ..Default::default()
            },
        )?;

        let applications = self.exact_interface_applications(iterator_call.ty, core.iterator());
        let application = match applications.as_slice() {
            [application] => *application,
            [] => {
                let found = self.type_name(iterator_call.ty);
                self.error(
                    source.iterable.span(),
                    format!(
                        "iterator operator result type `{found}` does not implement `Iterator<T>`"
                    ),
                );
                return None;
            }
            _ => {
                let found = self.type_name(iterator_call.ty);
                self.error(
                    source.iterable.span(),
                    format!(
                        "iterator operator result type `{found}` implements multiple distinct `Iterator<T>` applications"
                    ),
                );
                return None;
            }
        };
        let iterator_application = self.interface_applications[application].clone();
        let [element_type] = iterator_application.arguments.as_slice() else {
            unreachable!("the checked Iterator core has one type parameter")
        };
        let element_type = *element_type;

        let iterator_result = hir::BindingTemporary {
            local: self.alloc_desugared_iterator_hidden("for.iterator.result", iterator_call.ty),
            ty: iterator_call.ty,
        };
        let iterator_type = iterator_application.canonical_type;
        let iterator = hir::BindingTemporary {
            local: self.alloc_desugared_iterator_hidden("for.iterator", iterator_type),
            ty: iterator_type,
        };
        let option_type = self.option_type(element_type);
        let Type::Enum(option_application) = self.types[option_type] else {
            unreachable!("Option<E> is an enum application")
        };
        let option = self
            .option_core
            .expect("source iteration is lowered only after Option validation");
        let some = hir::AppliedEnumVariantRef::checked(
            &self.enums,
            &self.enum_applications,
            option_application,
            option.some(),
        )
        .expect("the specialized Option application owns canonical Some");
        let some_payload = hir::AppliedEnumVariantFieldRef::checked(
            &self.enums,
            &self.enum_applications,
            some,
            option.some_payload().local_index(),
        )
        .expect("the specialized Some variant owns its canonical payload");
        let none = hir::AppliedEnumVariantRef::checked(
            &self.enums,
            &self.enum_applications,
            option_application,
            option.none(),
        )
        .expect("the specialized Option application owns canonical None");
        let applied_option = hir::AppliedOptionCore::checked(
            &self.enums,
            &self.enum_applications,
            &self.types,
            option,
            element_type,
            some_payload,
            none,
        )
        .expect("the specialized Option application has the canonical Some/None relation");
        let next_function = self.interface_method_entities[core.next()].function;
        let next_callable = self.record_method_application(
            next_function,
            hir::MethodOwnerApplication::Interface(application),
        );
        let next_result = hir::BindingTemporary {
            local: self.alloc_desugared_iterator_hidden("for.next", option_type),
            ty: option_type,
        };
        let element = hir::BindingTemporary {
            local: self.alloc_desugared_iterator_hidden("for.element", element_type),
            ty: element_type,
        };
        let target = self.fresh_loop();
        let origin = self.expression_origin(source.span);

        self.push_scope();
        let planned = (|| {
            let binding = self
                .lower_irrefutable_binding_plan_from_subject(&source.pattern, element, false)?
                .into_plan();
            self.loop_targets.push(target);
            let mut body = Vec::new();
            for statement in &source.body.statements {
                self.lower_statement(statement, &mut body);
            }
            assert_eq!(self.loop_targets.pop(), Some(target));
            Some((binding, body))
        })();
        self.pop_scope();
        let (binding, body) = planned?;

        Some(hir::ForIterationPlan::new(
            target,
            source_setup,
            source_temporary,
            source_init,
            iterator_setup,
            iterator_call,
            hir::IteratorConformanceWitness::new(
                iterator_result,
                iterator,
                application,
                source.span,
                origin,
            ),
            hir::IteratorNextPlan::new(
                next_callable,
                next_result,
                applied_option,
                element,
                source.span,
                origin,
            ),
            binding,
            body,
        ))
    }
}
