use scoop_ast::{
    CallArgumentName, Decl, Expr, ParameterSyntax, SpreadSyntax, StatementKind,
    VarargDefaultSyntax, VariantDeclKind,
};

use crate::parse;
use crate::tests::{block_body, err, ok, only_function};

#[test]
fn parses_required_default_and_vararg_parameters() {
    let file = ok("fun collect(first: Int, rest: Int = 1, vararg tail: Int = [2]): Unit {}");
    let function = only_function(&file);
    assert!(matches!(
        function.params[0].syntax,
        ParameterSyntax::Required
    ));
    assert!(matches!(
        function.params[1].syntax,
        ParameterSyntax::Default { .. }
    ));
    assert!(matches!(
        function.params[2].syntax,
        ParameterSyntax::Vararg {
            default: VarargDefaultSyntax::Expression { .. },
            ..
        }
    ));
    assert_eq!(
        scoop_ast::dump(&file),
        "SourceFile\n  RootPackage\n  fun collect(first: Int, rest: Int = <expr>, vararg tail: Int = <expr>): Unit\n"
    );
}

#[test]
fn constructor_parameters_share_the_parameter_syntax() {
    let file = ok("struct S(val x: Int = 1, vararg val ys: Int)\n\
         class C(var name: String = \"c\", vararg val values: Int)\n\
         enum E { V(val x: Int = helper(), vararg val ys: Int) }");
    let Decl::Struct(structure) = &file.declarations[0] else {
        panic!("expected struct")
    };
    assert!(matches!(
        structure.fields[0].syntax,
        ParameterSyntax::Default { .. }
    ));
    assert!(matches!(
        structure.fields[1].syntax,
        ParameterSyntax::Vararg {
            default: VarargDefaultSyntax::EmptyWhenOmitted,
            ..
        }
    ));
    let Decl::Class(class) = &file.declarations[1] else {
        panic!("expected class")
    };
    assert!(matches!(
        class.constructor[0].syntax,
        ParameterSyntax::Default { .. }
    ));
    let Decl::Enum(enumeration) = &file.declarations[2] else {
        panic!("expected enum")
    };
    let VariantDeclKind::Constructor(fields) = &enumeration.variants[0].kind else {
        panic!("expected constructor variant")
    };
    assert!(matches!(
        fields[0].syntax,
        ParameterSyntax::Default {
            expression: Expr::Call(_),
            ..
        }
    ));
    assert!(matches!(fields[1].syntax, ParameterSyntax::Vararg { .. }));
}

#[test]
fn parses_named_and_spread_arguments_in_source_order() {
    let file = ok("fun main() {\n\
             emit(1, suffix = end(), *middle, values = *whole) { 0 }\n\
         }");
    let StatementKind::Expr(Expr::Call(call)) =
        &block_body(only_function(&file)).statements[0].kind
    else {
        panic!("expected call")
    };
    assert_eq!(call.args.len(), 5);
    assert!(matches!(call.args[0].name, CallArgumentName::Positional));
    assert!(matches!(call.args[0].spread, SpreadSyntax::Plain));
    assert!(matches!(&call.args[1].name, CallArgumentName::Named(name) if name.text == "suffix"));
    assert!(matches!(call.args[2].spread, SpreadSyntax::Spread(_)));
    assert!(matches!(&call.args[3].name, CallArgumentName::Named(name) if name.text == "values"));
    assert!(matches!(call.args[3].spread, SpreadSyntax::Spread(_)));
    assert!(matches!(call.args[4].expression, Expr::Lambda { .. }));
    assert_eq!(
        scoop_ast::dump(&file),
        "SourceFile\n  RootPackage\n  fun main()\n    Call emit\n      IntLiteral 1\n      Argument suffix=\n        Call end\n      Argument *\n        Var middle\n      Argument values=*\n        Var whole\n      Lambda 0 suspend=false\n        parameters omitted\n        IntLiteral 0\n"
    );
}

#[test]
fn member_invoke_and_base_constructor_keep_call_arguments() {
    let file = ok("class Base(val x: Int)\n\
         class Derived(val y: Int) : Base(x = y)\n\
         fun main() {\n\
             target.send(value = *items)\n\
             (factory())(first = 1)\n\
         }");
    let Decl::Class(derived) = &file.declarations[1] else {
        panic!("expected derived class")
    };
    let base_args = derived.supertypes[0]
        .constructor_arguments
        .as_ref()
        .expect("base constructor");
    assert!(matches!(&base_args[0].name, CallArgumentName::Named(name) if name.text == "x"));
    let Decl::Function(main) = &file.declarations[2] else {
        panic!("expected main")
    };
    let StatementKind::Expr(Expr::MethodCall { args, .. }) = &block_body(main).statements[0].kind
    else {
        panic!("expected member call")
    };
    assert!(matches!(args[0].spread, SpreadSyntax::Spread(_)));
    let StatementKind::Expr(Expr::Invoke { args, .. }) = &block_body(main).statements[1].kind
    else {
        panic!("expected invoke")
    };
    assert!(matches!(&args[0].name, CallArgumentName::Named(name) if name.text == "first"));
}

#[test]
fn diagnoses_duplicate_and_multiple_vararg_modifiers() {
    let (_, message) = err("fun f(vararg vararg x: Int) {}");
    assert_eq!(message, "duplicate `vararg` modifier on parameter");

    let (_, message) = err("fun f(vararg x: Int, vararg y: Int) {}");
    assert_eq!(
        message,
        "a parameter list may declare at most one `vararg` parameter"
    );
}

#[test]
fn malformed_default_and_spread_recover_to_later_declarations() {
    let diagnostics = parse(
        "fun bad(x: Int = ) {}\n\
         fun alsoBad() { emit(*) }\n\
         fun good() {}",
    )
    .expect_err("two malformed expressions must be reported");
    let messages = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        messages,
        [
            "expected expression, found `)`",
            "expected expression, found `)`"
        ]
    );
}
