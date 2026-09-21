use std::fmt;

use scoop_hir::{CoreBootstrapInterfaceSectionV1, CoreHirInterfaceBranchV1};
use scoop_mir::{
    CallableOwner, ConeIdentity, CoreBootstrapBridgeSectionV1, MirProductionBuildError,
    OdrFreeMirFoundation, StrongCallableBridgeSurfaceV1,
};

/// Projects one checked HIR production section and the complete strong MIR
/// foundation into the mandatory MIR production section.
///
/// Entry contracts and compiler protocols use the same complete strong MIR
/// foundation.
pub fn lower_production_section(
    artifact: ConeIdentity,
    hir: &CoreBootstrapInterfaceSectionV1,
    foundation: &OdrFreeMirFoundation,
) -> Result<CoreBootstrapBridgeSectionV1, MirProductionLoweringError> {
    let strong_callable_bridges =
        StrongCallableBridgeSurfaceV1::from_odr_free_foundation(foundation);
    let strong_callable_bridges = lower_callable_roles(hir, strong_callable_bridges)?;
    let entry_bridge = lower_entry_bridge(hir.output_contract())
        .map_err(MirProductionLoweringError::Production)?;
    CoreBootstrapBridgeSectionV1::try_new(artifact, entry_bridge, strong_callable_bridges)
        .map_err(MirProductionLoweringError::Production)
}

fn lower_callable_roles(
    hir: &CoreBootstrapInterfaceSectionV1,
    strong: StrongCallableBridgeSurfaceV1,
) -> Result<StrongCallableBridgeSurfaceV1, MirProductionLoweringError> {
    let CoreHirInterfaceBranchV1::Core(interface) = hir.core_interface() else {
        return Ok(strong);
    };

    let cycle = interface
        .compiler_protocols()
        .initialization_cycle_thrower();
    let scoop_hir::CoreProtocolCallableDefinitionV1::Function(cycle_definition) =
        cycle.definition()
    else {
        return Err(MirProductionLoweringError::InvalidInitializationCycleThrower);
    };
    let cycle_implementation = CallableOwner::Function(cycle_definition);
    let cycle_signature = strong
        .bridges()
        .iter()
        .find(|bridge| bridge.implementation() == cycle_implementation)
        .ok_or(MirProductionLoweringError::MissingInitializationCycleThrower)?
        .signature();
    let expected_cycle_signature = scoop_hir::concrete::ExactCallableSignature::new(
        scoop_hir::concrete::Effect::Ordinary,
        None,
        vec![interface.string_capability().exact_type()],
        scoop_mir::core_unit_exact_type(),
    );
    if cycle_signature != &expected_cycle_signature {
        return Err(MirProductionLoweringError::InitializationCycleSignatureMismatch);
    }
    strong
        .with_initialization_cycle(cycle_definition)
        .map_err(MirProductionLoweringError::Production)
}

/// Projects the sealed HIR output branch without dropping any part of the
/// executable source-entry proof.
fn lower_entry_bridge(
    output: &scoop_hir::HirOutputContractV1,
) -> Result<scoop_mir::EntryMirBridgeBranchV1, scoop_mir::MirProductionBuildError> {
    match output {
        scoop_hir::HirOutputContractV1::Library => Ok(scoop_mir::EntryMirBridgeBranchV1::Library),
        scoop_hir::HirOutputContractV1::Executable(source) => {
            let implementation = scoop_mir::CallableOwner::Function(source.declaration());
            scoop_mir::EntryMirBridgeV1::new(source.as_ref().clone(), implementation)
                .map(Box::new)
                .map(scoop_mir::EntryMirBridgeBranchV1::Executable)
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum MirProductionLoweringError {
    InvalidInitializationCycleThrower,
    MissingInitializationCycleThrower,
    InitializationCycleSignatureMismatch,
    Production(MirProductionBuildError),
}

impl fmt::Display for MirProductionLoweringError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "cannot lower MIR production section: {self:?}")
    }
}

impl std::error::Error for MirProductionLoweringError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Production(source) => Some(source),
            Self::InvalidInitializationCycleThrower
            | Self::MissingInitializationCycleThrower
            | Self::InitializationCycleSignatureMismatch => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use scoop_hir::HirOutputContractV1;
    use scoop_identity::{
        CallableOwner, CanonicalIdentifier, CborIdentityRecord, ConeIdentity, CoreBuiltinNominal,
        DeclarationScope, DefinitionOwnerChain, ExactOrdinaryNoArgUnitSignature, ExactTypeKey,
        ExecutableSourceEntryIdentity, PackagePath, PersistentExactTypeId, SourceDeclarationKey,
        SourceDeclarationSite,
    };

    use super::lower_entry_bridge;

    #[test]
    fn lowering_preserves_the_complete_executable_source_proof() {
        let source = source();
        let output = HirOutputContractV1::Executable(Box::new(source.clone()));
        let bridge = lower_entry_bridge(&output).unwrap();
        let scoop_mir::EntryMirBridgeBranchV1::Executable(bridge) = bridge else {
            panic!("the executable HIR branch has an executable MIR branch")
        };

        assert_eq!(bridge.source(), &source);
        assert_eq!(
            bridge.implementation(),
            CallableOwner::Function(source.declaration())
        );
        assert_eq!(
            lower_entry_bridge(&HirOutputContractV1::Library).unwrap(),
            scoop_mir::EntryMirBridgeBranchV1::Library
        );
    }

    fn source() -> ExecutableSourceEntryIdentity {
        let declaration = CborIdentityRecord::from_key(SourceDeclarationKey::function(
            SourceDeclarationSite::new(
                ConeIdentity::SINGLE_FILE,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new("main").unwrap(),
            0,
            None,
            Vec::new(),
        ))
        .unwrap();
        let unit = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
            CoreBuiltinNominal::Unit.identity_record().id(),
        ))
        .unwrap();
        ExecutableSourceEntryIdentity::try_new(
            &declaration,
            ExactOrdinaryNoArgUnitSignature::new(unit),
        )
        .unwrap()
    }
}
