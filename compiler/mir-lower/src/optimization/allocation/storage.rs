use scoop_identity::{CallableMaterialization, StructuralDefinitionSiteRole, SyntheticLocalRole};

use super::{
    index, mir,
    origins::{AllocationKind, Projection},
    uses::Plan,
};
use crate::local_values::LocalValueRegistry;

pub(super) struct ReplacementLocal {
    pub projection: Projection,
    pub local: mir::LocalId,
    pub initialization: Initialization,
}

pub(super) enum Initialization {
    AllocationValue(mir::Expr),
    /// Source constructor stores initialize the field before any legal read.
    FieldStores,
}

pub(super) fn create(
    module: &mir::Module,
    function: &mut mir::Function,
    id: mir::FunctionId,
    owner: CallableMaterialization,
    registry: &mut LocalValueRegistry,
    plan: &Plan,
) -> Vec<Vec<ReplacementLocal>> {
    plan.sources
        .allocations
        .iter()
        .map(|(candidate, creation)| {
            if !plan.removable[index(candidate)] {
                return Vec::new();
            }
            let mut record = |projection: Projection, ty: &mir::Type, initialization| {
                let field = match projection {
                    Projection::BoxPayload => "payload".to_owned(),
                    Projection::Capture(field) => format!("capture.{field}"),
                    Projection::Field(field) => format!("field.{field}"),
                };
                let local = function.body.locals.alloc(mir::Local {
                    debug_origin: Default::default(),
                    name: format!("$allocation.{}.{field}", index(creation.local)),
                    ty: ty.clone(),
                    mutable: matches!(projection, Projection::Field(_)),
                });
                registry.record_generated_local(
                    id,
                    local,
                    owner,
                    StructuralDefinitionSiteRole::SyntheticValue,
                    SyntheticLocalRole::Temporary,
                );
                ReplacementLocal {
                    projection,
                    local,
                    initialization,
                }
            };
            match &creation.kind {
                AllocationKind::Box(payload) => vec![record(
                    Projection::BoxPayload,
                    &payload.ty,
                    Initialization::AllocationValue(payload.clone()),
                )],
                AllocationKind::Closure { captures, .. } => captures
                    .iter()
                    .map(|capture| {
                        record(
                            Projection::Capture(capture.field()),
                            &capture.value().ty,
                            Initialization::AllocationValue(capture.value().clone()),
                        )
                    })
                    .collect(),
                AllocationKind::Class(class) => {
                    let mir::ClassRepresentation::Declared(declared) =
                        &module.classes[*class].representation
                    else {
                        unreachable!("only declared classes have replaceable fields")
                    };
                    declared
                        .fields
                        .iter()
                        .enumerate()
                        .map(|(index, field)| {
                            record(
                                Projection::Field(index as u32),
                                &field.ty,
                                Initialization::FieldStores,
                            )
                        })
                        .collect()
                }
            }
        })
        .collect()
}
