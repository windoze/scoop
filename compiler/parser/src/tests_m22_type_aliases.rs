use scoop_ast::{Decl, DeclaredVisibility, Span, TypeAliasDecl, TypeRefKind, VisibilitySyntax};

use crate::parse;
use crate::tests::{err, ok};

fn alias(file: &scoop_ast::SourceFile, index: usize) -> &TypeAliasDecl {
    let Decl::TypeAlias(alias) = &file.declarations[index] else {
        panic!("expected a typealias declaration")
    };
    alias
}

fn explicit_visibility(alias: &TypeAliasDecl) -> DeclaredVisibility {
    let VisibilitySyntax::Explicit { visibility, .. } = alias.visibility else {
        panic!("expected explicit visibility")
    };
    visibility
}

#[test]
fn top_level_typealiases_preserve_visibility_target_and_span() {
    let source = "typealias Count = Int\n\
                  public typealias Names = Array<String>\n\
                  private typealias Coordinates = (Int, Long)\n\
                  internal typealias Callback = (Int, String) -> Unit\n";
    let file = ok(source);
    assert_eq!(file.declarations.len(), 4);

    let count = alias(&file, 0);
    assert!(matches!(count.visibility, VisibilitySyntax::Omitted));
    assert_eq!(count.name.text, "Count");
    assert_eq!(count.name.span, Span::new(10, 15));
    assert_eq!(count.span, Span::new(0, 21));
    assert!(matches!(
        &count.target.kind,
        TypeRefKind::Named(name) if name.text == "Int"
    ));

    let names = alias(&file, 1);
    assert_eq!(explicit_visibility(names), DeclaredVisibility::Public);
    assert!(matches!(
        &names.target.kind,
        TypeRefKind::Generic(name, arguments)
            if name.text == "Array" && arguments.len() == 1
    ));

    let coordinates = alias(&file, 2);
    assert_eq!(
        explicit_visibility(coordinates),
        DeclaredVisibility::Private
    );
    assert!(matches!(
        &coordinates.target.kind,
        TypeRefKind::Tuple(elements) if elements.len() == 2
    ));

    let callback = alias(&file, 3);
    assert_eq!(explicit_visibility(callback), DeclaredVisibility::Internal);
    assert!(matches!(
        &callback.target.kind,
        TypeRefKind::Function(function)
            if !function.is_suspend && function.parameters.len() == 2
    ));
}

#[test]
fn typealias_dump_is_source_shaped() {
    let file = ok("typealias Count = Int\n\
                   public typealias Names = Array<String>\n\
                   private typealias Pair = (Int, String)\n\
                   internal typealias Callback = suspend (Int) -> Unit\n");
    assert_eq!(
        scoop_ast::dump(&file),
        concat!(
            "SourceFile\n",
            "  typealias Count = Int\n",
            "  public typealias Names = Array<String>\n",
            "  private typealias Pair = (Int, String)\n",
            "  internal typealias Callback = suspend (Int) -> Unit\n",
        )
    );
}

#[test]
fn annotations_and_non_visibility_modifiers_are_rejected_on_typealiases() {
    let (_, annotation) = err("@Marker typealias Alias = Int");
    assert_eq!(
        annotation,
        "annotations are not allowed on typealias declarations"
    );

    for modifier in [
        "suspend", "infix", "operator", "const", "open", "final", "abstract", "override",
    ] {
        let source = format!("{modifier} typealias Alias = Int");
        let (_, message) = err(&source);
        assert_eq!(
            message, "only visibility modifiers are allowed on typealias declarations",
            "modifier {modifier}"
        );
    }
}

#[test]
fn generic_typealiases_have_an_m22_diagnostic() {
    let (span, message) = err("typealias Box<T> = Array<T>");
    assert_eq!(span, Span::new(13, 14));
    assert_eq!(
        message,
        "generic typealias declarations are not supported in M22; typealias declarations must be non-generic"
    );
}

#[test]
fn nested_typealiases_have_the_same_explicit_scope_diagnostic() {
    let cases = [
        "class C { typealias Alias = Int }",
        "struct S { typealias Alias = Int }",
        "interface I { typealias Alias = Int }",
        "enum E { A\n typealias Alias = Int }",
        "object O { typealias Alias = Int }",
    ];
    for source in cases {
        let (_, message) = err(source);
        assert_eq!(
            message,
            "nested typealias declarations are not supported in M22; only top-level non-generic typealias declarations are supported",
            "source {source}"
        );
    }
}

#[test]
fn local_typealiases_have_an_m22_diagnostic() {
    let (_, message) = err("fun f() { typealias Local = Int }");
    assert_eq!(
        message,
        "local typealias declarations are not supported in M22; only top-level non-generic typealias declarations are supported"
    );
}

#[test]
fn malformed_typealias_syntax_has_focused_diagnostics() {
    let (_, missing_name) = err("typealias = Int");
    assert_eq!(missing_name, "expected typealias name, found `=`");

    let (_, missing_equal) = err("typealias Alias Int");
    assert_eq!(
        missing_equal,
        "expected `=` in typealias declaration, found `Int`"
    );

    let (_, missing_target) = err("typealias Alias =");
    assert_eq!(missing_target, "expected type, found end of file");
}

#[test]
fn typealias_errors_recover_at_declaration_and_member_boundaries() {
    let diagnostics = parse(
        "@Marker typealias Annotated = Int\n\
         public typealias Generic<T> = T\n\
         open typealias Modified = Int\n\
         private typealias AlsoGeneric<T> = T\n\
         fun retained() {}\n\
         class C {\n\
             typealias Nested = Int\n\
             fun alsoRetained() {}\n\
         }\n\
         typealias Retained = String\n",
    )
    .expect_err("unsupported alias forms must be diagnosed");
    assert_eq!(diagnostics.len(), 5, "{diagnostics:?}");
    assert_eq!(
        diagnostics[0].message,
        "annotations are not allowed on typealias declarations"
    );
    assert_eq!(
        diagnostics[1].message,
        "generic typealias declarations are not supported in M22; typealias declarations must be non-generic"
    );
    assert_eq!(
        diagnostics[2].message,
        "only visibility modifiers are allowed on typealias declarations"
    );
    assert_eq!(
        diagnostics[3].message,
        "generic typealias declarations are not supported in M22; typealias declarations must be non-generic"
    );
    assert_eq!(
        diagnostics[4].message,
        "nested typealias declarations are not supported in M22; only top-level non-generic typealias declarations are supported"
    );
}
