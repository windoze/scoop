use super::*;
use crate::{
    BaseInitialization, BaseInitializerTarget, ClassConstructorKind, ClassInitializationStep,
    ClassSecondaryDelegation, DefaultClassConstructorIdV1, DefaultConstructorRefV1,
    ExportCommonInitializationStepV1, ExportConstructorDelegationV1,
    ExportConstructorInitializationKindV1 as Kind, ExportConstructorInitializationV1,
    ExportPrimaryFieldStoreV1,
};
use scoop_identity::{CallableTemplateOrigin, LocalValueSelector};

mod common;
mod constructor;
pub(in crate::production::default_templates) use common::project as project_common;
pub(in crate::production::default_templates) use constructor::project as project_constructor;

pub(super) fn project(
    export: &ExportHir,
    entities: &DefaultEntityProjector<'_>,
    owner: scoop_identity::PersistentGenericTypeId,
    class: crate::ClassId,
) -> Result<ExportGenericNominalInitializationV1, Error> {
    let declaration = &export.classes[class];
    let common = project_common(entities, class)?;
    let constructors = declaration
        .constructors
        .iter()
        .map(|&id| project_constructor(entities, id))
        .collect::<Result<_, _>>()?;
    ExportGenericNominalInitializationV1::try_new(
        owner,
        common,
        constructors,
        super::release::project(export, entities, declaration)?,
    )
    .map_err(Error::Initialization)
}

fn class_field(
    projection: &ConstructorProjection<'_, '_>,
    field: crate::InitializingClassFieldRef,
) -> Result<crate::DefaultFieldRefV1, Error> {
    projection
        .entities
        .field(
            crate::FieldRef::ClassField {
                owner: field.owner,
                field: field.field,
            },
            &projection.binders,
        )
        .map_err(Error::Entity)
}
