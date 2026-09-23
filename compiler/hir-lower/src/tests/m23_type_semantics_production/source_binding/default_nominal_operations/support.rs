use super::*;

pub(super) fn with_source(
    source: &str,
    run: impl FnOnce(&hir::DependencyHirOutput, &hir::BoundNominalSourceContractsV1<'_, '_>),
) {
    super::super::nominal_parameters::support::with_sources(
        source,
        |output, fixture, sources, _| {
            let foundation = fixture.bind().unwrap();
            let nominals = foundation
                .bind_nominal_sources(&sources.members.nominals, &mut meter())
                .unwrap();
            run(output, &nominals);
        },
    );
}
pub(super) fn value_type(output: &hir::DependencyHirOutput, name: &str, position: u32) -> Type {
    let export = output.output().export.module();
    let id = export
        .functions
        .iter()
        .find(|(_, function)| function.name == format!("Samples.{name}"))
        .unwrap()
        .0;
    hir::DefaultSourceBodyProductionV1::from_dependency_hir(
        output,
        hir::ExportParameterOwner::Function(id),
        position,
        &mut meter(),
    )
    .unwrap()
    .result()
    .clone()
}
pub(super) fn owner(ty: &Type) -> hir::SourceNominalId {
    match ty {
        Type::Nominal(id) => hir::SourceNominalId::Concrete(*id),
        Type::NominalApplication { origin, .. } => hir::SourceNominalId::GenericTemplate(*origin),
        _ => panic!("fixture nominal type"),
    }
}
pub(super) fn source<'a>(
    nominals: &'a hir::BoundNominalSourceContractsV1<'_, '_>,
    ty: &Type,
) -> &'a hir::NominalSourceContractV1 {
    nominals.nominal_source(owner(ty)).unwrap()
}
pub(super) fn query(
    nominals: &hir::BoundNominalSourceContractsV1<'_, '_>,
    target: Target<'_>,
) -> Shape {
    nominals
        .default_nominal_operation_shape(target, &mut meter(), &WirePath::root())
        .unwrap()
}
pub(super) fn shape_type(
    nominals: &hir::BoundNominalSourceContractsV1<'_, '_>,
    ty: &Type,
    index: usize,
) -> Type {
    let Shape::Aggregate(shape) = query(nominals, Target::Struct(ty)) else {
        panic!("struct aggregate");
    };
    shape.fields()[index].clone()
}
