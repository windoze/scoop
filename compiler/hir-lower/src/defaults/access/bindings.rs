use scoop_hir as hir;

use super::ReferenceCollector;

impl ReferenceCollector<'_> {
    pub(super) fn for_plan(&mut self, plan: &hir::ForIterationPlan, origin: hir::DefinitionOrigin) {
        self.statements(plan.source_setup());
        self.binding_temporary(plan.source(), origin);
        self.expression(plan.source_init());
        self.statements(plan.iterator_setup());
        self.expression(plan.iterator_call());

        let conformance = plan.conformance();
        let conformance_origin = conformance.origin().definition();
        self.binding_temporary(conformance.source(), conformance_origin);
        self.binding_temporary(conformance.iterator(), conformance_origin);
        let interface =
            self.lowerer.interface_applications[conformance.application()].canonical_type;
        self.type_reference(interface, conformance_origin);

        let next = plan.next();
        let next_origin = next.origin().definition();
        self.callable_use(hir::Callable::Method(next.callable()), next_origin);
        self.binding_temporary(next.result(), next_origin);
        self.variant_field_shape(next.option().some_payload(), next_origin);
        self.constructor_use(
            hir::ExportDefaultConstructorTarget::Variant(next.option().none()),
            next_origin,
        );
        self.binding_temporary(next.element(), next_origin);

        self.binding_plan(plan.binding(), origin);
        self.statements(plan.body());
    }

    fn binding_plan(&mut self, plan: &hir::IrrefutableBindingPlan, origin: hir::DefinitionOrigin) {
        self.binding_temporary(plan.subject, origin);
        self.binding_shape(&plan.shape, origin);
        for action in &plan.actions {
            self.binding_action(action);
        }
    }

    fn binding_shape(
        &mut self,
        shape: &hir::IrrefutableBindingShape,
        origin: hir::DefinitionOrigin,
    ) {
        match shape {
            hir::IrrefutableBindingShape::Binding(leaf) => {
                self.binding_leaf(*leaf, origin);
            }
            hir::IrrefutableBindingShape::Wildcard => {}
            hir::IrrefutableBindingShape::Tuple(elements) => {
                for element in elements {
                    self.binding_shape(element, origin);
                }
            }
            hir::IrrefutableBindingShape::Struct {
                application,
                fields,
            } => {
                let owner = self.lowerer.struct_applications[*application].canonical_type;
                self.type_reference(owner, origin);
                for (_, field) in fields {
                    self.binding_shape(field, origin);
                }
            }
            hir::IrrefutableBindingShape::Class {
                application,
                components,
            } => {
                let owner = self.lowerer.class_applications[*application].canonical_type;
                self.type_reference(owner, origin);
                for (_, component) in components {
                    self.binding_shape(component, origin);
                }
            }
        }
    }

    fn binding_action(&mut self, action: &hir::IrrefutableBindingAction) {
        match action {
            hir::IrrefutableBindingAction::Project {
                source,
                result,
                projection,
                origin,
                ..
            } => {
                let origin = origin.definition();
                self.binding_temporary(*source, origin);
                self.binding_temporary(*result, origin);
                if let hir::BindingProjection::StructField(field) = projection {
                    self.field_use(hir::FieldRef::StructField(*field), origin);
                }
            }
            hir::IrrefutableBindingAction::Component {
                source,
                result,
                setup,
                call,
                span,
                ..
            } => {
                let origin = self.at(*span);
                self.binding_temporary(*source, origin);
                self.binding_temporary(*result, origin);
                self.statements(setup);
                self.expression(call);
            }
            hir::IrrefutableBindingAction::Bind {
                source,
                target,
                origin,
                ..
            } => {
                let origin = origin.definition();
                self.binding_temporary(*source, origin);
                self.binding_leaf(*target, origin);
            }
        }
    }

    fn binding_temporary(
        &mut self,
        temporary: hir::BindingTemporary,
        origin: hir::DefinitionOrigin,
    ) {
        self.type_reference(temporary.ty, origin);
    }

    fn binding_leaf(&mut self, leaf: hir::BindingLeaf, origin: hir::DefinitionOrigin) {
        self.type_reference(leaf.ty, origin);
    }
}
