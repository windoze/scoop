use super::*;

#[test]
fn constructor_materialization_closes_selected_bodies_and_keeps_source_contracts() {
    for (source, expected) in [
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m19-constructor-materialization/selection.scoop"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m19-constructor-materialization/selection.snap"
            )),
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m19-constructor-materialization/closure.scoop"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m19-constructor-materialization/closure.snap"
            )),
        ),
    ] {
        let output = lower(&[complete_core_file(), scoop_parser::parse(source).unwrap()]).unwrap();
        let mut rows = Vec::new();
        for (_, constructor) in output.export.class_constructors.iter() {
            let owner = &output.export.classes[constructor.owner].name;
            if owner.starts_with("Deferred") {
                rows.push(format!(
                    "source class {owner}/{}\n",
                    constructor.parameters.len()
                ));
            }
        }
        for (_, constructor) in output.export.struct_constructors.iter() {
            let owner = &output.export.structs[constructor.owner].name;
            if owner.starts_with("Deferred") {
                rows.push(format!(
                    "source struct {owner}/{}\n",
                    constructor.parameters.len()
                ));
            }
        }
        for (_, constructor) in output.local.class_constructors.iter() {
            let owner = &output.local.classes[constructor.class].name;
            if owner.starts_with("Deferred") {
                rows.push(format!(
                    "concrete class {owner}/{}\n",
                    constructor.parameters.len()
                ));
            }
        }
        for (_, constructor) in output.local.struct_constructors.iter() {
            let owner = &output.local.structs[constructor.structure].name;
            if owner.starts_with("Deferred") {
                rows.push(format!(
                    "concrete struct {owner}/{}\n",
                    constructor.parameters.len()
                ));
            }
        }
        if source.contains("DeferredTypeOnly") {
            assert!(
                output
                    .local
                    .classes
                    .iter()
                    .any(|(_, c)| c.name == "DeferredTypeOnly")
            );
        }
        // The unused overload must not instantiate its GC-free helper for String.
        assert!(
            output
                .local
                .functions
                .iter()
                .all(|(_, f)| !f.name.starts_with("gcFree"))
        );
        let mir = scoop_mir_lower::lower(&output.local).unwrap();
        for (_, function) in mir.functions.iter() {
            if function.name.starts_with("init.Deferred")
                || function.name.starts_with("ctor.Deferred")
            {
                let (name, _) = function.name.split_once(".$c").unwrap();
                rows.push(format!(
                    "mir {name}/{}: {:?}\n",
                    function.params.len(),
                    function.gc_effect
                ));
            }
        }
        rows.sort();
        assert_eq!(rows.concat(), expected);
    }
}
