use super::super::*;

mod casts;
mod dependency_binders;
mod files;
mod generic_references;
mod local_calls;
mod local_captures;
mod local_owner_arguments;

fn lower_source(source: &str) -> Result<hir::Output, Vec<Diagnostic>> {
    lower(&[complete_core_file(), scoop_parser::parse(source).unwrap()])
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
