//! Private representation inputs shared by native and ordinary Scoop ABI replay.

use std::borrow::Cow;

use scoop_hir::NativeBoundaryCAbiV1;

use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct AbiNominalDefinition<'a> {
    shape: Cow<'a, NativeBoundaryNominalShape>,
    type_parameter_count: u32,
    c_abi: NativeBoundaryCAbiV1,
}

impl<'a> AbiNominalDefinition<'a> {
    pub(super) fn native(record: &'a NativeBoundaryTypeDefinitionRecord) -> Self {
        Self {
            shape: Cow::Borrowed(record.shape()),
            type_parameter_count: record.type_parameter_count(),
            c_abi: record.c_abi(),
        }
    }

    pub(super) fn shared(shape: NativeBoundaryNominalShape, type_parameter_count: u32) -> Self {
        Self {
            shape: Cow::Owned(shape),
            type_parameter_count,
            c_abi: NativeBoundaryCAbiV1::SourceRepresentation,
        }
    }

    pub(super) fn shape(&self) -> &NativeBoundaryNominalShape {
        &self.shape
    }

    pub(super) const fn c_abi(&self) -> NativeBoundaryCAbiV1 {
        self.c_abi
    }

    pub(super) fn agrees_with_source(&self, other: &Self) -> bool {
        self.type_parameter_count == other.type_parameter_count && self.shape == other.shape
    }
}

pub(super) fn native_definitions<'a>(
    records: &'a [NativeBoundaryTypeDefinitionRecord],
    meter: &mut BudgetMeter,
) -> Result<HashMap<NativeBoundaryNominalOwner, AbiNominalDefinition<'a>>, NativeBoundaryCompileError>
{
    let mut definitions = HashMap::new();
    let path = WirePath::root().field(34);
    meter
        .charge_work(records.len() as u64, &path)
        .map_err(NativeBoundaryCompileError::Resource)?;
    meter
        .charge_owned_bytes(
            (records.len() as u64)
                .saturating_mul(std::mem::size_of::<AbiNominalDefinition<'_>>() as u64),
            &path,
        )
        .map_err(NativeBoundaryCompileError::Resource)?;
    meter
        .try_reserve_map_slots(&mut definitions, records.len(), &path)
        .map_err(NativeBoundaryCompileError::Resource)?;
    for record in records {
        if definitions
            .insert(record.owner(), AbiNominalDefinition::native(record))
            .is_some()
        {
            return Err(NativeBoundaryCompileError::ConflictingTypeWitness {
                owner: record.owner(),
            });
        }
    }
    Ok(definitions)
}
