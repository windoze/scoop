use scoop_identity::{CallableMaterialization, StructuralDefinitionSiteRole, SyntheticLocalRole};

use super::{
    index, mir,
    origins::{AllocationKind, Projection},
    uses::Plan,
};
use crate::local_values::LocalValueRegistry;

pub(super) struct Snapshot {
    pub projection: Projection,
    pub local: mir::LocalId,
    pub init: mir::Expr,
}

pub(super) fn create(
    function: &mut mir::Function,
    id: mir::FunctionId,
    owner: CallableMaterialization,
    registry: &mut LocalValueRegistry,
    plan: &Plan,
) -> Vec<Vec<Snapshot>> {
    plan.sources
        .allocations
        .iter()
        .map(|(candidate, creation)| {
            if !plan.removable[index(candidate)] {
                return Vec::new();
            }
            let mut record = |projection: Projection, value: &mir::Expr| {
                let field = match projection {
                    Projection::BoxPayload => "payload".to_owned(),
                    Projection::Capture(field) => format!("capture.{field}"),
                };
                let local = function.body.locals.alloc(mir::Local {
                    name: format!("$allocation.{}.{field}", index(creation.local)),
                    ty: value.ty.clone(),
                    mutable: false,
                });
                registry.record_generated_local(
                    id,
                    local,
                    owner,
                    StructuralDefinitionSiteRole::SyntheticValue,
                    SyntheticLocalRole::Temporary,
                );
                Snapshot {
                    projection,
                    local,
                    init: value.clone(),
                }
            };
            match &creation.kind {
                AllocationKind::Box(payload) => vec![record(Projection::BoxPayload, payload)],
                AllocationKind::Closure { captures, .. } => captures
                    .iter()
                    .map(|capture| record(Projection::Capture(capture.field()), capture.value()))
                    .collect(),
            }
        })
        .collect()
}
