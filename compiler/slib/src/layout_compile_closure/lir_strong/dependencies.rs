//! Borrow only the machine inputs required by the shared dependency catalog.
use super::{
    LirStrongProductionReplayedCrossConeLayoutSections as Sections, SharedLirStrongProductionError,
};
use crate::production_dependencies::{self, DefinitionProvider};

pub(super) fn definitions(
    consumer: scoop_identity::ConeIdentity,
    dependencies: &[&Sections<'_>],
) -> Result<
    (
        scoop_lir::StrongTypeReferenceDefinitionsV2,
        scoop_lir::StrongInitializationDefinitionCatalogV2,
    ),
    SharedLirStrongProductionError,
> {
    let inputs = dependencies
        .iter()
        .map(|dependency| DefinitionProvider {
            identities: dependency.prepared.shared_metadata().identities,
            foundation: dependency.prepared.lir_foundation(),
            strong: dependency.lir_strong_production(),
            layouts: dependency.layout.exports().layouts(),
        })
        .collect::<Vec<_>>();
    production_dependencies::definitions(consumer, &inputs)
}
