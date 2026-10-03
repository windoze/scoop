use la_arena::Arena;
use scoop_identity::ConeCoordinate;
use scoop_lir::*;

use super::{boxed_exact, selection::Source};

pub(super) fn check(
    selected: &StrongProductionDependencySelectionV2<'_>,
    source: &Source,
    layout: &CrossConeLayoutAbiSectionV1<'_>,
    shape: &ParamFreeShapeSupportExportV1,
    external: &Arena<ExternalTypeDescriptor>,
    descriptor: ExternalTypeDescriptorId,
) {
    let construct = |provider, entries: &Arena<ExternalTypeDescriptor>| {
        selected.materialize_boxed_value_descriptor(
            provider,
            shape.source_nominal(),
            entries,
            descriptor,
            LirType::Aggregate(Vec::new()),
        )
    };
    assert!(matches!(
        construct(selected.consumer(), external),
        Err(LayoutExternalMaterializationError::ProviderPartition { .. })
    ));
    assert!(matches!(
        construct(layout.provider(), &Arena::new()),
        Err(LayoutExternalMaterializationError::BoxDescriptor(
            BoxDescriptorError::InvalidDescriptor
        ))
    ));
    let mut replaced = external.clone();
    replaced[descriptor] = ExternalTypeDescriptor::new(
        ConeCoordinate::new("test", "wrong-box-provider", "1.0.0")
            .unwrap()
            .identity()
            .unwrap(),
        external[descriptor].target(),
    )
    .unwrap();
    assert!(matches!(
        construct(layout.provider(), &replaced),
        Err(LayoutExternalMaterializationError::BoxDescriptor(
            BoxDescriptorError::InvalidDescriptor
        ))
    ));
    replaced[descriptor] = *external.iter().next().unwrap().1;
    assert!(matches!(
        construct(layout.provider(), &replaced),
        Err(LayoutExternalMaterializationError::BoxDescriptor(
            BoxDescriptorError::InvalidDescriptor
        ))
    ));

    let subject = ExternalStrongShapeSubjectV1::TypeDescriptor(boxed_exact(shape));
    let descriptor_only = Source {
        roots: vec![LayoutAbiDependencyV1::new(
            layout.provider(),
            LayoutAbiSemanticTargetV1::Descriptor(boxed_exact(shape)),
        )],
        physical: vec![(layout.provider(), subject)],
    };
    let descriptor_imports = selected
        .physical_imports()
        .records()
        .iter()
        .filter(|import| import.subject() == subject)
        .cloned()
        .collect();
    let unqualified = StrongProductionDependencySelectionV2::try_new(
        selected.consumer(),
        selected.target_profile(),
        &[layout.exports()],
        descriptor_imports,
        &descriptor_only.roots,
    )
    .unwrap();
    assert!(
        unqualified
            .materialize_type_descriptor(layout.provider(), boxed_exact(shape))
            .is_ok()
    );
    assert!(matches!(
        unqualified.materialize_boxed_value_descriptor(
            layout.provider(),
            shape.source_nominal(),
            external,
            descriptor,
            LirType::Aggregate(Vec::new()),
        ),
        Err(LayoutExternalMaterializationError::MissingShapeSupport { .. })
    ));

    let missing_import = Source {
        roots: source.roots.clone(),
        physical: source
            .physical
            .iter()
            .copied()
            .filter(|(_, entry)| *entry != subject)
            .collect(),
    };
    let retained_imports = selected
        .physical_imports()
        .records()
        .iter()
        .filter(|import| import.subject() != subject)
        .cloned()
        .collect();
    let incomplete = StrongProductionDependencySelectionV2::try_new(
        selected.consumer(),
        selected.target_profile(),
        &[layout.exports()],
        retained_imports,
        &missing_import.roots,
    )
    .unwrap();
    assert!(matches!(incomplete.materialize_boxed_value_descriptor(
        layout.provider(), shape.source_nominal(), external, descriptor,
        LirType::Aggregate(Vec::new()),
    ), Err(LayoutExternalMaterializationError::MissingPhysicalImport { subject: found, .. }) if found == subject));
}

pub(super) fn reference_shape(
    consumer: scoop_identity::ConeIdentity,
    layout: &CrossConeLayoutAbiSectionV1<'_>,
    shape: &ParamFreeShapeSupportExportV1,
    external: &Arena<ExternalTypeDescriptor>,
    descriptor: ExternalTypeDescriptorId,
) {
    let source = Source {
        roots: vec![LayoutAbiDependencyV1::new(
            layout.provider(),
            LayoutAbiSemanticTargetV1::ShapeSupport(shape.source_nominal()),
        )],
        physical: Vec::new(),
    };
    let selected = StrongProductionDependencySelectionV2::try_new(
        consumer,
        layout.target_profile(),
        &[layout.exports()],
        Vec::new(),
        &source.roots,
    )
    .unwrap();
    assert!(matches!(selected.materialize_boxed_value_descriptor(
        layout.provider(), shape.source_nominal(), external, descriptor,
        MANAGED_PTR,
    ), Err(LayoutExternalMaterializationError::UnavailableBoxedValue(found)) if found == shape.source_nominal()));
}

pub(super) fn codegen(
    mut module: Module,
    coordinate: &ConeCoordinate,
    provider: scoop_identity::ConeIdentity,
    profile: scoop_codegen::ValidatedBackendProfile,
    descriptor: ExternalTypeDescriptorId,
) {
    let original = module.meta.external_type_descriptors[descriptor];
    module.meta.external_type_descriptors[descriptor] = ExternalTypeDescriptor::new(
        ConeCoordinate::new("test", "rebound-box-provider", "1.0.0")
            .unwrap()
            .identity()
            .unwrap(),
        original.target(),
    )
    .unwrap();
    let output = ConeLirOutput::try_new(module, Vec::new()).unwrap();
    let error = scoop_codegen::render_llvm_ir_members(
        &output,
        coordinate,
        &[provider],
        EntryProductionSourceV1::Library,
        profile,
    )
    .unwrap_err();
    assert!(
        error.to_string().contains("BoxedValue descriptor"),
        "{error}"
    );
}
