use super::*;
use scoop_hir::{
    CanonicalNominalInterfacesV1, NativeBoundaryNominalOwner, NativeBoundaryTypeDefinitionRecord,
};
use scoop_identity::ConeIdentity;
use std::borrow::Cow;

mod intrinsics;

/// Borrowed type inputs from one already validated, reachable artifact.
#[derive(Clone, Copy)]
pub(crate) struct AbiReplayDependency<'a> {
    pub identity: ConeIdentity,
    pub identities: &'a ValidatedIdentityGraph,
    pub foundation: &'a OdrFreeHirFoundation,
    pub nominals: &'a CanonicalNominalInterfacesV1,
}

pub(super) fn collect<'a>(
    current: AbiReplayDependency<'a>,
    dependencies: &[AbiReplayDependency<'a>],
    meter: &mut BudgetMeter,
) -> Result<AbiReplayTypes<'a>, NativeBoundaryCompileError> {
    let mut exact = HashMap::new();
    let mut definitions = HashMap::new();
    let path = WirePath::root().field(33);
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
            insert_definition(&mut definitions, Cow::Borrowed(record), meter)?;
        }
    }
    for source in std::iter::once(current).chain(dependencies.iter().copied()) {
        intrinsics::collect(source, &mut definitions, meter)?;
    }
    Ok(AbiReplayTypes { exact, definitions })
}

fn insert_definition<'a>(
    definitions: &mut HashMap<
        NativeBoundaryNominalOwner,
        Cow<'a, NativeBoundaryTypeDefinitionRecord>,
    >,
    record: Cow<'a, NativeBoundaryTypeDefinitionRecord>,
    meter: &mut BudgetMeter,
) -> Result<(), NativeBoundaryCompileError> {
    let path = WirePath::root().field(33);
    meter
        .charge_work(
            scoop_wire::encoded_length(record.as_ref())
                .map_err(NativeBoundaryCompileError::Encoding)?,
            &path,
        )
        .map_err(NativeBoundaryCompileError::Resource)?;
    if let Some(previous) = definitions.get(&record.owner()) {
        if previous != &record {
            return Err(NativeBoundaryCompileError::ConflictingTypeWitness {
                owner: record.owner(),
            });
        }
    } else {
        meter
            .charge_owned_bytes(
                std::mem::size_of::<Cow<'_, NativeBoundaryTypeDefinitionRecord>>() as u64,
                &path,
            )
            .map_err(NativeBoundaryCompileError::Resource)?;
        meter
            .try_reserve_map_slots(definitions, 1, &path)
            .map_err(NativeBoundaryCompileError::Resource)?;
        definitions.insert(record.owner(), record);
    }
    Ok(())
}

pub(super) struct AbiReplayTypes<'a> {
    pub exact: HashMap<PersistentExactTypeId, Arc<ExactTypeKey>>,
    definitions: HashMap<NativeBoundaryNominalOwner, Cow<'a, NativeBoundaryTypeDefinitionRecord>>,
}

impl AbiReplayTypes<'_> {
    pub(super) fn definition_refs(
        &self,
        meter: &mut BudgetMeter,
    ) -> Result<
        HashMap<NativeBoundaryNominalOwner, &NativeBoundaryTypeDefinitionRecord>,
        NativeBoundaryCompileError,
    > {
        let mut result = HashMap::new();
        meter
            .try_reserve_map_slots(
                &mut result,
                self.definitions.len(),
                &WirePath::root().field(33),
            )
            .map_err(NativeBoundaryCompileError::Resource)?;
        result.extend(
            self.definitions
                .iter()
                .map(|(owner, record)| (*owner, record.as_ref())),
        );
        Ok(result)
    }
}
