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
    ) -> Result<Self, Error> {
        input
            .bridge
            .validate_sources(input.mir.module().cone, input.identities, mir_source)
            .map_err(|error| Error::MirSource(Box::new(error)))?;
        input
            .bridge
            .initialization_uses()
            .validate_registration_edges(
                input
                    .registration
                    .registration_production()
                    .initialization_units()
                    .external_dependency_edges()?,
            )
            .map_err(|error| Error::InitializationEdges(Box::new(error)))?;
        let committed = mir_source.committed_external_uses().map_err(|error| {
            Error::MirSource(Box::new(mir::MirTypeBridgeSourceJoinError::Source(error)))
        })?;
        let expected = lower_layout_abi_exports(input, dependencies)?;
        let uses = uses::project(input, dependencies, &expected, committed)?;
        let physical = physical::project(input)?;
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
    ) -> Result<(), Error> {
        use LayoutAbiSourceInventoryV1 as Inventory;
        compare(
            exports.layouts(),
            self.expected.layouts(),
            Inventory::Layouts,
        )?;
        compare(
            exports.descriptors(),
            self.expected.descriptors(),
            Inventory::Descriptors,
        )?;
        compare(
            exports.dispatch(),
            self.expected.dispatch(),
            Inventory::Dispatch,
        )?;
        compare(
            exports.callables(),
            self.expected.callables(),
            Inventory::Callables,
        )?;
        compare(
            exports.shape_support(),
            self.expected.shape_support(),
            Inventory::ShapeSupport,
        )
    }

    fn committed_semantic_roots(&self) -> Result<&[lir::LayoutAbiDependencyV1], Error> {
        Ok(&self.uses)
    }

    fn validate_physical_imports(
        &self,
        imports: &[lir::ExternalShapeLinkImportV1<'_>],
    ) -> Result<(), Error> {
        physical::validate(&self.physical, imports)
    }
}

fn compare<T: WireEncode + Eq>(
    actual: &T,
    expected: &T,
    inventory: LayoutAbiSourceInventoryV1,
) -> Result<(), Error> {
    if actual != expected {
        return Err(Error::Inventory(inventory));
    }
    Ok(())
}

fn push<T>(values: &mut Vec<T>, value: T) -> Result<(), Error> {
    let path = WirePath::root();

    scoop_wire::allocation::try_reserve(values, 1, &path)?;
    values.push(value);
    Ok(())
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
    InitializationEdges(Box<mir::MirObjectBridgeError>),
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
            Self::InitializationEdges(error) => Some(error.as_ref()),
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
