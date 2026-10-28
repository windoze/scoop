use super::*;

mod directories;
use scoop_hir as hir;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Inputs {
    queries: Mutex<Vec<String>>,
}

impl hir::CompileTimeInputHost for Inputs {
    fn directory(
        &self,
        _source: &scoop_identity::SourceIdentity,
        path: &hir::CompileTimePath,
        filter: &hir::CompileTimeDirectoryFilter,
    ) -> Result<Arc<[hir::CompileTimeFile]>, hir::CompileTimeDirectoryError> {
        self.queries
            .lock()
            .unwrap()
            .push(format!("directory:{}:{filter:?}", path.as_str()));
        Ok(Arc::from([hir::CompileTimeFile {
            path: "one.bin".into(),
            bytes: Arc::from([0, 255]),
        }]))
    }

    fn file(
        &self,
        _: &scoop_identity::SourceIdentity,
        path: &hir::CompileTimePath,
    ) -> Result<Arc<[u8]>, String> {
        self.queries.lock().unwrap().push(path.as_str().to_owned());
        if path.as_str() == "invalid-utf8" {
            return Ok(Arc::from(b"ok\xff".as_slice()));
        }
        Ok(Arc::from(b"\xef\xbb\xbfhello\r\n\0".as_slice()))
    }
    fn environment(
        &self,
        name: &hir::CompileTimeEnvironmentName,
    ) -> Result<Option<String>, String> {
        self.queries.lock().unwrap().push(name.as_str().to_owned());
        Ok(match name.as_str() {
            "SCOOP_PRESENT" => Some("hello".into()),
            "SCOOP_EMPTY" | "SCOOP_" => Some(String::new()),
            _ => None,
        })
    }
}

const DECLARATIONS: &str = r#"
@Intrinsic("compile_env") public fun env(name: String): Option<String>
@Intrinsic("include_str") public fun includeStr(path: String): String
@Intrinsic("include_bytes") public fun includeBytes(path: String): Array<Int8>
public struct EmbeddedFile(val path: String, val bytes: Array<Int8>)
public struct EmbeddedDir(val files: Array<EmbeddedFile>)
@Intrinsic("include_dir") public fun includeDir(path: String, include: Array<String> = ["**"], exclude: Array<String> = []): EmbeddedDir
"#;

fn lower(
    source: &str,
    inputs: Arc<Inputs>,
) -> Result<hir::DependencyExportHirOutput, Vec<scoop_ast::Diagnostic>> {
    let mut declarations = super::super::complete_core_file();

    declarations
        .declarations
        .extend(scoop_parser::parse(DECLARATIONS).unwrap().declarations);
    let core = support::trusted_core_from_source(declarations, DECLARATIONS);
    let parsed = support::parsed_ordinary_text(source);
    let protocols = core.foundation.import_core_inputs(&core.interface).unwrap();
    let world = core.world(parsed.cone());
    let input = CurrentConeSources::try_new(&parsed, protocols, &world)
        .unwrap()
        .with_compile_time_inputs(inputs);
    crate::lower_current_export(scoop_identity::RequestedConeKind::Library, &input)
}

#[test]
fn compile_env_expands_before_generic_specialization_and_preserves_empty() {
    let inputs = Arc::new(Inputs::default());
    let output = lower(
        r#"
        import env as compileEnv
        public fun present(): String? = compileEnv("SCOOP_PRESENT")
        public fun empty(): String? = compileEnv("SCOOP_EMPTY")
        public fun missing(): String? = compileEnv("SCOOP_MISSING")
        public fun concatenated(): String? = compileEnv("SCOOP_" + "PRESENT")
        public fun defaulted(value: String? = compileEnv("SCOOP_DEFAULT")): String? = value
        public fun <T> generic(value: T): String? = compileEnv("SCOOP_")
        "#,
        inputs.clone(),
    )
    .unwrap();
    let export = output.export.module();
    for (name, expected) in [
        ("present", Some("hello")),
        ("empty", Some("")),
        ("missing", None),
        ("concatenated", Some("hello")),
        ("generic", Some("")),
    ] {
        let (_, function) = export
            .functions
            .iter()
            .find(|(_, value)| value.name == name)
            .unwrap();
        let hir::FunctionKind::User(body) = &function.kind else {
            panic!("ordinary function body")
        };
        let [statement] = body.statements.as_slice() else {
            panic!("compile-time input needs no runtime argument setup")
        };
        let hir::StatementKind::Return { value } = &statement.kind else {
            panic!("expression-bodied return")
        };
        let hir::ExprKind::VariantConstruct { args, .. } = &value.kind else {
            panic!("env is already an ordinary enum constant in Export HIR")
        };
        assert_eq!(args.len(), usize::from(expected.is_some()));
        if let Some(expected) = expected {
            assert!(
                matches!(&args[0].kind, hir::ExprKind::StringLiteral { value, .. } if value == expected)
            );
        }
    }
    let queries = inputs.queries.lock().unwrap();
    for name in [
        "SCOOP_PRESENT",
        "SCOOP_EMPTY",
        "SCOOP_MISSING",
        "SCOOP_",
        "SCOOP_DEFAULT",
    ] {
        assert!(queries.iter().any(|value| value == name));
    }
}

#[test]
fn compile_env_rejects_invalid_names_and_runtime_inputs_before_host_lookup() {
    for (expression, expected) in [
        ("compileEnv(\"HOME\")", "must start with `SCOOP_`"),
        ("compileEnv(\"scoop_VALUE\")", "must start with `SCOOP_`"),
        ("compileEnv(\"SCOOP_A=B\")", "must not contain `=` or NUL"),
        (
            "compileEnv(\"SCOOP_A\\u{0}\")",
            "must not contain `=` or NUL",
        ),
        (
            "compileEnv(name)",
            "requires a compile-time constant String",
        ),
        ("::compileEnv", "does not support callable references"),
    ] {
        let inputs = Arc::new(Inputs::default());
        let diagnostics = lower(
            &format!("import env as compileEnv\nfun invalid(name: String) {{ val result = {expression} }}"),
            inputs.clone(),
        )
        .err().expect("invalid compile-time input must be rejected");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "{diagnostics:?}"
        );
        assert!(inputs.queries.lock().unwrap().is_empty());
    }
}

#[test]
fn compile_env_const_initializer_folds_forward_string_arguments() {
    let inputs = Arc::new(Inputs::default());
    let output = lower(
        r#"
        import env as compileEnv
        public const val value: String? = compileEnv(name = PREFIX + "PRESENT")
        const val PREFIX: String = "SCOOP_"
        public const val empty: String? = compileEnv("SCOOP_EMPTY")
        public const val missing: String? = compileEnv("SCOOP_MISSING")
        public fun read(): String? = value
        "#,
        inputs,
    )
    .unwrap();
    for (name, expected) in [
        ("value", Some("hello")),
        ("empty", Some("")),
        ("missing", None),
    ] {
        let (_, property) = output
            .export
            .module()
            .properties
            .iter()
            .find(|(_, property)| property.name == name)
            .unwrap();
        let hir::PropertyRepresentation::Const {
            value: hir::HirConstantImage::Enum { fields, .. },
        } = &property.representation
        else {
            panic!("env produces an ordinary typed enum constant")
        };
        assert_eq!(fields.len(), usize::from(expected.is_some()));
        if let Some(expected) = expected {
            assert_eq!(fields, &[hir::HirConstantImage::String(expected.into())]);
        }
    }
}

#[test]
fn compile_env_constant_call_does_not_admit_constructors_or_same_named_functions() {
    for source in [
        "fun env(name: String): String = name\nconst val value: String = env(\"SCOOP_PRESENT\")",
        "const val value: String? = Some(\"SCOOP_PRESENT\")",
        "struct Name(val value: String)\nconst val value: Name = Name(\"SCOOP_PRESENT\")",
    ] {
        let inputs = Arc::new(Inputs::default());
        let diagnostics = lower(source, inputs.clone())
            .err()
            .expect("ordinary calls are not constant expressions");
        assert!(
            diagnostics
                .iter()
                .any(|d| d.message.contains("ordinary functions and constructors")),
            "{diagnostics:?}"
        );
        assert!(inputs.queries.lock().unwrap().is_empty());
    }
}

#[test]
fn include_str_expands_literals_in_generic_bodies_and_defaults() {
    let inputs = Arc::new(Inputs::default());
    let output = lower(
        r#"
        import includeStr as embed
        public fun <T> generic(v: T): String = embed("file" + ".txt")
        public fun defaulted(v: String = embed("default.txt")): String = v
        public fun read(): String = embed("file.txt")
        "#,
        inputs.clone(),
    )
    .unwrap();
    for name in ["generic", "read"] {
        let function = output
            .export
            .module()
            .functions
            .iter()
            .find(|(_, function)| function.name == name)
            .unwrap()
            .1;
        let hir::FunctionKind::User(body) = &function.kind else {
            panic!("ordinary body")
        };
        let [statement] = body.statements.as_slice() else {
            panic!("no runtime argument evaluation")
        };
        let hir::StatementKind::Return { value } = &statement.kind else {
            panic!("return")
        };
        assert!(
            matches!(&value.kind, hir::ExprKind::StringLiteral { value, .. } if value == "\u{feff}hello\r\n\0")
        );
    }
    assert!(
        inputs
            .queries
            .lock()
            .unwrap()
            .iter()
            .any(|path| path == "default.txt")
    );
}

#[test]
fn include_str_rejects_invalid_paths_references_runtime_arguments_and_utf8() {
    for (expression, expected, queried) in [
        ("embed(\"\")", "must be nonempty", false),
        ("embed(\"a\\u{0}b\")", "must not contain NUL", false),
        (
            "embed(path)",
            "requires a compile-time constant String path",
            false,
        ),
        ("::embed", "does not support callable references", false),
        ("embed(\"invalid-utf8\")", "byte offset 2", true),
    ] {
        let inputs = Arc::new(Inputs::default());
        let diagnostics = lower(&format!("import includeStr as embed\nfun invalid(path: String) {{ val result = {expression} }}"), inputs.clone()).err().expect("invalid include");
        assert!(
            diagnostics.iter().any(|d| d.message.contains(expected)),
            "{diagnostics:?}"
        );
        assert_eq!(!inputs.queries.lock().unwrap().is_empty(), queried);
    }
}

#[test]
fn include_bytes_embeds_non_utf8_in_generic_bodies_and_defaults() {
    let inputs = Arc::new(Inputs::default());
    let output = lower(
        r#"
        import includeBytes as embed
        public fun <T> generic(v: T): Array<Int8> = embed("invalid-utf8")
        public fun defaulted(v: Array<Int8> = embed("default.bin")): Array<Int8> = v
        public fun read(): Array<Int8> = embed("invalid-utf8")
    "#,
        inputs.clone(),
    )
    .unwrap();
    for name in ["generic", "read"] {
        let function = output
            .export
            .module()
            .functions
            .iter()
            .find(|(_, f)| f.name == name)
            .unwrap()
            .1;
        let hir::FunctionKind::User(body) = &function.kind else {
            panic!("ordinary body")
        };
        let [statement] = body.statements.as_slice() else {
            panic!("no runtime argument evaluation")
        };
        let hir::StatementKind::Return { value } = &statement.kind else {
            panic!("return")
        };
        assert!(
            matches!(&value.kind, hir::ExprKind::ByteArrayLiteral(bytes) if bytes == b"ok\xff")
        );
    }
    assert!(
        inputs
            .queries
            .lock()
            .unwrap()
            .iter()
            .any(|path| path == "default.bin")
    );
}

#[test]
fn include_bytes_rejects_runtime_paths_and_callable_references() {
    for (expression, expected) in [
        (
            "embed(path)",
            "requires a compile-time constant String path",
        ),
        ("::embed", "does not support callable references"),
        ("embed(\"\")", "must be nonempty"),
    ] {
        let inputs = Arc::new(Inputs::default());
        let diagnostics = lower(&format!("import includeBytes as embed\nfun invalid(path: String) {{ val result = {expression} }}"), inputs.clone()).err().expect("invalid include");
        assert!(
            diagnostics.iter().any(|d| d.message.contains(expected)),
            "{diagnostics:?}"
        );
        assert!(inputs.queries.lock().unwrap().is_empty());
    }
}
