use super::*;
use scoop_mir as mir;
use scoop_mir_lower::{
    MirTypeBridgeDependencyTablesV1, MirTypeBridgeExportInputV1,
    MirTypeBridgeExportProductionError as Error, lower_type_bridge_exports,
};

mod assertions;
mod initialization;
mod rejections;
mod source_projection;

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}

fn fixture(name: &str) -> (std::path::PathBuf, String) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m23-mir-export-assembly");
    let source = std::fs::read_to_string(path.join(format!("{name}.scoop"))).unwrap();
    (path, source)
}

fn with_exports<R>(
    source: &str,
    run: impl FnOnce(
        MirTypeBridgeExportInputV1<'_>,
        &mir::CanonicalParamFreeMirTypeExportsV1,
        &mir::MirTypeBridgeExportConstituentsV1,
    ) -> R,
) -> R {
    with_production(source, |output, input, hir, graph, _| {
        let public = public_interface(output);
        let core = trusted_core();
        let world = core.world(input.module().cone);
        let nominals = world
            .direct_provider(ConeIdentity::CORE)
            .unwrap()
            .nominal_interfaces();
        let classifier =
            hir::NominalExactLeafClassifierV1::try_from_nominal_interfaces(nominals.records())
                .unwrap();
        let ordinary = scoop_mir_lower::lower_cross_cone_bridge_section(
            input.module().cone,
            &public,
            &classifier,
            input.foundation(),
            &mir::SelectedExternalMirSet::empty(input.module().cone),
        )
        .unwrap();
        let mut dependencies = dependencies::unit(input, graph).into_records();
        if input
            .module()
            .meta
            .source_exact_types
            .get(&mir::Type::Boolean)
            .is_some()
        {
            dependencies.extend(dependencies::boolean(input, graph).into_records());
        }
        let dependencies = mir::CanonicalParamFreeMirTypeExportsV1::try_new(dependencies).unwrap();
        let context = MirTypeBridgeExportInputV1 {
            hir: output,
            public: &public,
            source: hir,
            mir: input,
            ordinary: &ordinary,
            nominal_classifier: &classifier,
            identities: graph,
        };
        let exports = produce(context, &[&dependencies], &mut meter()).unwrap();
        run(context, &dependencies, &exports)
    })
}

fn produce(
    input: MirTypeBridgeExportInputV1<'_>,
    types: &[&mir::CanonicalParamFreeMirTypeExportsV1],
    meter: &mut BudgetMeter,
) -> Result<mir::MirTypeBridgeExportConstituentsV1, Error> {
    let uses = mir::CanonicalMirExternalInitializationUsesV1::try_new(vec![], meter).unwrap();
    lower_type_bridge_exports(
        input,
        MirTypeBridgeDependencyTablesV1 {
            types,
            callables: &[],
            dispatch: &[],
        },
        uses,
        meter,
    )
}

#[test]
fn actual_mir_export_assembly_completes_all_tables_and_keeps_ordinary_partition() {
    for name in ["standalone", "combined"] {
        let (directory, source) = fixture(name);
        let (bytes, dump) = with_exports(&source, |input, dependencies, exports| {
            assertions::actual(input, dependencies, exports);
            assertions::roundtrip(input, dependencies, exports);
            (assertions::bytes(exports), assertions::dump(input, exports))
        });
        with_exports(
            &format!("private struct Unrelated() {{}}\n{source}"),
            |_, _, exports| assert_eq!(assertions::bytes(exports), bytes),
        );
        if let Some(path) = std::env::var_os("SCOOP_MIR_EXPORT_SNAPSHOT_DIR") {
            std::fs::create_dir_all(&path).unwrap();
            std::fs::write(
                std::path::Path::new(&path).join(format!("{name}.snap")),
                dump,
            )
            .unwrap();
        } else {
            assert_eq!(
                dump,
                std::fs::read_to_string(directory.join(format!("{name}.snap"))).unwrap()
            );
        }
    }
}

#[test]
fn actual_mir_export_assembly_requires_complete_dependencies_and_one_shared_budget() {
    let (_, source) = fixture("combined");
    with_exports(&source, |input, dependencies, _| {
        rejections::check(input, dependencies);
    });
}
