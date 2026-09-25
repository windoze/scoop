//! Actual source bodies consume provider-owned helpers through layout selection.

use super::super::machine_selection::Source;
use super::*;
use scoop_identity::{ExactTypeDiagnosticCatalog, PersistentExactTypeId, PersistentTypeId};

mod rejections;

#[derive(Clone, Copy)]
pub(super) struct Provider<'a, 'p> {
    pub view: &'a lir::ShapeLinkProviderV1<'p>,
    pub layout: &'a lir::CrossConeLayoutAbiSectionV1<'p>,
    pub target: &'a scoop_toolchain::ResolvedTargetProfile,
    pub string: PersistentExactTypeId,
}

pub(super) fn check(
    name: &str,
    fixtures: &Path,
    input: scoop_mir_lower::MirTypeBridgeExportInputV1<'_>,
    callables: &lir::SelectedExternalLirSet,
    shapes: &[(ConeIdentity, PersistentTypeId)],
    provider: Provider<'_, '_>,
) {
    let plan = input.mir.materialization();
    assert!(plan.source_nominal_shapes().is_empty());
    assert!(plan.generated_nominal_shapes().is_empty());
    assert!(!plan.dependency_generated_nominal_shapes().is_empty());
    assert!(input.mir.module().meta.boxing_adjusts.is_empty());
    let mut physical = vec![(
        provider.layout.provider(),
        lir::ExternalStrongShapeSubjectV1::TypeDescriptor(provider.string),
    )];
    for root in plan.dependency_generated_nominal_shapes() {
        assert_eq!(root.provider(), provider.layout.provider());
        let shape = provider
            .layout
            .shape_support()
            .records()
            .iter()
            .find(|shape| shape.source_nominal() == root.source())
            .unwrap();
        assert_eq!(root.source_exact(), shape.exact());
        assert_eq!(
            root.exact(),
            shape.roles().boxed_value().available().unwrap().exact()
        );
        physical.push((
            root.provider(),
            lir::ExternalStrongShapeSubjectV1::TypeDescriptor(root.exact()),
        ));
    }
    physical.sort_unstable();
    physical.dedup();
    let mut roots = shapes
        .iter()
        .map(|(owner, source)| {
            lir::LayoutAbiDependencyV1::new(
                *owner,
                lir::LayoutAbiSemanticTargetV1::ShapeSupport(*source),
            )
        })
        .collect::<Vec<_>>();
    roots.push(lir::LayoutAbiDependencyV1::new(
        provider.layout.provider(),
        lir::LayoutAbiSemanticTargetV1::Descriptor(provider.string),
    ));
    roots.sort_unstable();
    let source = Source { roots, physical };
    let consumer = input.mir.module().cone;
    // These fixtures define only functions. This symbol surface serves the
    // pre-lowering TD import check; it is not a Compile artifact proof.
    let foundation =
        lir::OdrFreeLirFoundation::try_new(consumer, lir::CanonicalLirFoundation::empty()).unwrap();
    let definitions =
        lir::StrongObjectSymbolSurfaceV1::from_odr_free_foundation(&foundation).unwrap();
    let selected = lir::StrongProductionDependencySelectionV2::try_new(
        consumer,
        provider.target.lir_target(),
        &[provider.layout],
        source.imports(provider.view, consumer, &definitions),
        &source,
        &mut meter(),
    )
    .unwrap();
    let string = scoop_lir_lower::RuntimeStringDescriptor::External(
        selected
            .materialize_type_descriptor(provider.layout.provider(), provider.string, &mut meter())
            .unwrap(),
    );
    assert!(matches!(
        scoop_lir_lower::lower(input.mir, string, callables, provider.target.lir_target(),),
        Err(scoop_lir_lower::StrongLirLoweringError::MissingDependencyLayoutSelection { .. })
    ));

    let coordinate = ConeCoordinate::new("dev.example", "shape-consumer", "0.1.0").unwrap();
    assert_eq!(consumer, coordinate.identity().unwrap());
    let coordinates = [ConeCoordinate::reserved_core(), coordinate.clone()];
    let diagnostics =
        ExactTypeDiagnosticCatalog::try_new(input.identities, &coordinates, &mut meter()).unwrap();
    rejections::check(
        input.mir,
        callables,
        &diagnostics,
        string,
        &selected,
        &source,
        provider,
    );
    let output = scoop_lir_lower::lower_with_layout_dependencies(
        input.mir,
        string,
        callables,
        provider.target.lir_target(),
        &selected,
        &diagnostics,
        &mut meter(),
    )
    .unwrap_or_else(|error| panic!("{name} dependency machine lowering: {error}"));
    assert!(output.module().meta.type_descriptors.is_empty());
    assert!(output.module().meta.layouts.is_empty());
    assert_eq!(
        output.module().meta.external_type_descriptors.len(),
        source.physical.len()
    );
    snapshot(
        &fixtures.join(format!("{name}.machine.mir.snap")),
        &mir::dump(input.mir.module()),
    );
    snapshot(
        &fixtures.join(format!("{name}.machine.lir.snap")),
        &lir::dump(output.module()),
    );
    emit(name, &output, &coordinate, provider);
}

fn emit(
    name: &str,
    output: &lir::SingleConeStrongLirOutput,
    coordinate: &ConeCoordinate,
    provider: Provider<'_, '_>,
) {
    let profile = scoop_codegen::ValidatedBackendProfile::from_selection(
        provider.target.lir_target_selection(),
    )
    .unwrap();
    let dependencies = [provider.layout.provider()];
    let rendered = scoop_codegen::render_llvm_ir_members(
        output,
        coordinate,
        &dependencies,
        lir::EntryProductionSourceV1::Library,
        profile,
    )
    .unwrap();
    let ir = rendered
        .iter()
        .map(|member| member.llvm_ir())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(ir.contains("@scoop_rt_box_value"), "{ir}");
    assert!(ir.contains("@scoop_rt_is_instance"), "{ir}");
    if name == "combined" {
        assert!(ir.contains("@scoop_rt_box_zst"), "{ir}");
        assert!(ir.contains("@scoop_rt_unbox_value"), "{ir}");
    }
    for (_, descriptor) in output.module().meta.external_type_descriptors.iter() {
        let symbol = descriptor.expected_symbol().symbol();
        for declaration in ir.lines().filter(|line| {
            line.starts_with('@') && line.contains(symbol.as_str()) && line.contains(" = ")
        }) {
            assert!(declaration.contains("external"), "{declaration}");
        }
    }
    let directory = tempfile::tempdir().unwrap();
    let objects = scoop_codegen::emit_object_set(
        output,
        coordinate,
        &dependencies,
        lir::EntryProductionSourceV1::Library,
        directory.path(),
        profile,
    )
    .unwrap();
    assert_eq!(objects.members().len(), output.module().functions.len() + 1);
}
