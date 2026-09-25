//! Representation contracts use the same provider-scoped declaration inputs.

use super::*;
use scoop_hir::{NominalCLayoutPolicyV1, NominalSourceShapeV1, SourceNominalId};

pub(crate) fn validate_shared_target_normalization(
    artifact: &mut ValidatedGraphArtifact<'_>,
    current: AbiReplayDependency<'_>,
    dependencies: &[AbiReplayDependency<'_>],
    view: &NativeBoundaryFoundationView<'_>,
    materialized_types: &[PersistentExactTypeId],
) -> Result<(), NativeBoundaryCompileError> {
    let types = scoop_abi::collect_abi_types(current, dependencies)?;
    let roots = representation_roots(current, &types, materialized_types)?;
    normalization::validate(artifact, current.identities, view, types, &roots)
}

fn representation_roots(
    current: AbiReplayDependency<'_>,
    types: &scoop_abi::AbiReplayTypes<'_>,
    materialized_types: &[PersistentExactTypeId],
) -> Result<Vec<PersistentExactTypeId>, NativeBoundaryCompileError> {
    let path = WirePath::root().field(16);
    let mut roots = Vec::new();
    for exact in materialized_types {
        let key = types
            .exact
            .get(exact)
            .ok_or(NativeBoundaryTargetError::MissingExactType { exact: *exact })?;
        let ExactTypeKey::Nominal(owner) = key.as_ref() else {
            continue;
        };
        let Some(declaration) = current
            .nominals
            .declaration(SourceNominalId::Concrete(*owner))
        else {
            continue;
        };
        if matches!(declaration.source_shape(), NominalSourceShapeV1::Struct(shape)
            if matches!(shape.c_layout_policy(), NominalCLayoutPolicyV1::CLayout { .. }))
        {
            scoop_wire::allocation::try_reserve(&mut roots, 1, &path)
                .map_err(NativeBoundaryCompileError::Resource)?;
            roots.push(*exact);
        }
    }
    Ok(roots)
}
