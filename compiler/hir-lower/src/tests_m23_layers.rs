//! Lookup-layer tests (DESIGN 2.3): exact imports shadow the current
//! package, which shadows star imports, which shadow the implicitly
//! imported core surface. The transitional single-unit model treats
//! every non-user file as part of the implicitly imported core unit, so
//! cross-package isolation between library files is exercised through
//! package declarations inside that unit.

use super::*;
use crate::tests::core::core_file;

fn parse(text: &str) -> ast::SourceFile {
    scoop_parser::parse(text).expect("test source parses")
}

fn lower_unit(sources: &[ast::SourceFile]) -> Result<hir::Module, Vec<scoop_ast::Diagnostic>> {
    let mut files = vec![core_file()];
    files.extend(sources.iter().cloned());
    lower(&files).map(|output| output.export)
}

const FOO_LIBRARY: &str = r#"package org.foo

public fun ping(): Int {
    return 1
}

public fun hello(count: Int): Int {
    return count
}
"#;

const BAR_LIBRARY: &str = r#"package org.bar

public fun ping(): Int {
    return 2
}

public fun hello(): String {
    return "bar"
}
"#;

#[test]
fn current_package_beats_core_prelude_for_same_name() {
    // `caller` lives in org.foo; both org.foo and org.bar declare
    // `hello`, and both are inside the implicitly imported unit. The
    // current-package layer sees only org.foo's overload.
    let library = r#"package org.foo

public fun caller(): Int {
    return hello(2)
}
"#;
    let module = lower_unit(&[
        parse(FOO_LIBRARY),
        parse(BAR_LIBRARY),
        parse(library),
        parse("fun main() {}"),
    ])
    .expect("lowers");
    let caller = module
        .functions
        .iter()
        .find(|(_, function)| function.name == "caller")
        .map(|(id, _)| id)
        .expect("caller exists");
    // The call resolved to a single Int-returning hello without an
    // ambiguity diagnostic; the lowering succeeded and the winner's
    // body type-checks against Int.
    let _ = caller;
}

#[test]
fn same_name_across_packages_without_import_is_ambiguous() {
    // A user file (root package) sees both hellos only through the
    // implicit core layer — same layer, different shapes.
    let user = r#"fun main() {
    val value = ping()
}
"#;
    let diagnostics =
        lower_unit(&[parse(FOO_LIBRARY), parse(BAR_LIBRARY), parse(user)]).expect_err("ambiguous");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("call to `ping` is ambiguous")),
        "{diagnostics:?}"
    );
}

#[test]
fn exact_import_selects_one_package() {
    let user = r#"import org.bar.hello

fun main() {
    val greeting: String = hello()
}
"#;
    let module =
        lower_unit(&[parse(FOO_LIBRARY), parse(BAR_LIBRARY), parse(user)]).expect("lowers");
    let main = module.functions[module.entry].clone();
    let _ = main;
}

#[test]
fn star_import_selects_a_package_surface() {
    let user = r#"import org.bar.*

fun main() {
    val greeting: String = hello()
}
"#;
    let module =
        lower_unit(&[parse(FOO_LIBRARY), parse(BAR_LIBRARY), parse(user)]).expect("lowers");
    let _ = module;
}

#[test]
fn star_import_does_not_shadow_exact_import_layer() {
    // Exact imports sit ABOVE star imports: importing org.bar.* and
    // org.foo.hello by exact path selects org.foo's Int overload.
    let user = r#"import org.bar.*
import org.foo.hello

fun main() {
    val greeting: Int = hello(3)
}
"#;
    let module =
        lower_unit(&[parse(FOO_LIBRARY), parse(BAR_LIBRARY), parse(user)]).expect("lowers");
    let _ = module;
}

#[test]
fn core_prelude_still_reaches_other_package_files() {
    // A user file in a non-root package still sees the core unit's
    // declarations (here `println`) through the implicit core layer.
    let user = r#"package dev.example

fun main() {
    println("hello")
}
"#;
    let module = lower_unit(&[parse(user)]).expect("lowers");
    let _ = module;
}

#[test]
fn current_package_shadows_core_for_user_files() {
    // The user file's own package declarations win over the implicit
    // core unit's same-name declarations.
    let user = r#"package dev.example

fun describe(): String {
    return "user"
}

fun main() {
    val text: String = describe()
}
"#;
    // Core unit also declares `describe` returning Int.
    let library = r#"package dev.example

public fun describe(count: Int): Int {
    return count
}
"#;
    let module = lower_unit(&[parse(library), parse(user)]).expect("lowers");
    let main = module.functions[module.entry].clone();
    let _ = main;
}

const TYPE_FOO: &str = r#"package org.shape

public struct Point(val x: Int, val y: Int)
"#;

const TYPE_BAR: &str = r#"package org.metric

public class Point public constructor(public val magnitude: Long)
"#;

#[test]
fn same_name_types_in_distinct_packages_lower() {
    // Per-package type namespaces: both Points coexist in one Cone.
    let library = r#"package org.shape

public fun origin(): Point {
    return Point(0, 0)
}
"#;
    let user = r#"package org.metric

fun main() {
    val p = Point(3)
    println(p.magnitude)
}
"#;
    let module = lower_unit(&[
        parse(TYPE_FOO),
        parse(TYPE_BAR),
        parse(library),
        parse(user),
    ])
    .expect("lowers");
    let _ = module;
}

#[test]
fn type_resolution_is_layered_for_imports() {
    // The user file (root package) imports org.shape.Point exactly;
    // `Point` must resolve to the struct, not the core-unit class of
    // the same name (both are otherwise reachable through the implicit
    // core layer).
    let user = r#"import org.shape.Point

fun main() {
    val origin: Point = Point(0, 0)
    println(origin.x)
}
"#;
    let module = lower_unit(&[parse(TYPE_FOO), parse(TYPE_BAR), parse(user)]).expect("lowers");
    let _ = module;
}

#[test]
fn same_layer_type_ambiguity_is_diagnosed() {
    // Without imports both Points sit in the implicit core layer.
    let user = r#"fun main() {
    val origin = Point(0, 0)
}
"#;
    let diagnostics =
        lower_unit(&[parse(TYPE_FOO), parse(TYPE_BAR), parse(user)]).expect_err("ambiguous type");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("`Point` is ambiguous")),
        "{diagnostics:?}"
    );
}

#[test]
fn star_import_binds_package_type_surface() {
    let user = r#"import org.metric.*

fun main() {
    val p = Point(3)
    println(p.magnitude)
}
"#;
    let module = lower_unit(&[parse(TYPE_FOO), parse(TYPE_BAR), parse(user)]).expect("lowers");
    let _ = module;
}
