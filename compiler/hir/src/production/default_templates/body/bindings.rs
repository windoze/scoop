//! Irrefutable binding-plan projection.

use crate::{
    BindingLeaf, BindingProjection, BindingTemporary, CanonicalBooleanV1, DefaultBindingActionV1,
    DefaultBindingClassComponentV1, DefaultBindingLeafV1, DefaultBindingPlanV1,
    DefaultBindingProjectionV1, DefaultBindingShapeV1, DefaultBindingStructFieldV1,
    DefaultBindingTemporaryV1, IrrefutableBindingAction, IrrefutableBindingPlan,
    IrrefutableBindingShape,
};

use super::BodyProjection;

impl BodyProjection<'_, '_, '_, '_> {
    pub(super) fn binding_temporary(
        &self,
        temporary: BindingTemporary,
    ) -> Result<DefaultBindingTemporaryV1, super::super::DefaultBodyProjectionError> {
        Ok(DefaultBindingTemporaryV1::new(
            self.local(temporary.local)?,
            self.type_key(temporary.ty)?,
        ))
    }

    fn binding_leaf(
        &self,
        leaf: BindingLeaf,
    ) -> Result<DefaultBindingLeafV1, super::super::DefaultBodyProjectionError> {
        Ok(DefaultBindingLeafV1::new(
            self.local(leaf.local)?,
            self.type_key(leaf.ty)?,
            CanonicalBooleanV1::from(leaf.mutability.is_mutable()),
        ))
    }

    fn binding_shape(
        &self,
        shape: &IrrefutableBindingShape,
    ) -> Result<DefaultBindingShapeV1, super::super::DefaultBodyProjectionError> {
        let entities = self.entities;
        let _depth = entities.resources.enter::<DefaultBindingShapeV1>()?;
        match shape {
            IrrefutableBindingShape::Binding(leaf) => {
                Ok(DefaultBindingShapeV1::binding(self.binding_leaf(*leaf)?))
            }
            IrrefutableBindingShape::Wildcard => Ok(DefaultBindingShapeV1::wildcard()),
            IrrefutableBindingShape::Tuple(elements) => {
                self.entities
                    .resources
                    .collection::<DefaultBindingShapeV1>(elements.len())?;
                let elements = elements
                    .iter()
                    .map(|element| self.binding_shape(element))
                    .collect::<Result<Vec<_>, _>>()?;
                DefaultBindingShapeV1::try_tuple(elements)
                    .map_err(super::super::DefaultBodyProjectionError::BindingShape)
            }
            IrrefutableBindingShape::Struct {
                application,
                fields,
            } => {
                let owner = super::super::arena_get(
                    &self.entities.export().struct_applications,
                    *application,
                )
                .ok_or(super::super::DefaultEntityProjectionError::Unknown {
                    kind: "binding struct application",
                    index: super::super::raw_index(*application),
                })?;
                self.entities.resources.sort(fields.len())?;
                self.entities
                    .resources
                    .collection::<DefaultBindingStructFieldV1>(fields.len())?;
                let fields = fields
                    .iter()
                    .map(|(field, shape)| {
                        self.binding_shape(shape).map(|shape| {
                            DefaultBindingStructFieldV1::new(field.local_index(), shape)
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                DefaultBindingShapeV1::try_struct(self.type_key(owner.canonical_type)?, fields)
                    .map_err(super::super::DefaultBodyProjectionError::BindingShape)
            }
            IrrefutableBindingShape::Class {
                application,
                components,
            } => {
                let owner = super::super::arena_get(
                    &self.entities.export().class_applications,
                    *application,
                )
                .ok_or(super::super::DefaultEntityProjectionError::Unknown {
                    kind: "binding class application",
                    index: super::super::raw_index(*application),
                })?;
                self.entities.resources.sort(components.len())?;
                self.entities
                    .resources
                    .collection::<DefaultBindingClassComponentV1>(components.len())?;
                let components = components
                    .iter()
                    .map(|(index, shape)| {
                        self.binding_shape(shape)
                            .map(|shape| DefaultBindingClassComponentV1::new(*index, shape))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                DefaultBindingShapeV1::try_class(self.type_key(owner.canonical_type)?, components)
                    .map_err(super::super::DefaultBodyProjectionError::BindingShape)
            }
        }
    }

    fn binding_projection(
        &self,
        projection: BindingProjection,
    ) -> Result<DefaultBindingProjectionV1, super::super::DefaultBodyProjectionError> {
        match projection {
            BindingProjection::TupleIndex(index) => {
                Ok(DefaultBindingProjectionV1::tuple_index(index))
            }
            BindingProjection::StructField(field) => DefaultBindingProjectionV1::try_struct_field(
                self.entities
                    .field(crate::FieldRef::StructField(field), self.binders)?,
            )
            .map_err(super::super::DefaultBodyProjectionError::BindingProjection),
        }
    }

    fn binding_action(
        &mut self,
        action: &IrrefutableBindingAction,
    ) -> Result<DefaultBindingActionV1, super::super::DefaultBodyProjectionError> {
        match action {
            IrrefutableBindingAction::Project {
                source,
                result,
                projection,
                origin,
                ..
            } => Ok(DefaultBindingActionV1::project(
                self.binding_temporary(*source)?,
                self.binding_temporary(*result)?,
                self.binding_projection(*projection)?,
                self.origin(origin.definition())?,
            )),
            IrrefutableBindingAction::Component {
                source,
                index,
                result,
                setup,
                call,
                span,
            } => DefaultBindingActionV1::try_component(
                self.binding_temporary(*source)?,
                *index,
                self.binding_temporary(*result)?,
                self.statements(setup)?,
                self.expression(call)?,
                self.span_origin(*span)?,
            )
            .map_err(super::super::DefaultBodyProjectionError::BindingAction),
            IrrefutableBindingAction::Bind {
                source,
                target,
                origin,
                ..
            } => Ok(DefaultBindingActionV1::bind(
                self.binding_temporary(*source)?,
                self.binding_leaf(*target)?,
                self.origin(origin.definition())?,
            )),
        }
    }

    pub(super) fn binding_plan(
        &mut self,
        plan: &IrrefutableBindingPlan,
    ) -> Result<DefaultBindingPlanV1, super::super::DefaultBodyProjectionError> {
        self.entities
            .resources
            .collection::<DefaultBindingActionV1>(plan.actions.len())?;
        let actions = plan
            .actions
            .iter()
            .map(|action| self.binding_action(action))
            .collect::<Result<Vec<_>, _>>()?;
        DefaultBindingPlanV1::try_new(
            self.binding_temporary(plan.subject)?,
            self.binding_shape(&plan.shape)?,
            actions,
        )
        .map_err(super::super::DefaultBodyProjectionError::BindingPlan)
    }
}
