use std::collections::{BTreeMap, BTreeSet};

use scoop_identity::{ConeIdentity, PersistentInitializationUnitId};

use super::{
    StrongInitializationUnitSemanticPlan, StrongInitializationUnitSemanticPlanBuildError,
    StrongInitializationUnitSemanticPlanSetV1, StrongInitializationUnitSemanticPlanSetV2,
};
use crate::{
    InitializationDefinitionResolutionErrorV2, InitializationDependencyResolutionError, Module,
    OdrFreeLirFoundation, StrongDigestFinalizationPlanV1, StrongExternalInitializationUseV2,
    StrongInitializationDefinitionCatalogV2, StrongInitializationUnitDefinitionRefV2,
    StrongRegistrationIdentitySurfaceV1,
};

impl StrongInitializationUnitSemanticPlanSetV2 {
    /// Projects final local initialization semantics and joins every foreign
    /// edge to a checked provider definition. Repeated physical uses of the
    /// same dependency (for different MIR causes) are validated before this
    /// boundary and collapse to the single dependency id carried on wire.
    pub fn from_module(
        module: &Module,
        foundation: &OdrFreeLirFoundation,
        identities: &StrongRegistrationIdentitySurfaceV1,
        digests: &StrongDigestFinalizationPlanV1,
        selected: &crate::StrongProductionDependencySelectionV2<'_>,
        external_uses: &[StrongExternalInitializationUseV2],
    ) -> Result<Self, StrongInitializationUnitSemanticPlanV2BuildError> {
        if module.cone != foundation.producer() {
            return Err(
                StrongInitializationUnitSemanticPlanV2BuildError::ProducerMismatch {
                    module: module.cone,
                    foundation: foundation.producer(),
                },
            );
        }
        let local = StrongInitializationUnitSemanticPlanSetV1::from_module(module)
            .map_err(StrongInitializationUnitSemanticPlanV2BuildError::LocalSemantics)?;

        let mut definitions = BTreeMap::new();
        for semantic in local.units() {
            let definition = StrongInitializationUnitDefinitionRefV2::from_foundation(
                semantic.unit(),
                foundation,
                identities,
                digests,
            )
            .map_err(StrongInitializationUnitSemanticPlanV2BuildError::LocalDefinition)?;
            definitions.insert(semantic.unit(), definition);
        }

        let local_units = local
            .units()
            .iter()
            .map(|semantic| semantic.unit())
            .collect::<BTreeSet<_>>();
        for use_record in external_uses {
            use_record
                .validate_against(selected)
                .map_err(StrongInitializationUnitSemanticPlanV2BuildError::ExternalUse)?;
            if use_record.consumer() != module.cone {
                return Err(
                    StrongInitializationUnitSemanticPlanV2BuildError::UseConsumer {
                        expected: module.cone,
                        actual: use_record.consumer(),
                    },
                );
            }
            if !local_units.contains(&use_record.local_unit()) {
                return Err(
                    StrongInitializationUnitSemanticPlanV2BuildError::UnknownLocalUnit(
                        use_record.local_unit(),
                    ),
                );
            }
            let dependency = use_record.dependency();
            match definitions.insert(dependency.unit(), dependency) {
                Some(previous) if previous != dependency => {
                    return Err(
                        StrongInitializationUnitSemanticPlanV2BuildError::ConflictingDefinition {
                            unit: dependency.unit(),
                            first: previous.provider(),
                            second: dependency.provider(),
                        },
                    );
                }
                _ => {}
            }
        }

        let definitions = definitions.into_values().collect::<Vec<_>>();
        let catalog = StrongInitializationDefinitionCatalogV2::new(module.cone, &definitions)
            .map_err(StrongInitializationUnitSemanticPlanV2BuildError::Definitions)?;
        let mut units = Vec::with_capacity(local.units().len());
        for semantic in local.units() {
            let mut dependencies = semantic
                .dependencies()
                .iter()
                .copied()
                .chain(
                    external_uses
                        .iter()
                        .filter(|use_record| use_record.local_unit() == semantic.unit())
                        .map(|use_record| use_record.dependency_unit()),
                )
                .collect::<Vec<_>>();
            dependencies.sort_unstable();
            dependencies.dedup();
            let resolved = catalog
                .resolve_ids(semantic.unit(), &dependencies)
                .map_err(StrongInitializationUnitSemanticPlanV2BuildError::Dependencies)?;
            units.push(StrongInitializationUnitSemanticPlan::from_artifact(
                semantic.unit(),
                semantic.diagnostic_path().to_owned(),
                semantic.schedule(),
                semantic.storage(),
                semantic.failure_root(),
                semantic.initializer(),
                semantic.ensure(),
                resolved.into_references(),
            ));
        }
        Ok(Self::from_artifact(local.static_storages().clone(), units))
    }
}

#[derive(Debug)]
pub enum StrongInitializationUnitSemanticPlanV2BuildError {
    ProducerMismatch {
        module: ConeIdentity,
        foundation: ConeIdentity,
    },
    LocalSemantics(StrongInitializationUnitSemanticPlanBuildError),
    LocalDefinition(InitializationDefinitionResolutionErrorV2),
    ExternalUse(crate::StrongExternalInitializationUseErrorV2),
    UseConsumer {
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    UnknownLocalUnit(PersistentInitializationUnitId),
    ConflictingDefinition {
        unit: PersistentInitializationUnitId,
        first: ConeIdentity,
        second: ConeIdentity,
    },
    Definitions(InitializationDependencyResolutionError),
    Dependencies(InitializationDependencyResolutionError),
}

impl std::fmt::Display for StrongInitializationUnitSemanticPlanV2BuildError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "cannot project V2 initialization semantics: {self:?}"
        )
    }
}

impl std::error::Error for StrongInitializationUnitSemanticPlanV2BuildError {}
