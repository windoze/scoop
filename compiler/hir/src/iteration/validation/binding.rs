use std::collections::HashSet;

use super::*;

impl Validator<'_> {
    pub(super) fn validate_binding_plan(
        &mut self,
        plan: &IrrefutableBindingPlan,
        reserved: &HashSet<u32>,
    ) -> Check {
        self.check_temporary(plan.subject, "binding subject temporary")?;
        let mut defined = reserved.clone();
        self.require_defined(plan.subject, &defined)?;
        self.validate_binding_actions(&plan.actions, &mut defined)?;

        let mut consumed = vec![false; plan.actions.len()];
        self.validate_binding_shape(&plan.shape, plan.subject, &plan.actions, &mut consumed)?;
        if consumed.iter().any(|consumed| !consumed) {
            return fail("binding plan contains actions outside its checked shape");
        }
        Ok(())
    }

    /// Validate the runtime schedule in its independent source order. Shape
    /// validation happens afterwards because named struct patterns retain a
    /// declaration-order shape while their actions deliberately do not.
    fn validate_binding_actions(
        &mut self,
        actions: &[IrrefutableBindingAction],
        defined: &mut HashSet<u32>,
    ) -> Check {
        for action in actions {
            match action {
                IrrefutableBindingAction::Project {
                    source,
                    result,
                    projection,
                    ..
                } => {
                    self.require_defined(*source, defined)?;
                    self.check_temporary(*result, "binding projection result temporary")?;
                    let expected = self.projection_result_type(*source, *projection)?;
                    if result.ty != expected || !defined.insert(result.local.into_raw().into_u32())
                    {
                        return fail("binding projection action has an invalid result");
                    }
                }
                IrrefutableBindingAction::Component {
                    source,
                    index,
                    result,
                    setup,
                    call,
                    ..
                } => {
                    self.require_defined(*source, defined)?;
                    self.check_temporary(*result, "binding component result temporary")?;
                    if result.ty != call.ty || defined.contains(&result.local.into_raw().into_u32())
                    {
                        return fail("binding component action has an invalid result");
                    }
                    self.validate_statements(setup)?;
                    let receiver = self.validate_receiver_setup(
                        setup,
                        *source,
                        defined,
                        "component receiver setup",
                    )?;
                    if receiver.contains(&result.local.into_raw().into_u32()) {
                        return fail("component setup aliases its result temporary");
                    }
                    self.validate_protocol_call(
                        call,
                        &receiver,
                        OperatorKind::Component { index: *index },
                        "component call",
                    )?;
                    if !defined.insert(result.local.into_raw().into_u32()) {
                        return fail("binding component result reuses an already defined local");
                    }
                }
                IrrefutableBindingAction::Bind { source, target, .. } => {
                    self.require_defined(*source, defined)?;
                    self.check_leaf(*target)?;
                    if source.ty != target.ty {
                        return fail("binding leaf type differs from its source");
                    }
                    if target.mutability != BindingMutability::Immutable {
                        return fail("for binding leaf must be immutable");
                    }
                    if !defined.insert(target.local.into_raw().into_u32()) {
                        return fail("binding leaf reuses an already defined local");
                    }
                }
            }
        }
        Ok(())
    }

    fn projection_result_type(
        &self,
        source: BindingTemporary,
        projection: BindingProjection,
    ) -> Check<TypeId> {
        match projection {
            BindingProjection::TupleIndex(index) => {
                let Type::Tuple(elements) = checked_arena(&self.module.types, source.ty)
                    .ok_or_else(|| invalid("tuple projection source has an invalid type"))?
                else {
                    return fail("tuple projection has a non-tuple source");
                };
                elements
                    .get(index as usize)
                    .copied()
                    .ok_or_else(|| invalid("tuple projection index is out of bounds"))
            }
            BindingProjection::StructField(field) => {
                let checked = AppliedStructFieldRef::checked(
                    &self.module.structs,
                    &self.module.struct_applications,
                    field.application(),
                    field.local_index(),
                )
                .ok_or_else(|| invalid("struct binding projection field is invalid"))?;
                if checked != field {
                    return fail("struct binding projection field is not canonical");
                }
                let application = self.checked_struct_application(field.application())?;
                if application.canonical_type != source.ty {
                    return fail("struct binding projection differs from its source type");
                }
                let declaration = &self.module.structs[application.template];
                let field = declaration
                    .semantic_fields()
                    .get(field.local_index() as usize)
                    .ok_or_else(|| invalid("struct binding projection field is out of bounds"))?;
                let bindings = declaration
                    .type_params
                    .iter()
                    .zip(&application.arguments)
                    .map(|(parameter, argument)| (parameter.id, *argument))
                    .collect::<Vec<_>>();
                self.instantiate_type(field.ty, &bindings)
            }
        }
    }

    fn validate_binding_shape(
        &mut self,
        shape: &IrrefutableBindingShape,
        source: BindingTemporary,
        actions: &[IrrefutableBindingAction],
        consumed: &mut [bool],
    ) -> Check {
        match shape {
            IrrefutableBindingShape::Binding(leaf) => {
                self.check_leaf(*leaf)?;
                let index = self.unique_action_index(
                    actions,
                    |action| {
                        matches!(
                            action,
                            IrrefutableBindingAction::Bind {
                                source: action_source,
                                target,
                                ..
                            } if *action_source == source && target == leaf
                        )
                    },
                    "binding shape is missing its exact Bind action",
                )?;
                self.consume_action(consumed, index)?;
            }
            IrrefutableBindingShape::Wildcard => {}
            IrrefutableBindingShape::Tuple(elements) => {
                let Type::Tuple(element_types) = checked_arena(&self.module.types, source.ty)
                    .ok_or_else(|| invalid("tuple binding source has an invalid type"))?
                else {
                    return fail("tuple binding shape has a non-tuple source");
                };
                if element_types.len() != elements.len() {
                    return fail("tuple binding shape does not cover every element");
                }
                let element_types = element_types.clone();
                for (index, (shape, ty)) in elements.iter().zip(element_types).enumerate() {
                    if matches!(shape, IrrefutableBindingShape::Wildcard) {
                        continue;
                    }
                    let projected = self.take_projection_action(
                        actions,
                        consumed,
                        source,
                        BindingProjection::TupleIndex(index as u32),
                        ty,
                    )?;
                    self.validate_binding_shape(shape, projected, actions, consumed)?;
                }
            }
            IrrefutableBindingShape::Struct {
                application,
                fields,
            } => {
                let application_value = self.checked_struct_application(*application)?;
                if application_value.canonical_type != source.ty {
                    return fail("struct binding shape application differs from its source type");
                }
                let declaration = &self.module.structs[application_value.template];
                let declared_fields = declaration.semantic_fields();
                if fields.len() != declared_fields.len() {
                    return fail("struct binding shape does not cover every declared field");
                }
                let bindings = declaration
                    .type_params
                    .iter()
                    .zip(&application_value.arguments)
                    .map(|(parameter, argument)| (parameter.id, *argument))
                    .collect::<Vec<_>>();
                for (index, ((field, shape), declaration_field)) in
                    fields.iter().zip(declared_fields).enumerate()
                {
                    let checked = AppliedStructFieldRef::checked(
                        &self.module.structs,
                        &self.module.struct_applications,
                        *application,
                        index as u32,
                    )
                    .ok_or_else(|| invalid("struct binding field is invalid"))?;
                    if *field != checked {
                        return fail("struct binding fields are not in declaration order");
                    }
                    if matches!(shape, IrrefutableBindingShape::Wildcard) {
                        continue;
                    }
                    let field_ty = self.instantiate_type(declaration_field.ty, &bindings)?;
                    let projected = self.take_projection_action(
                        actions,
                        consumed,
                        source,
                        BindingProjection::StructField(*field),
                        field_ty,
                    )?;
                    self.validate_binding_shape(shape, projected, actions, consumed)?;
                }
            }
            IrrefutableBindingShape::Class {
                application,
                components,
            } => {
                let application_value = self.checked_class_application(*application)?;
                if application_value.canonical_type != source.ty {
                    return fail("class binding shape application differs from its source type");
                }
                for (position, (index, shape)) in components.iter().enumerate() {
                    if index.get() as usize != position + 1 {
                        return fail("class binding component indices are not contiguous");
                    }
                    let component =
                        self.take_component_action(actions, consumed, source, *index)?;
                    self.validate_binding_shape(shape, component, actions, consumed)?;
                }
            }
        }
        Ok(())
    }

    fn take_projection_action(
        &self,
        actions: &[IrrefutableBindingAction],
        consumed: &mut [bool],
        source: BindingTemporary,
        projection: BindingProjection,
        result_ty: TypeId,
    ) -> Check<BindingTemporary> {
        let index = self.unique_action_index(
            actions,
            |action| {
                matches!(
                    action,
                    IrrefutableBindingAction::Project {
                        source: action_source,
                        projection: action_projection,
                        ..
                    } if *action_source == source && *action_projection == projection
                )
            },
            "binding shape is missing its projection action",
        )?;
        let IrrefutableBindingAction::Project { result, .. } = &actions[index] else {
            unreachable!("the selected binding action is a projection")
        };
        if result.ty != result_ty {
            return fail("binding projection action does not match its checked shape");
        }
        let result = *result;
        self.consume_action(consumed, index)?;
        Ok(result)
    }

    fn take_component_action(
        &self,
        actions: &[IrrefutableBindingAction],
        consumed: &mut [bool],
        source: BindingTemporary,
        index: std::num::NonZeroU32,
    ) -> Check<BindingTemporary> {
        let action = self.unique_action_index(
            actions,
            |action| {
                matches!(
                    action,
                    IrrefutableBindingAction::Component {
                        source: action_source,
                        index: action_index,
                        ..
                    } if *action_source == source && *action_index == index
                )
            },
            "class binding shape is missing its component action",
        )?;
        let IrrefutableBindingAction::Component { result, .. } = &actions[action] else {
            unreachable!("the selected binding action is a component")
        };
        let result = *result;
        self.consume_action(consumed, action)?;
        Ok(result)
    }

    fn unique_action_index(
        &self,
        actions: &[IrrefutableBindingAction],
        predicate: impl Fn(&IrrefutableBindingAction) -> bool,
        missing: &'static str,
    ) -> Check<usize> {
        let mut matching = actions
            .iter()
            .enumerate()
            .filter_map(|(index, action)| predicate(action).then_some(index));
        let Some(index) = matching.next() else {
            return fail(missing);
        };
        if matching.next().is_some() {
            return fail("binding shape matches multiple runtime actions");
        }
        Ok(index)
    }

    fn consume_action(&self, consumed: &mut [bool], index: usize) -> Check {
        let Some(slot) = consumed.get_mut(index) else {
            return fail("binding shape selects an invalid runtime action");
        };
        if std::mem::replace(slot, true) {
            return fail("binding shape reuses one runtime action");
        }
        Ok(())
    }

    fn require_defined(&self, temporary: BindingTemporary, defined: &HashSet<u32>) -> Check {
        self.check_temporary(temporary, "binding action source temporary")?;
        if !defined.contains(&temporary.local.into_raw().into_u32()) {
            return fail("binding action reads a temporary before its definition");
        }
        Ok(())
    }
}
