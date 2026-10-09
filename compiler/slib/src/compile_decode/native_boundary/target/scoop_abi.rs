//! Canonical Scoop ABI replay shared by native and cross-Cone validation.

use std::collections::HashMap;
use std::sync::Arc;

use scoop_identity::{ExactTypeKey, IdentityLayer, PersistentExactTypeId, ValidatedIdentityGraph};
use scoop_wire::WirePath;

use super::{AbiNominalDefinition, allocate_vec};
use crate::{NativeBoundaryCompileError, compile_decode::native_boundary::records_by_id};

mod dependencies;
pub(crate) use dependencies::{AbiReplayDependency, AbiReplayTypes, collect as collect_abi_types};

pub(super) fn exact_type_records(
    graph: &ValidatedIdentityGraph,
) -> Result<HashMap<PersistentExactTypeId, Arc<ExactTypeKey>>, NativeBoundaryCompileError> {
    records_by_id(
        [
            graph
                .records::<PersistentExactTypeId, ExactTypeKey>(
                    IdentityLayer::Hir,
                    &WirePath::root().field(15),
                )
                .map_err(NativeBoundaryCompileError::Identity)?,
            graph
                .records::<PersistentExactTypeId, ExactTypeKey>(
                    IdentityLayer::Mir,
                    &WirePath::root().field(1),
                )
                .map_err(NativeBoundaryCompileError::Identity)?,
            graph
                .records::<PersistentExactTypeId, ExactTypeKey>(
                    IdentityLayer::Lir,
                    &WirePath::root().field(1),
                )
                .map_err(NativeBoundaryCompileError::Identity)?,
        ],
        &WirePath::root().field(15),
    )
}
