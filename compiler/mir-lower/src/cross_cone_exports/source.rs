//! Dependency uses and initialization definitions from complete HIR/MIR.

use super::*;
use scoop_identity::{CallableOwner, PersistentInitializationUnitId};

mod uses;

pub fn lower_type_bridge_dependencies(
    input: MirTypeBridgeExportInputV1<'_>,
) -> Result<Vec<mir::MirTypeBridgeDependencyV1>, MirTypeBridgeUseLoweringError> {
    uses::project(input)
}

pub fn lower_type_bridge_initialization_units(
    input: &mir::SingleConeStrongMirInput,
) -> Result<Vec<mir::MirTypeBridgeInitializationUnitV1>, MirTypeBridgeUseLoweringError> {
    use MirTypeBridgeUseLoweringError as Error;
    let mut units = Vec::new();
    scoop_wire::allocation::try_reserve(
        &mut units,
        input.materialization().initialization_roots().len(),
        &WirePath::root(),
    )?;
    for root in input.materialization().initialization_roots() {
        let CallableOwner::Generated(initializer) = root.initializer().implementation() else {
            return Err(Error::InitializationDefinition(root.identity()));
        };
        let CallableOwner::Generated(ensure) = root.ensure().implementation() else {
            return Err(Error::InitializationDefinition(root.identity()));
        };
        let definition = input
            .production()
            .strong_callable_bridges()
            .get(root.ensure().implementation())
            .ok_or(Error::InitializationDefinition(root.identity()))?;
        units.push(mir::MirTypeBridgeInitializationUnitV1::new(
            root.identity(),
            initializer,
            ensure,
            mir::MirBridgeCallableSignatureV1::new(
                definition.signature().clone(),
                input.module().functions[root.ensure().function()].gc_effect,
            ),
        ));
    }
    units.sort_unstable_by_key(mir::MirTypeBridgeInitializationUnitV1::unit);
    Ok(units)
}

#[derive(Debug)]
pub enum MirTypeBridgeUseLoweringError {
    Resource(WireError),
    InitializationDefinition(PersistentInitializationUnitId),
    SharedTypeOccurrences(Box<hir::HirDependencyTypeRelationError>),
}
impl From<WireError> for MirTypeBridgeUseLoweringError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl std::fmt::Display for MirTypeBridgeUseLoweringError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cannot lower MIR type bridge uses: {self:?}")
    }
}
impl std::error::Error for MirTypeBridgeUseLoweringError {}
