//! LIR-stage consumers of the actual provider's finite boxed helpers.

use la_arena::Arena;
use scoop_identity::{ConeCoordinate, PersistentExactTypeId};
use scoop_lir as lir;

mod consumer;
mod rejections;
use super::machine_selection as selection;

pub(super) fn check(
    name: &str,
    target: &scoop_toolchain::ResolvedTargetProfile,
    producer: &lir::SingleConeStrongLirOutput,
    layout: &lir::CrossConeLayoutAbiSectionV1<'_>,
    ordinary: &lir::CrossConeLirBridgeSectionV1,
    production: &lir::StrongProductionSectionV2,
) {
    let references = match name {
        "shared-shapes-standalone" => false,
        "shared-shapes-combined" => true,
        _ => return,
    };
    let empty = shape(producer, layout, "SharedShapeEmpty");
    let value = shape(producer, layout, "SharedShapeValue");
    let string = match producer.module().meta.well_known_type_descriptors.string {
        lir::TypeDescriptorRef::Local(id) => producer.module().meta.type_descriptors[id]
            .identity
            .exact_type(),
        lir::TypeDescriptorRef::External(_) => panic!("provider must define String"),
    };
    let coordinate = ConeCoordinate::new("test", "external-box-consumer", "1.0.0").unwrap();
    let mut module = consumer::module(
        coordinate.identity().unwrap(),
        layout.target_profile(),
        lir::ExternalTypeDescriptor::new(layout.provider(), string).unwrap(),
    );
    let foundation = lir::OdrFreeLirFoundation::from_module(&module).unwrap();
    let definitions =
        lir::StrongObjectSymbolSurfaceV1::from_odr_free_foundation(&foundation).unwrap();
    let provider = lir::ShapeLinkProviderV1::try_new(lir::ShapeLinkProviderPartsV1 {
        foundation: producer.foundation(),
        production,
        ordinary,
        layouts: layout.layouts(),
        callables: layout.callables(),
        descriptors: layout.descriptors(),
        dispatch: layout.dispatch(),
    })
    .unwrap();
    let source = selection::Source::new(layout.provider(), &[empty, value], string);
    let imports = source.imports(&provider, module.cone, &definitions);
    let selected = lir::StrongProductionDependencySelectionV2::try_new(
        module.cone,
        module.meta.target_profile,
        &[layout.exports()],
        imports.clone(),
        &source,
    )
    .unwrap();
    let complete = lir::CrossConeLayoutAbiSectionV1::try_new(
        selection::empty_exports(&foundation, module.meta.target_profile),
        &[layout.exports()],
        imports,
        &source,
    )
    .unwrap();
    module.meta.external_type_descriptors = Arena::new();
    let string_id = module.meta.external_type_descriptors.alloc(
        selected
            .materialize_type_descriptor(layout.provider(), string)
            .unwrap(),
    );
    module.meta.well_known_type_descriptors.string = lir::TypeDescriptorRef::External(string_id);
    rejections::reference_shape(
        module.cone,
        layout,
        shape(producer, layout, "SharedShapeReference"),
        &module.meta.external_type_descriptors,
        string_id,
    );
    let mut cases = Vec::new();
    for (shape, ty, offsets) in [
        (empty, lir::LirType::Aggregate(Vec::new()), Vec::new()),
        (
            value,
            lir::LirType::Aggregate(if references {
                vec![lir::MANAGED_PTR, lir::LirType::Aggregate(Vec::new())]
            } else {
                vec![lir::LirType::Aggregate(Vec::new()), lir::LirType::I32]
            }),
            if references { vec![0] } else { Vec::new() },
        ),
    ] {
        let exact = boxed_exact(shape);
        let id = module.meta.external_type_descriptors.alloc(
            selected
                .materialize_type_descriptor(layout.provider(), exact)
                .unwrap(),
        );
        let descriptor = selected
            .materialize_boxed_value_descriptor(
                layout.provider(),
                shape.source_nominal(),
                &module.meta.external_type_descriptors,
                id,
                ty.clone(),
            )
            .unwrap();
        assert_eq!(
            descriptor,
            complete
                .selected()
                .materialize_boxed_value_descriptor(
                    layout.provider(),
                    shape.source_nominal(),
                    &module.meta.external_type_descriptors,
                    id,
                    ty,
                )
                .unwrap()
        );
        rejections::check(
            &selected,
            &source,
            layout,
            shape,
            &module.meta.external_type_descriptors,
            id,
        );
        let function = consumer::roundtrip(module.cone, descriptor, &offsets);
        cases.push((function.callable_body.id(), id, offsets));
        module.functions.push(function);
    }
    assert!(module.meta.type_descriptors.is_empty());
    consumer::snapshot(&module, references);
    let output = lir::SingleConeStrongLirOutput::try_new(module, Vec::new(), None).unwrap();
    let profile =
        scoop_codegen::ValidatedBackendProfile::from_selection(target.lir_target_selection())
            .unwrap();
    let rendered = scoop_codegen::render_llvm_ir_members(
        &output,
        &coordinate,
        &[layout.provider()],
        lir::EntryProductionSourceV1::Library,
        profile,
    )
    .unwrap();
    consumer::check_ir(&rendered, output.module(), &cases);
    let directory = tempfile::tempdir().unwrap();
    let objects = scoop_codegen::emit_object_set(
        &output,
        &coordinate,
        &[layout.provider()],
        lir::EntryProductionSourceV1::Library,
        directory.path(),
        profile,
    )
    .unwrap();
    assert_eq!(objects.members().len(), cases.len() + 1);
    rejections::codegen(
        output.into_module(),
        &coordinate,
        layout.provider(),
        profile,
        cases[1].1,
    );
}

fn shape<'a>(
    producer: &lir::SingleConeStrongLirOutput,
    layout: &'a lir::CrossConeLayoutAbiSectionV1<'_>,
    name: &str,
) -> &'a lir::ParamFreeShapeSupportExportV1 {
    let exact = producer
        .shape_support()
        .roots()
        .iter()
        .find(|root| {
            root.declaration().name()
                == &scoop_identity::DeclarationName::Named(
                    scoop_identity::CanonicalIdentifier::new(name).unwrap(),
                )
        })
        .unwrap_or_else(|| panic!("missing provider source root for {name}"))
        .source()
        .exact();
    layout
        .shape_support()
        .records()
        .iter()
        .find(|record| record.exact() == exact)
        .unwrap()
}

fn boxed_exact(shape: &lir::ParamFreeShapeSupportExportV1) -> PersistentExactTypeId {
    shape.roles().boxed_value().available().unwrap().exact()
}
