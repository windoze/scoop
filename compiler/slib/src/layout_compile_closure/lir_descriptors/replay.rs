use super::SharedLirDescriptorValidationError as Error;
use scoop_identity::ExactTypeDiagnosticGraph;
use scoop_lir as lir;
use scoop_mir as mir;
use scoop_wire::WirePath;

mod references;

#[derive(Clone, Copy)]
pub struct SharedLirDescriptorInputsV1<'a> {
    pub layouts: &'a lir::CanonicalExactLayoutExportsV1,
    pub dispatch: &'a lir::CanonicalExactDispatchExportsV1,
    pub dependencies: &'a [&'a lir::CanonicalExactDescriptorExportsV1],
}

/// Reconstructs the complete descriptor constituent from checked MIR types,
/// layouts and dispatch. No selected use or machine registration is implied.
pub fn replay_shared_mir_descriptors(
    target: lir::LirTargetProfile,
    types: &mir::CanonicalParamFreeMirTypeExportsV1,
    inputs: SharedLirDescriptorInputsV1<'_>,
    diagnostics: &impl ExactTypeDiagnosticGraph,
    foundation: &lir::ConeLirFoundation,
) -> Result<lir::CanonicalExactDescriptorExportsV1, Error> {
    let references = references::References::new(target, types, inputs, foundation.producer())?;
    let path = WirePath::root();

    let mut records = Vec::new();
    scoop_wire::allocation::try_reserve(&mut records, types.records().len(), &path)?;
    for ty in types.records() {
        if matches!(
            ty.representation(),
            mir::MirTypeRepresentationV1::ObjectBacking { .. }
        ) {
            continue;
        }
        let parent = match parent(ty) {
            mir::MirBaseClassV1::None => None,
            mir::MirBaseClassV1::Base(exact) => Some(references.get(exact)?),
        };
        let mut interfaces = Vec::new();
        for table in inputs.dispatch.records() {
            if table.owner_exact() != ty.exact() {
                continue;
            }
            if let lir::ExactDispatchRoleV1::Itable { interface_exact } = table.role() {
                scoop_wire::allocation::try_reserve(&mut interfaces, 1, &path)?;
                interfaces.push(references.get(interface_exact)?);
            }
        }

        interfaces.sort_unstable_by_key(|reference| reference.exact_type());
        let record = lir::ExactDescriptorExportV1::replay_from_constituents(
            target,
            lir::ExactDescriptorSourceInputV1 {
                exact: ty.exact(),
                parent,
                interfaces: &interfaces,
            },
            inputs.layouts,
            inputs.dispatch,
            diagnostics,
            foundation,
        )
        .map_err(|source| Error::Replay {
            exact: ty.exact(),
            source: Box::new(source),
        })?;
        records.push(record);
    }
    Ok(lir::CanonicalExactDescriptorExportsV1::try_new(
        target, foundation, records,
    )?)
}

fn parent(ty: &mir::ParamFreeMirTypeExportV1) -> mir::MirBaseClassV1 {
    use mir::{MirParamFreeIntrinsicV1 as Intrinsic, MirTypeRepresentationV1 as Representation};
    match ty.representation() {
        Representation::Class { .. }
        | Representation::Object { .. }
        | Representation::Intrinsic(Intrinsic::String)
        | Representation::BoxedValue { .. } => ty.base_and_interfaces().base,
        Representation::Intrinsic(_)
        | Representation::Struct { .. }
        | Representation::Enum { .. }
        | Representation::Interface
        | Representation::ObjectBacking { .. }
        | Representation::CoroutineStep { .. }
        | Representation::CoroutineSlot { .. } => mir::MirBaseClassV1::None,
    }
}
