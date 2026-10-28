use super::*;
use scoop_hir as hir;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Inputs {
    queries: Mutex<Vec<String>>,
}

impl hir::CompileTimeInputHost for Inputs {
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

fn lower(
    source: &str,
    inputs: Arc<Inputs>,
) -> Result<hir::DependencyExportHirOutput, Vec<scoop_ast::Diagnostic>> {
    let mut declarations = super::super::complete_core_file();
    let declaration = "@Intrinsic(\"compile_env\") public fun env(name: String): Option<String>";
    declarations
        .declarations
        .extend(scoop_parser::parse(declaration).unwrap().declarations);
    let core = support::trusted_core_from_source(declarations, declaration);
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
