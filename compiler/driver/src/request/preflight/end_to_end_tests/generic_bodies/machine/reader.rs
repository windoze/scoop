use scoop_identity::{ConeCoordinate, ConeIdentity};
use scoop_lir::{
    ConeLirOutput, ConeProductionSectionV2, EntryProductionSourceV1, ExternalStrongShapeSubjectV1,
};
use scoop_wire::{decode_canonical, encode};

pub(super) fn check(
    input: &ConeLirOutput,
    production: &ConeProductionSectionV2,
    coordinate: &ConeCoordinate,
    direct_dependencies: &[ConeIdentity],
    dependencies: &[&scoop_slib::PhysicalImportsReplayedCrossConeLayoutSections],
    selected: &scoop_lir::StrongProductionDependencySelectionV2<'_>,
) {
    let definitions = selected
        .physical_imports()
        .records()
        .iter()
        .filter(|import| {
            matches!(
                import.subject(),
                ExternalStrongShapeSubjectV1::TypeDescriptor(_)
                    | ExternalStrongShapeSubjectV1::Callable(_)
            )
        })
        .map(|import| {
            let provider = dependencies
                .iter()
                .find(|dependency| dependency.identity() == import.provider())
                .unwrap();
            scoop_lir::StrongShapeDefinitionRefV1::from_foundation(
                import.subject(),
                provider.lir_foundation(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let definitions = scoop_lir::StrongTypeReferenceDefinitionsV2::new(
        input.module().cone,
        &definitions,
        &dependencies
            .iter()
            .map(|dependency| dependency.lir_exports().layouts())
            .collect::<Vec<_>>(),
    )
    .unwrap();
    let initialization =
        scoop_lir::StrongInitializationDefinitionCatalogV2::new(input.module().cone, &[]).unwrap();
    let bytes = encode(production).unwrap();
    let decoded = decode_canonical::<scoop_lir::DecodedConeProductionSectionV2>(&bytes).unwrap();
    let replayed = decoded
        .replay(
            coordinate.clone(),
            direct_dependencies,
            input.module().meta.target_profile,
            input.foundation(),
            EntryProductionSourceV1::Library,
            &input
                .shape_support()
                .roots()
                .iter()
                .map(|root| root.declaration().clone())
                .collect::<Vec<_>>(),
            &definitions,
            &initialization,
        )
        .unwrap();
    assert_eq!(encode(&replayed).unwrap(), bytes);
}
