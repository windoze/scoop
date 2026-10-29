use super::*;

#[test]
fn directory_calls_in_core_use_the_same_bound_input_rules() {
    let inputs = Arc::new(Inputs::default());
    let text = format!(
        r#"{DECLARATIONS}
        public fun defaulted(): EmbeddedDir = includeDir("local")
        public fun named(): EmbeddedDir = includeDir(exclude = [], path = "named", include = ["*.txt"])
        public fun environment(): Option<String> = env(name = "SCOOP_PRESENT")
        public fun text(): String = includeStr(path = "text")
        public fun bytes(): Array<Int8> = includeBytes(path = "bytes")
        "#
    );
    let mut declarations = super::super::super::complete_core_file();
    declarations
        .declarations
        .extend(scoop_parser::parse(&text).unwrap().declarations);
    let parsed = support::parsed_sources(
        super::super::super::core_source_identity("src/core.scoop"),
        declarations,
        "<core>",
        &text,
    );
    let world =
        hir::ImportedSemanticWorld::from_dependencies(parsed.cone(), vec![], vec![]).unwrap();
    let sources = CurrentConeSources::try_new(
        &parsed,
        crate::CoreProtocolInput::CurrentDeclarations,
        &world,
    )
    .unwrap()
    .with_compile_time_inputs(inputs.clone());
    let output =
        crate::lower_current_export(scoop_identity::RequestedConeKind::Library, &sources).unwrap();
    for (_, function) in output.export.module().functions.iter() {
        if !matches!(
            function.name.as_str(),
            "defaulted" | "named" | "environment" | "text" | "bytes"
        ) {
            continue;
        }
        let hir::FunctionKind::User(body) = &function.kind else {
            panic!("ordinary local body");
        };
        assert_eq!(body.statements.len(), 1, "no runtime input setup: {body:?}");
    }
    let queries = inputs.queries.lock().unwrap();
    assert!(
        queries
            .iter()
            .any(|query| query.starts_with("directory:local:"))
    );
    assert!(
        queries
            .iter()
            .any(|query| query.starts_with("directory:named:"))
    );
    for query in ["SCOOP_PRESENT", "text", "bytes"] {
        assert!(queries.iter().any(|actual| actual == query), "{queries:?}");
    }
}

#[test]
fn directory_calls_bind_defaults_and_named_inputs_before_static_expansion() {
    let inputs = Arc::new(Inputs::default());
    let output = lower(r#"
        import includeDir as embed
        import EmbeddedDir
        public fun defaulted(): EmbeddedDir = embed("default")
        public fun named(): EmbeddedDir = embed(exclude = ["private/**"], path = "named", include = ["**/" + "*.bin"])
        public fun empty(): EmbeddedDir = embed("empty", include = [])
        public fun <T> generic(value: T): EmbeddedDir = embed("generic")
        public fun parameter(value: EmbeddedDir = embed("parameter")): EmbeddedDir = value
    "#, inputs.clone()).unwrap();
    let module = output.export.module();
    for name in ["defaulted", "named", "empty", "generic"] {
        let (_, function) = module
            .functions
            .iter()
            .find(|(_, function)| function.name == name)
            .unwrap();
        let hir::FunctionKind::User(body) = &function.kind else {
            panic!("ordinary body");
        };
        let [statement] = body.statements.as_slice() else {
            panic!("no runtime argument setup: {body:?}");
        };
        let hir::StatementKind::Return { value } = &statement.kind else {
            panic!("return");
        };
        let hir::ExprKind::StructConstruct { fields, .. } = &value.kind else {
            panic!("ordinary directory value");
        };
        let [files] = fields.as_slice() else {
            panic!("one directory field");
        };
        let hir::ExprKind::EmbeddedFileArrayLiteral(files) = &files.kind else {
            panic!("static file table");
        };
        assert_eq!(
            files,
            &[hir::CompileTimeFile {
                path: "one.bin".into(),
                bytes: Arc::from([0, 255])
            }]
        );
    }
    let queries = inputs.queries.lock().unwrap();
    for (path, include, exclude) in [
        ("default", vec!["**"], vec![]),
        ("named", vec!["**/*.bin"], vec!["private/**"]),
        ("empty", vec![], vec![]),
        ("generic", vec!["**"], vec![]),
        ("parameter", vec!["**"], vec![]),
    ] {
        let filter = hir::CompileTimeDirectoryFilter {
            include: include.into_iter().map(String::from).collect(),
            exclude: exclude.into_iter().map(String::from).collect(),
        };
        assert!(
            queries.contains(&format!("directory:{path}:{filter:?}")),
            "{queries:?}"
        );
    }
}

#[test]
fn directory_runtime_inputs_and_callable_references_fail_before_io() {
    for (expression, expected) in [
        (
            "embed(path)",
            "requires a compile-time constant String path",
        ),
        (
            "embed(\"assets\", include = patterns)",
            "include requires an array literal",
        ),
        (
            "embed(\"assets\", exclude = [path])",
            "exclude elements must be compile-time constant Strings",
        ),
        (
            "embed(\"assets\", include = [patterns[0]])",
            "include elements must be compile-time constant Strings",
        ),
        ("::embed", "does not support callable references"),
    ] {
        let inputs = Arc::new(Inputs::default());
        let source = format!(
            "import includeDir as embed\nfun invalid(path: String, patterns: Array<String>) {{ val result = {expression} }}"
        );
        let diagnostics = lower(&source, inputs.clone()).err().expect("invalid input");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "{diagnostics:?}"
        );
        assert!(inputs.queries.lock().unwrap().is_empty());
    }
}
