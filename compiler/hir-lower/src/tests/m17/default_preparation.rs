use super::super::*;

mod casts;
mod dependency_binders;
mod errors;
mod files;
mod generic_references;
mod local_calls;
mod local_captures;
mod local_own_binders;
mod local_owner_arguments;

const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-default-preparation/dependencies.scoop"
));

fn lower_source(source: &str) -> Result<hir::Output, Vec<Diagnostic>> {
    lower(&[complete_core_file(), scoop_parser::parse(source).unwrap()])
}

fn source_blocks(dump: &str) -> String {
    let mut keep = false;
    let mut result = String::new();
    for line in dump.lines() {
        if line.starts_with("  ") && !line.starts_with("   ") {
            keep = line.contains("Prep") || line.contains("prep") || line.starts_with("  fun main");
        }
        if keep {
            result.push_str(line);
            result.push('\n');
        }
    }
    result
}

#[test]
fn forward_default_dependencies_lower_all_callable_roles_and_captures() {
    let output = lower_source(SOURCE).unwrap();
    let hir_dump = source_blocks(&hir::dump(&output.export));
    assert!(hir_dump.contains("prepForward"));
    assert_eq!(
        hir_dump,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-default-preparation/dependencies.hir.snap"
        ))
    );
    let mir = scoop_mir_lower::lower(&output.local).unwrap();
    let mir_dump = source_blocks(&scoop_mir::dump(&mir));
    assert!(mir_dump.contains("prepForward"));
    assert_eq!(
        mir_dump,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-default-preparation/dependencies.mir.snap"
        ))
    );
}

#[test]
fn defaults_do_not_prepare_an_unselected_overload_or_an_explicit_argument() {
    let output = lower_source("fun prep(value: Int = select(1)): Int = value\nfun select(value: Int): Int = value\nfun select(value: String = \"other\"): Int = 0\nfun main() { prep() }").unwrap();
    let export = output.export.module();
    let (prep, _) = export
        .functions
        .iter()
        .find(|(_, f)| f.name == "prep")
        .unwrap();
    let interface = export
        .source_parameter_interfaces
        .iter()
        .find(|p| p.owner == hir::ExportParameterOwner::Function(prep))
        .unwrap();
    let hir::ExportParameterCalling::Default { source, value_type } =
        interface.parameters[0].calling
    else {
        panic!("default");
    };
    let body =
        &export.export_default_exprs[export.export_default_sources[source].declared().unwrap().0];
    assert_eq!(body.references.callables.len(), 1);
    assert_eq!(body.value.ty, value_type);
}
