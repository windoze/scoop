//! Section source inputs projected from actual MIR/LIR and registration records.

use super::*;
use LayoutAbiSourceProjectionError as Error;
use scoop_wire::WireEncode;

mod physical;
mod uses;

pub struct LayoutAbiSourceProjectionV1 {
    expected: lir::LayoutAbiExportConstituentsV1,
    uses: Vec<lir::LayoutAbiDependencyV1>,
    physical: Vec<physical::RequiredImport>,
}

impl LayoutAbiSourceProjectionV1 {
    pub fn from_input<E: std::fmt::Debug + Send + Sync + 'static>(
        input: LayoutAbiExportInputV1<'_>,
        dependencies: LayoutAbiExportDependenciesV1<'_>,
        mir_source: &impl mir::MirTypeBridgeSectionSourceAuthorityV1<E>,
        meter: &mut BudgetMeter,
    ) -> Result<Self, Error> {
        input
            .bridge
            .validate_sources(input.mir.module().cone, input.identities, mir_source, meter)
            .map_err(|error| Error::MirSource(Box::new(error)))?;
        let committed = mir_source.committed_external_uses().map_err(|error| {
            Error::MirSource(Box::new(mir::MirTypeBridgeSourceJoinError::Source(error)))
        })?;
        let expected = lower_layout_abi_exports(input, dependencies, meter)?;
        let uses = uses::project(input, dependencies, &expected, committed, meter)?;
        let physical = physical::project(input, meter)?;
        Ok(Self {
            expected,
            uses,
            physical,
        })
    }
}

impl lir::LayoutAbiSectionSourceAuthorityV1<Error> for LayoutAbiSourceProjectionV1 {
    fn validate_local_exports(
        &self,
        exports: &lir::LayoutAbiExportConstituentsV1,
        meter: &mut BudgetMeter,
    ) -> Result<(), Error> {
        use LayoutAbiSourceInventoryV1 as Inventory;
        compare(
            exports.layouts(),
            self.expected.layouts(),
            Inventory::Layouts,
            meter,
        )?;
        compare(
            exports.descriptors(),
            self.expected.descriptors(),
            Inventory::Descriptors,
            meter,
        )?;
        compare(
            exports.dispatch(),
            self.expected.dispatch(),
            Inventory::Dispatch,
            meter,
        )?;
        compare(
            exports.callables(),
            self.expected.callables(),
            Inventory::Callables,
            meter,
        )?;
        compare(
            exports.shape_support(),
            self.expected.shape_support(),
            Inventory::ShapeSupport,
            meter,
        )
    }

    fn committed_semantic_roots(&self) -> Result<&[lir::LayoutAbiDependencyV1], Error> {
        Ok(&self.uses)
    }

    fn validate_physical_imports(
        &self,
        imports: &[lir::ExternalShapeLinkImportV1<'_>],
        meter: &mut BudgetMeter,
    ) -> Result<(), Error> {
        physical::validate(&self.physical, imports, meter)
    }
}

fn compare<T: WireEncode + Eq>(
    actual: &T,
    expected: &T,
    inventory: LayoutAbiSourceInventoryV1,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    for value in [actual, expected] {
        meter.charge_work(scoop_wire::encoded_length(value)?, &WirePath::root())?;
    }
    if actual != expected {
        return Err(Error::Inventory(inventory));
    }
    Ok(())
}

fn push<T>(values: &mut Vec<T>, value: T, meter: &mut BudgetMeter) -> Result<(), Error> {
    let path = WirePath::root();
    meter.check_table_entries(values.len() as u64 + 1, &path)?;
    meter.charge_owned_bytes(std::mem::size_of::<T>() as u64, &path)?;
    meter.try_reserve_collection_slots(values, 1, &path)?;
    values.push(value);
    Ok(())
}

fn sort_cost(count: usize, meter: &mut BudgetMeter) -> Result<(), Error> {
    Ok(meter.charge_work(
        (count as u64).saturating_mul(u64::from(count.checked_ilog2().unwrap_or(0)) + 2),
        &WirePath::root(),
    )?)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LayoutAbiSourceInventoryV1 {
    Layouts,
    Descriptors,
    Dispatch,
    Callables,
    ShapeSupport,
}

#[derive(Debug)]
pub enum LayoutAbiSourceProjectionError {
    MirSource(Box<dyn std::error::Error + Send + Sync>),
    Production(LayoutAbiExportLoweringError),
    Resource(WireError),
    Encoding(scoop_wire::cbor::EncodeError),
    Inventory(LayoutAbiSourceInventoryV1),
    TypeProvider {
        exact: PersistentExactTypeId,
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    LocalDependency(mir::MirTypeBridgeTargetV1),
    PhysicalInventory,
    PhysicalDefinition {
        provider: ConeIdentity,
        subject: lir::ExternalStrongShapeSubjectV1,
    },
}
impl From<LayoutAbiExportLoweringError> for Error {
    fn from(value: LayoutAbiExportLoweringError) -> Self {
        Self::Production(value)
    }
}
impl From<WireError> for Error {
    fn from(value: WireError) -> Self {
        Self::Resource(value)
    }
}
impl From<scoop_wire::cbor::EncodeError> for Error {
    fn from(value: scoop_wire::cbor::EncodeError) -> Self {
        Self::Encoding(value)
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cannot project the layout/ABI section source: {self:?}")
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::MirSource(error) => Some(error.as_ref()),
            Self::Production(error) => Some(error),
            Self::Resource(error) => Some(error),
            Self::Encoding(error) => Some(error),
            Self::Inventory(_)
            | Self::TypeProvider { .. }
            | Self::LocalDependency(_)
            | Self::PhysicalInventory
            | Self::PhysicalDefinition { .. } => None,
        }
    }
}
