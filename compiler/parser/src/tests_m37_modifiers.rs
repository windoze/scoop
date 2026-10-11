//! Compatibility markers are discarded in their legal grammar positions.

use scoop_ast::{Decl, MethodModifier, Span};

use crate::tests::{err, ok};

#[test]
fn repeated_markers_keep_ordinary_function_semantics() {
    let file = ok(
        "inline tailrec inline fun <reified reified T> pick(noinline crossinline value: T): T = value",
    );
    let Decl::Function(function) = &file.declarations[0] else {
        panic!("expected function");
    };
    assert_eq!(function.type_params[0].name.text, "T");
    assert_eq!(function.params[0].name.text, "value");
    assert_eq!(function.modifier, MethodModifier::Final);
    assert!(!function.is_suspend);
}

#[test]
fn parameter_markers_mix_with_vararg_and_constructor_properties() {
    ok("class Box<reified T>(noinline val value: T) {\n\
           constructor(crossinline count: Int, noinline value: T): this(value) {}\n\
         }\n\
         struct Pair(noinline val first: Int, crossinline val second: Int)\n\
         enum Choice { Some(noinline val value: Int), None\n inline fun f(): Int = 1 }\n\
         enum Number { One\n inline val value: Int get() = 1 }\n\
         annotation class Label(noinline val value: String)\n\
         fun f(noinline vararg crossinline values: Int) {}\n\
         fun g() { val f = fun(noinline value: Int): Int { return value } }");
}

#[test]
fn property_accessor_and_local_function_markers_are_accepted() {
    ok("inline val answer: Int inline get() = 42\n\
         class Counter {\n\
           inline var value: Int = 0\n\
             inline get() = field\n\
             inline private inline set(noinline value) { field = value }\n\
         }\n\
         fun main() {\n\
           suspend inline tailrec fun local(): Int = 42\n\
           inline tailrec fun other(): Int = 42\n\
         }");
}

#[test]
fn contextual_markers_remain_available_as_names() {
    ok("enum Keywords { inline\n tailrec\n Other }\n\
         fun <reified> inline(noinline: reified, crossinline: Int): reified = noinline\n\
         fun tailrec(): Int = 1\n\
         fun main() {\n\
           val inline = 42\n\
           tailrec()\n\
           println(inline)\n\
         }");
}

#[test]
fn invalid_targets_and_real_duplicate_modifiers_still_report_spans() {
    for (source, keyword, message) in [
        (
            "inline class C",
            "inline",
            "invalid modifier on class declaration",
        ),
        (
            "tailrec val x: Int = 1",
            "tailrec",
            "`tailrec` is only allowed on functions",
        ),
        (
            "fun f(vararg noinline vararg xs: Int) {}",
            "vararg",
            "duplicate `vararg` modifier on parameter",
        ),
    ] {
        let (span, actual) = err(source);
        let start = source.rfind(keyword).unwrap() as u32;
        assert_eq!(span, Span::new(start, start + keyword.len() as u32));
        assert_eq!(actual, message);
    }
}
