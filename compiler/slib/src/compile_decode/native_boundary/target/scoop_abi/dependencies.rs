use super::*;
use scoop_hir::{CanonicalNominalInterfacesV1, NativeBoundaryNominalOwner};
use scoop_identity::ConeIdentity;

mod shared;

/// Borrowed type inputs from one already validated, reachable artifact.
#[derive(Clone, Copy)]
pub(crate) struct AbiReplayDependency<'a> {
    pub identity: ConeIdentity,
    pub identities: &'a ValidatedIdentityGraph,
    pub foundation: &'a scoop_hir::CanonicalHirFoundation,
    pub nominals: &'a CanonicalNominalInterfacesV1,
}

impl<'a> From<scoop_hir::SharedTypeMetadataV1<'a>> for AbiReplayDependency<'a> {
    fn from(source: scoop_hir::SharedTypeMetadataV1<'a>) -> Self {
        Self {
            identity: source.provider,
            identities: source.identities,
            foundation: source.foundation,
            nominals: source.public.nominal_interfaces(),
        }
    }
}

pub(crate) fn collect<'a>(
    current: AbiReplayDependency<'a>,
    dependencies: &[AbiReplayDependency<'a>],
) -> Result<AbiReplayTypes<'a>, NativeBoundaryCompileError> {
    let mut exact = HashMap::new();
    let mut definitions = HashMap::new();
    let path = WirePath::root().field(34);
    for source in std::iter::once(current).chain(dependencies.iter().copied()) {
        for (id, key) in exact_type_records(source.identities)? {
            if let Some(previous) = exact.get(&id) {
                if previous != &key {
                    return Err(NativeBoundaryCompileError::ConflictingExactType { exact: id });
                }
            } else {
                scoop_wire::allocation::try_reserve_map(&mut exact, 1, &path)
                    .map_err(NativeBoundaryCompileError::Resource)?;
                exact.insert(id, key);
            }
        }
        for record in source.foundation.native_boundary_types() {
            insert_definition(
                &mut definitions,
                record.owner(),
                AbiNominalDefinition::native(
                    record,
                    super::super::nominals::is_interface(record.owner(), source.identities)?,
                ),
            )?;
        }
    }
    for source in std::iter::once(current).chain(dependencies.iter().copied()) {
        shared::collect(source, &mut definitions)?;
    }
    Ok(AbiReplayTypes { exact, definitions })
}

fn insert_definition<'a>(
    definitions: &mut HashMap<NativeBoundaryNominalOwner, AbiNominalDefinition<'a>>,
    owner: NativeBoundaryNominalOwner,
    record: AbiNominalDefinition<'a>,
) -> Result<(), NativeBoundaryCompileError> {
    let path = WirePath::root().field(34);

    if let Some(previous) = definitions.get(&owner) {
        if previous != &record {
            return Err(NativeBoundaryCompileError::ConflictingTypeWitness { owner });
        }
    } else {
        scoop_wire::allocation::try_reserve_map(definitions, 1, &path)
            .map_err(NativeBoundaryCompileError::Resource)?;
        definitions.insert(owner, record);
    }
    Ok(())
}

pub(crate) struct AbiReplayTypes<'a> {
    pub(in crate::compile_decode::native_boundary::target) exact:
        HashMap<PersistentExactTypeId, Arc<ExactTypeKey>>,
    pub(in crate::compile_decode::native_boundary::target) definitions:
        HashMap<NativeBoundaryNominalOwner, AbiNominalDefinition<'a>>,
}
