use super::*;
use scoop_hir::{NativeBoundaryNominalOwner, NativeBoundaryTypeDefinitionRecord};

/// Borrowed type inputs from one already validated, reachable artifact.
#[derive(Clone, Copy)]
pub(crate) struct AbiReplayDependency<'a> {
    pub identities: &'a ValidatedIdentityGraph,
    pub foundation: &'a OdrFreeHirFoundation,
}

pub(super) fn collect<'a>(
    current: AbiReplayDependency<'a>,
    dependencies: &[AbiReplayDependency<'a>],
    meter: &mut BudgetMeter,
) -> Result<AbiReplayTypes<'a>, NativeBoundaryCompileError> {
    let mut exact = HashMap::new();
    let mut definitions = HashMap::new();
    let path = WirePath::root().field(30);
    for source in std::iter::once(current).chain(dependencies.iter().copied()) {
        for (id, key) in exact_type_records(source.identities, meter)? {
            meter
                .charge_work(
                    scoop_wire::encoded_length(key.as_ref())
                        .map_err(NativeBoundaryCompileError::Encoding)?,
                    &path,
                )
                .map_err(NativeBoundaryCompileError::Resource)?;
            if let Some(previous) = exact.get(&id) {
                if previous != &key {
                    return Err(NativeBoundaryCompileError::ConflictingExactType { exact: id });
                }
            } else {
                meter
                    .try_reserve_map_slots(&mut exact, 1, &path)
                    .map_err(NativeBoundaryCompileError::Resource)?;
                exact.insert(id, key);
            }
        }
        for record in source.foundation.native_boundary_types() {
            meter
                .charge_work(
                    scoop_wire::encoded_length(record)
                        .map_err(NativeBoundaryCompileError::Encoding)?,
                    &path,
                )
                .map_err(NativeBoundaryCompileError::Resource)?;
            if let Some(previous) = definitions.get(&record.owner()) {
                if *previous != record {
                    return Err(NativeBoundaryCompileError::ConflictingTypeWitness {
                        owner: record.owner(),
                    });
                }
            } else {
                meter
                    .try_reserve_map_slots(&mut definitions, 1, &path)
                    .map_err(NativeBoundaryCompileError::Resource)?;
                definitions.insert(record.owner(), record);
            }
        }
    }
    Ok(AbiReplayTypes { exact, definitions })
}

pub(super) struct AbiReplayTypes<'a> {
    pub exact: HashMap<PersistentExactTypeId, Arc<ExactTypeKey>>,
    pub definitions: HashMap<NativeBoundaryNominalOwner, &'a NativeBoundaryTypeDefinitionRecord>,
}
