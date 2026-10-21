use super::*;

fn core_with_documentation() -> ast::SourceFile {
    let mut core = complete_core_file();
    for source in [
        include_str!("../../../../sysroot/src/scoop.core/src/serialization_annotations.scoop"),
        include_str!("../../../../sysroot/src/scoop.core/src/documentation.scoop"),
    ] {
        core.declarations
            .extend(scoop_parser::parse(source).unwrap().declarations);
    }
    core
}

fn applications(source: &str) -> Vec<ast::Annotation> {
    let parsed = scoop_parser::parse(&format!("{source}\nstruct Marker")).unwrap();
    let Decl::Struct(declaration) = &parsed.declarations[0] else {
        panic!()
    };
    declaration.annotations.clone()
}

fn bootstrap(files: Vec<ast::SourceFile>) -> Result<hir::Output, Vec<ast::Diagnostic>> {
    let mut parsed = Vec::new();
    let mut texts = Vec::new();
    let mut contexts = Vec::new();
    for (index, file) in files.into_iter().enumerate() {
        let identity = core_source_identity(&format!("src/doc-{index}.scoop"));
        parsed.push(ast::IdentifiedParsedSource::new(identity.clone(), file));
        texts.push(ast::CurrentSourceText::new(identity.clone(), String::new()));
        contexts.push(ast::CurrentSourceDiagnosticContext::new(
            identity,
            std::path::PathBuf::from("<core>"),
        ));
    }
    let input = ast::CurrentConeParsedSources::try_new(
        ast::AllParsedSources::try_new(ast::NonEmptyVec::new(parsed.remove(0), parsed)).unwrap(),
        ast::NonEmptyVec::new(texts.remove(0), texts),
        ast::NonEmptyVec::new(contexts.remove(0), contexts),
    )
    .unwrap();
    crate::lower_core_bootstrap(&input)
}

#[test]
fn m60_doc_annotations_bind_aliases_repeatability_and_exact_targets() {
    let source = scoop_parser::parse(
        r#"
        import DocHidden as Omit
        import DocAlias as Search
        @Search("record") @Search("entry")
        struct Item(@Omit val value: Int)
        @Omit fun helper(): Int = 4
        fun main() { helper() }
        "#,
    )
    .unwrap();
    let output = bootstrap(vec![core_with_documentation(), source]).unwrap();
    let module = output.export.module();
    let (item, _) = module
        .structs
        .iter()
        .find(|(_, structure)| structure.name == "Item")
        .unwrap();
    let annotations = &module
        .annotations
        .targets
        .iter()
        .find(|entry| {
            entry.target == hir::SourceAnnotationTarget::Nominal(hir::NominalOwner::Struct(item))
        })
        .unwrap()
        .annotations;
    assert_eq!(annotations.len(), 2);
    assert_eq!(annotations[0].annotation, annotations[1].annotation);
    for (application, expected) in annotations.iter().zip(["record", "entry"]) {
        assert_eq!(
            application.arguments,
            [hir::CanonicalAnnotationValueV1::Scalar(
                hir::CanonicalConstValueV1::String(expected.into())
            )]
        );
    }
    let rule = module
        .annotations
        .declarations
        .iter()
        .find(|declaration| declaration.identity.id() == annotations[0].annotation)
        .unwrap();
    assert!(rule.rules.repeatable);
    assert!(
        !rule
            .rules
            .permits(hir::AnnotationTargetKind::ValueParameter)
    );
    assert!(module.annotations.targets.iter().any(|entry| {
        matches!(entry.target, hir::SourceAnnotationTarget::Field(field) if field.structure() == item)
            && entry.annotations.len() == 1
    }));
}

#[test]
fn m60_doc_annotation_targets_cover_source_api_declarations() {
    let source = scoop_parser::parse(
        r#"
        @DocAlias("record") class C(@DocHidden val x: Int) {
            @DocHidden constructor() : this(1) {}
            @DocAlias("read") fun read(): Int = x
        }
        @DocHidden interface Marker {}
        @DocAlias("singleton") object Service {}
        @DocHidden annotation class Note
        @DocAlias("number") typealias Number = Int
        @DocAlias("data") enum Data { @DocHidden Empty, Value(@DocAlias("payload") val value: Int) }
        @DocHidden val global: Int = 1
        fun main() {}
        "#,
    )
    .unwrap();
    lower(&[core_with_documentation(), source]).unwrap();
}

#[test]
fn m60_doc_annotations_reject_wrong_targets_arguments_and_duplicates() {
    for (source, expected) in [
        (
            "fun f(@DocHidden value: Int) {}",
            "annotation `@DocHidden` is not allowed on ValueParameter",
        ),
        (
            "fun f(value: @DocAlias(\"number\") Int) {}",
            "annotation `@DocAlias` is not allowed on Type",
        ),
        (
            "@DocHidden @DocHidden struct S()",
            "annotation `@DocHidden` must not be repeated on the same target",
        ),
        (
            "@DocAlias(1) struct S()",
            "annotation argument must be String, found Int",
        ),
        (
            "class C { val value: Int @DocHidden get() = 1 }",
            "annotation `@DocHidden` is not allowed on PropertyGetter",
        ),
        (
            "class C(@param:DocHidden val value: Int)",
            "annotation `@DocHidden` is not allowed on ValueParameter",
        ),
    ] {
        let source = scoop_parser::parse(&format!("{source}\nfun main() {{}}")).unwrap();
        let errors = lower(&[core_with_documentation(), source]).unwrap_err();
        assert!(
            errors.iter().any(|error| error.message == expected),
            "{expected}: {errors:?}"
        );
    }
}

#[test]
fn m60_intrinsic_types_and_functions_accept_typed_documentation_aliases() {
    let mut core = core_with_documentation();
    core.imports = scoop_parser::parse("import DocAlias as Search")
        .unwrap()
        .imports;
    for declaration in &mut core.declarations {
        match declaration {
            Decl::Struct(structure) if structure.name.text == "Int" => {
                structure
                    .annotations
                    .extend(applications("@Search(\"integer\")"));
                for member in &mut structure.members {
                    if let ast::StructMember::Function(function) = member
                        && matches!(function.name.text.as_str(), "plus" | "characterUnchecked")
                    {
                        function.annotations.extend(applications("@DocHidden"));
                    }
                }
            }
            Decl::Class(class) if class.name.text == "String" => {
                class
                    .annotations
                    .extend(applications("@DocAlias(\"text\")"));
            }
            Decl::Function(function) if function.name.text == "currentSourceLocation" => {
                function
                    .annotations
                    .extend(applications("@DocAlias(\"location\")"));
            }
            _ => {}
        }
    }
    bootstrap(vec![core]).unwrap();
}

#[test]
fn m60_intrinsic_documentation_does_not_relax_required_effects() {
    let mut core = core_with_documentation();
    let structure = core
        .declarations
        .iter_mut()
        .find_map(|declaration| match declaration {
            Decl::Struct(structure) if structure.name.text == "Int" => Some(structure),
            _ => None,
        })
        .unwrap();
    let function = structure
        .members
        .iter_mut()
        .find_map(|member| match member {
            ast::StructMember::Function(function) if function.name.text == "plus" => Some(function),
            _ => None,
        })
        .unwrap();
    function
        .annotations
        .retain(|annotation| annotation.name.single_text() != Some("NoGC"));
    function
        .annotations
        .extend(applications("@DocAlias(\"add\")"));
    let user = scoop_parser::parse("fun main() {}").unwrap();
    let errors = lower(&[core, user]).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("requires exactly `@NoGC`"))
    );
}

#[test]
fn m60_same_named_noncore_documentation_is_not_an_intrinsic_exception() {
    for name in ["Int", "String", "currentSourceLocation"] {
        let mut core = core_with_documentation();
        let annotation = applications("@shadow.DocAlias(\"not a core annotation\")");
        for declaration in &mut core.declarations {
            match declaration {
                Decl::Struct(structure) if structure.name.text == name => {
                    structure.annotations.extend(annotation.clone());
                }
                Decl::Class(class) if class.name.text == name => {
                    class.annotations.extend(annotation.clone());
                }
                Decl::Function(function) if function.name.text == name => {
                    function.annotations.extend(annotation.clone());
                }
                _ => {}
            }
        }
        let other = scoop_parser::parse(
            "package shadow\npublic annotation class DocAlias(val name: String)",
        )
        .unwrap();
        let errors = bootstrap(vec![core, other]).unwrap_err();
        assert!(
            errors.iter().any(|error| error.message
                == "`@Intrinsic` only permits ordinary core documentation annotations"),
            "{name}: {errors:?}"
        );
    }
}
