//! Actual typed layout uses and physical references from complete MIR/LIR.

use super::*;
use LayoutAbiDependencyLoweringError as Error;

mod uses;

pub fn lower_layout_abi_dependencies(
    input: LayoutAbiExportInputV1<'_>,
    dependencies: LayoutAbiExportDependenciesV1<'_>,
    exports: &lir::LayoutAbiExportConstituentsV1,
    committed: &[mir::MirTypeBridgeDependencyV1],
) -> Result<Vec<lir::LayoutAbiDependencyV1>, Error> {
    uses::project(input, dependencies, exports, committed)
}

fn push<T>(values: &mut Vec<T>, value: T) -> Result<(), Error> {
    scoop_wire::allocation::try_reserve(values, 1, &WirePath::root())?;
    values.push(value);
    Ok(())
}

#[derive(Debug)]
pub enum LayoutAbiDependencyLoweringError {
    Production(LayoutAbiExportLoweringError),
    Resource(WireError),
    TypeProvider {
        exact: PersistentExactTypeId,
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    LocalDependency(mir::MirTypeBridgeTargetV1),
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
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cannot lower layout/ABI dependencies: {self:?}")
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Production(error) => Some(error),
            Self::Resource(error) => Some(error),
            Self::TypeProvider { .. } | Self::LocalDependency(_) => None,
        }
    }
}
