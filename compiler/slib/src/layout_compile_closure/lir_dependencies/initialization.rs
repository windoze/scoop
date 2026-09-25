use super::*;
use scoop_wire::BudgetMeter;

/// Join the complete external registration edges to source-replayed MIR uses.
/// Definition and physical-import replay remain separate mandatory checks.
pub fn replay_shared_lir_initialization_dependencies(
    uses: &mir::CanonicalMirExternalInitializationUsesV1,
    registrations: &lir::StrongInitializationUnitRegistrationPlanSetV2,
    meter: &mut BudgetMeter,
) -> Result<(), SharedLirDependencyGraphError> {
    uses.validate_registration_edges(registrations.external_dependency_edges(meter)?, meter)
        .map_err(|error| SharedLirDependencyGraphError::InitializationEdges(Box::new(error)))
}
