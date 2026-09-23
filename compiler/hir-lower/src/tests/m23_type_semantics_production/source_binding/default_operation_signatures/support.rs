pub(super) use super::super::nominal_parameters::support::with_sources;
mod core;
use super::*;
pub(super) use core::{with_core_source, with_core_sources};

pub(super) fn with_source(
    source: &str,
    run: impl FnOnce(
        &hir::DependencyHirOutput,
        &hir::BoundNominalParameterProtocolsV1<'_, '_, '_, '_>,
        &hir::ImportedCoreFundamentalTypeProtocol,
    ),
) {
    with_sources(source, |output, fixture, sources, core| {
        let foundation = fixture.bind().unwrap();
        sources.with_bound(&foundation, core, |members, constructors| {
            let protocols = members
                .bind_parameter_protocols(constructors, &sources.protocols, &mut meter())
                .unwrap();
            run(output, &protocols, core);
        });
    });
}
pub(super) fn template(
    output: &hir::DependencyHirOutput,
    name: &str,
    position: u32,
) -> hir::DefaultSourceTemplateV1 {
    let name = format!("Samples.{name}");
    let id = output
        .output()
        .export
        .module()
        .functions
        .iter()
        .find(|(_, f)| f.name == name)
        .unwrap()
        .0;
    hir::DefaultSourceBodyProductionV1::from_dependency_hir(
        output,
        hir::ExportParameterOwner::Function(id),
        position,
        &mut meter(),
    )
    .unwrap()
    .into_source_template(&mut meter())
    .unwrap()
}
pub(super) fn callables(template: &hir::DefaultSourceTemplateV1) -> Vec<hir::DefaultCallableRefV1> {
    template
        .references()
        .callables()
        .iter()
        .map(|record| match record.target() {
            hir::ExportDefaultCallableTargetV1::Callable(reference) => reference.clone(),
            target => panic!("expected a direct member: {target:?}"),
        })
        .collect()
}
pub(super) fn constructor(template: &hir::DefaultSourceTemplateV1) -> hir::DefaultConstructorRefV1 {
    let [record] = template.references().constructors() else {
        panic!("one constructor reference");
    };
    record.target().clone()
}
