//! Physical dependency selection follows actual MIR references.

use super::*;
use LayoutProductionError as Error;
use scoop_lir as lir;
use scoop_mir as mir;
use scoop_slib as slib;

pub(in crate::request::preflight) fn select_lir_dependencies<'a>(
    input: &mir::ConeMirInput,
    initialization: impl IntoIterator<
        Item = (ConeIdentity, scoop_identity::PersistentInitializationUnitId),
    >,
    dependencies: &[&'a slib::PhysicalImportsReplayedCrossConeLayoutSections],
    target: lir::LirTargetProfile,
) -> Result<lir::StrongProductionDependencySelectionV2<'a>, Error> {
    let mut roots = Vec::new();
    let mut physical = Vec::new();
    collect_mir_references(
        input,
        &dependencies
            .iter()
            .map(|dependency| dependency.lir_exports())
            .collect::<Vec<_>>(),
        &mut roots,
        &mut physical,
    )?;
    physical.extend(initialization.into_iter().map(|(provider, unit)| {
        (
            provider,
            lir::ExternalStrongShapeSubjectV1::InitializationRegistration(unit),
        )
    }));
    roots.sort_unstable();
    roots.dedup();
    physical.sort_unstable();
    physical.dedup();
    let providers = dependencies
        .iter()
        .map(|dependency| {
            let exports = dependency.lir_exports();
            lir::ShapeLinkProviderV1::try_new(lir::ShapeLinkProviderPartsV1 {
                foundation: dependency.lir_foundation(),
                production: dependency.lir_strong_production(),
                ordinary: dependency.lir_cross_cone_bridge(),
                layouts: exports.layouts(),
                callables: exports.callables(),
                descriptors: exports.descriptors(),
                dispatch: exports.dispatch(),
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(Error::Shape)?;
    let imports = physical
        .into_iter()
        .map(|(identity, subject)| {
            let provider = providers
                .iter()
                .find(|provider| provider.provider() == identity)
                .ok_or(lir::ShapeLinkError::Provider)?;
            lir::ExternalShapeLinkImportV1::replay(provider, subject, input.module().cone)
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(Error::Shape)?;
    lir::StrongProductionDependencySelectionV2::try_new(
        input.module().cone,
        target,
        &dependencies
            .iter()
            .map(|d| d.lir_exports())
            .collect::<Vec<_>>(),
        imports,
        &roots,
    )
    .map_err(Error::Selection)
}
