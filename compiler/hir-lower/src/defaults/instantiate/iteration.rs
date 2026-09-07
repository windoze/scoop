use super::*;

impl Lowerer {
    pub(super) fn instantiate_default_for(
        &mut self,
        source: &hir::ForIterationPlan,
        context: &mut InstantiationContext,
    ) -> hir::ForIterationPlan {
        let source_setup = source
            .source_setup()
            .iter()
            .map(|statement| self.instantiate_default_statement(statement, context))
            .collect();
        let source_temporary = self.instantiate_binding_temporary(source.source(), context);
        let source_init = self.instantiate_default_expr(source.source_init(), context);
        let iterator_setup = source
            .iterator_setup()
            .iter()
            .map(|statement| self.instantiate_default_statement(statement, context))
            .collect();
        let iterator_call = self.instantiate_default_expr(source.iterator_call(), context);

        let source_conformance = source.conformance();
        let conformance_source =
            self.instantiate_binding_temporary(source_conformance.source(), context);
        let conformance_iterator =
            self.instantiate_binding_temporary(source_conformance.iterator(), context);
        let conformance_application = self
            .instantiate_default_interface_application(source_conformance.application(), context);
        let conformance = hir::IteratorConformanceWitness::new(
            conformance_source,
            conformance_iterator,
            conformance_application,
            source_conformance.span(),
            instantiate_origin(source_conformance.origin(), context.evaluation),
        );

        let source_next = source.next();
        let method = self.method_applications[source_next.callable()].clone();
        let method_owner = self.instantiate_default_method_owner(method.owner, context);
        let callable = self.record_method_application(method.function, method_owner);
        let result = self.instantiate_binding_temporary(source_next.result(), context);
        let element = self.instantiate_binding_temporary(source_next.element(), context);
        let source_option = source_next.option();
        let some_payload =
            self.instantiate_default_applied_enum_field(source_option.some_payload(), context);
        let none = self.instantiate_default_applied_enum_variant(source_option.none(), context);
        let option = hir::AppliedOptionCore::checked(
            &self.enums,
            &self.enum_applications,
            &self.types,
            self.option_core
                .expect("default iteration is instantiated after Option validation"),
            element.ty,
            some_payload,
            none,
        )
        .expect("default substitution preserves the checked Option specialization");
        let next = hir::IteratorNextPlan::new(
            callable,
            result,
            option,
            element,
            source_next.span(),
            instantiate_origin(source_next.origin(), context.evaluation),
        );

        let mapped_target = self.fresh_loop();
        context.loop_targets.push((source.target(), mapped_target));
        let binding = self.instantiate_default_binding_plan(source.binding(), context);
        let body = source
            .body()
            .iter()
            .map(|statement| self.instantiate_default_statement(statement, context))
            .collect();
        assert_eq!(
            context.loop_targets.pop(),
            Some((source.target(), mapped_target))
        );

        hir::ForIterationPlan::new(
            mapped_target,
            source_setup,
            source_temporary,
            source_init,
            iterator_setup,
            iterator_call,
            conformance,
            next,
            binding,
            body,
        )
    }

    fn instantiate_default_binding_plan(
        &mut self,
        source: &hir::IrrefutableBindingPlan,
        context: &mut InstantiationContext,
    ) -> hir::IrrefutableBindingPlan {
        hir::IrrefutableBindingPlan {
            subject: self.instantiate_binding_temporary(source.subject, context),
            shape: self.instantiate_default_binding_shape(&source.shape, context),
            actions: source
                .actions
                .iter()
                .map(|action| match action {
                    hir::IrrefutableBindingAction::Project {
                        source,
                        result,
                        projection,
                        span,
                        origin,
                    } => hir::IrrefutableBindingAction::Project {
                        source: self.instantiate_binding_temporary(*source, context),
                        result: self.instantiate_binding_temporary(*result, context),
                        projection: match *projection {
                            hir::BindingProjection::TupleIndex(index) => {
                                hir::BindingProjection::TupleIndex(index)
                            }
                            hir::BindingProjection::StructField(field) => {
                                let application = self.instantiate_default_struct_application(
                                    field.application(),
                                    context,
                                );
                                hir::BindingProjection::StructField(
                                    hir::AppliedStructFieldRef::checked(
                                        &self.structs,
                                        &self.struct_applications,
                                        application,
                                        field.local_index(),
                                    )
                                    .expect("default substitution preserves a binding field"),
                                )
                            }
                        },
                        span: *span,
                        origin: instantiate_origin(*origin, context.evaluation),
                    },
                    hir::IrrefutableBindingAction::Component {
                        source,
                        index,
                        result,
                        setup,
                        call,
                        span,
                    } => hir::IrrefutableBindingAction::Component {
                        source: self.instantiate_binding_temporary(*source, context),
                        index: *index,
                        result: self.instantiate_binding_temporary(*result, context),
                        setup: setup
                            .iter()
                            .map(|statement| self.instantiate_default_statement(statement, context))
                            .collect(),
                        call: self.instantiate_default_expr(call, context),
                        span: *span,
                    },
                    hir::IrrefutableBindingAction::Bind {
                        source,
                        target,
                        span,
                        origin,
                    } => hir::IrrefutableBindingAction::Bind {
                        source: self.instantiate_binding_temporary(*source, context),
                        target: self.instantiate_binding_leaf(*target, context),
                        span: *span,
                        origin: instantiate_origin(*origin, context.evaluation),
                    },
                })
                .collect(),
        }
    }

    fn instantiate_default_binding_shape(
        &mut self,
        source: &hir::IrrefutableBindingShape,
        context: &InstantiationContext,
    ) -> hir::IrrefutableBindingShape {
        match source {
            hir::IrrefutableBindingShape::Binding(leaf) => {
                hir::IrrefutableBindingShape::Binding(self.instantiate_binding_leaf(*leaf, context))
            }
            hir::IrrefutableBindingShape::Wildcard => hir::IrrefutableBindingShape::Wildcard,
            hir::IrrefutableBindingShape::Tuple(elements) => hir::IrrefutableBindingShape::Tuple(
                elements
                    .iter()
                    .map(|element| self.instantiate_default_binding_shape(element, context))
                    .collect(),
            ),
            hir::IrrefutableBindingShape::Struct {
                application,
                fields,
            } => {
                let application =
                    self.instantiate_default_struct_application(*application, context);
                hir::IrrefutableBindingShape::Struct {
                    application,
                    fields: fields
                        .iter()
                        .map(|(field, shape)| {
                            let field = hir::AppliedStructFieldRef::checked(
                                &self.structs,
                                &self.struct_applications,
                                application,
                                field.local_index(),
                            )
                            .expect("default substitution preserves a binding shape field");
                            (
                                field,
                                self.instantiate_default_binding_shape(shape, context),
                            )
                        })
                        .collect(),
                }
            }
            hir::IrrefutableBindingShape::Class {
                application,
                components,
            } => hir::IrrefutableBindingShape::Class {
                application: self.instantiate_default_class_application(*application, context),
                components: components
                    .iter()
                    .map(|(index, shape)| {
                        (
                            *index,
                            self.instantiate_default_binding_shape(shape, context),
                        )
                    })
                    .collect(),
            },
        }
    }

    fn instantiate_binding_temporary(
        &mut self,
        source: hir::BindingTemporary,
        context: &InstantiationContext,
    ) -> hir::BindingTemporary {
        hir::BindingTemporary {
            local: mapped_local(context, source.local),
            ty: self.instantiate_method_ty(source.ty, &context.bindings),
        }
    }

    fn instantiate_binding_leaf(
        &mut self,
        source: hir::BindingLeaf,
        context: &InstantiationContext,
    ) -> hir::BindingLeaf {
        hir::BindingLeaf {
            local: mapped_local(context, source.local),
            ty: self.instantiate_method_ty(source.ty, &context.bindings),
            mutability: source.mutability,
        }
    }
}
