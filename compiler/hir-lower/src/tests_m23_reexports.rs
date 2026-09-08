//! `public import` re-export validation tests (DESIGN 2.4): direct-edge
//! authorization, destination binding conflicts and the recorded
//! re-export surface. The transitional single-unit model treats every
//! non-user file as part of the implicitly imported core unit, which is
//! the only direct dependency edge; targets from the current Cone's own
//! user code cannot be copied into a re-export.

use super::*;
use crate::tests::core::core_file;

fn parse(text: &str) -> ast::SourceFile {
    scoop_parser::parse(text).expect("test source parses")
}

/// Lowers `core_file() + library + user` as one Cone; the library file
/// declares its own package and belongs to the implicit core unit.
fn lower_with_library(
    library: &str,
    user: &str,
) -> Result<hir::Module, Vec<scoop_ast::Diagnostic>> {
    lower(&[core_file(), parse(library), parse(user)]).map(|output| output.export)
}

fn user_file_index(module: &hir::Module) -> usize {
    module.semantic_surface.files.len() - 1
}

const LIBRARY: &str = r#"package org.foo

public struct User(val name: String)

public typealias Name = String

public fun render(value: Int): Int {
    return value
}

public fun render(value: String): String {
    return value
}

public fun hello(count: Int): Int {
    return count
}

internal fun assist(): Int {
    return 0
}
"#;

#[test]
fn public_import_records_authorized_reexport() {
    let user = r#"package dev.example

public import org.foo.User
fun main() {}
"#;
    let module = lower_with_library(LIBRARY, user).expect("lowers");
    assert_eq!(module.semantic_surface.reexports.len(), 1);
    let reexport = &module.semantic_surface.reexports[0];
    let user_package = module.semantic_surface.files[user_file_index(&module)].package;
    assert_eq!(reexport.package, user_package);
    assert_eq!(reexport.name, "User");
    assert_eq!(reexport.binding.local_name, "User");
    assert_eq!(reexport.binding.targets.len(), 1);
    assert!(matches!(
        reexport.binding.targets[0].target,
        hir::ImportedTarget::Struct { .. }
    ));
    // The local import binding is unchanged by publication.
    let imports = &module.semantic_surface.files[user_file_index(&module)].imports;
    assert!(imports.exact[0].binding.is_some());
}

#[test]
fn public_import_alias_publishes_under_the_alias() {
    let user = r#"package dev.example

public import org.foo.User as Account
fun main() {}
"#;
    let module = lower_with_library(LIBRARY, user).expect("lowers");
    let reexports = &module.semantic_surface.reexports;
    assert_eq!(reexports.len(), 1);
    assert_eq!(reexports[0].name, "Account");
    assert_eq!(reexports[0].binding.local_name, "Account");
}

#[test]
fn public_import_function_overloads_publish_one_target_per_function() {
    let user = r#"package dev.example

public import org.foo.render
fun main() {}
"#;
    let module = lower_with_library(LIBRARY, user).expect("lowers");
    let reexport = &module.semantic_surface.reexports[0];
    assert_eq!(reexport.name, "render");
    assert_eq!(reexport.binding.targets.len(), 2);
    assert!(
        reexport
            .binding
            .targets
            .iter()
            .all(|target| matches!(target.target, hir::ImportedTarget::Function { .. }))
    );
}

#[test]
fn public_import_of_current_cone_target_is_diagnosed() {
    // The target is declared by the user file itself: current-Cone
    // declarations reach the public surface through their own package's
    // export rules and cannot be copied into a re-export.
    let user = r#"package dev.example

public import dev.example.Widget

public class Widget
fun main() {}
"#;
    let diagnostics = lower_with_library(LIBRARY, user).expect_err("unauthorized re-export");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("public import target `Widget` is not provided by a direct dependency")),
        "{diagnostics:?}"
    );
}

#[test]
fn public_import_of_non_public_target_is_diagnosed() {
    let user = r#"package dev.example

public import org.foo.assist
fun main() {}
"#;
    let diagnostics = lower_with_library(LIBRARY, user).expect_err("internal re-export");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("public import target `assist` is not public and cannot be re-exported")),
        "{diagnostics:?}"
    );
}

#[test]
fn public_import_type_destination_conflicts_with_local_declaration() {
    // `import Option` resolves through the root package to the core
    // enum; the destination name is taken by a same-package class.
    let user = r#"package dev.example

public import Option

public class Option
fun main() {}
"#;
    let diagnostics = lower_with_library(LIBRARY, user).expect_err("type conflict");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(
                "public import re-export `Option` conflicts with a class in package `dev.example`"
            )),
        "{diagnostics:?}"
    );
}

#[test]
fn public_import_function_signature_conflicts_with_local_declaration() {
    // `org.foo.hello(count: Int)` shares a signature with the local
    // `hello`; the re-export of that overload is refused while the rest
    // of the binding still publishes. Only the conflicting overload is
    // diagnosed — the import itself resolved.
    let user = r#"package dev.example

public import org.foo.hello

public fun hello(count: Int): Int {
    return count
}
fun main() {}
"#;
    let diagnostics = lower_with_library(LIBRARY, user).expect_err("signature conflict");
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert!(
        diagnostics[0].message.contains(
            "public import re-export `hello` conflicts with a function with the same signature in package `dev.example`"
        ),
        "{diagnostics:?}"
    );
}

#[test]
fn same_origin_reexports_merge_instead_of_conflicting() {
    // Two public imports of the same entity under the same destination
    // name: the second merges with the first instead of conflicting.
    let user = r#"package dev.example

public import org.foo.User
public import org.foo.User as User
fun main() {}
"#;
    let module = lower_with_library(LIBRARY, user).expect("lowers");
    assert_eq!(module.semantic_surface.reexports.len(), 2);
    assert!(
        module
            .semantic_surface
            .reexports
            .iter()
            .all(|reexport| reexport.name == "User")
    );
}

#[test]
fn different_origin_reexports_conflict_at_the_destination() {
    let libraries = r#"package org.bar

public class User(val id: Int)
"#;
    let user = r#"package dev.example

public import org.foo.User
public import org.bar.User
fun main() {}
"#;
    let diagnostics = lower(&[core_file(), parse(LIBRARY), parse(libraries), parse(user)])
        .map(|output| output.export)
        .expect_err("different-origin re-export conflict");
    // The message names the existing entry's kind (the struct published
    // by the first re-export), mirroring local duplicate diagnostics.
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(
                "public import re-export `User` conflicts with a struct in package `dev.example`"
            )),
        "{diagnostics:?}"
    );
    // The conflicting import is attributed to the importing file.
    let importing_file = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message.contains("conflicts with a struct"))
        .expect("conflict diagnostic");
    assert_eq!(importing_file.file, 3, "{diagnostics:?}");
}

#[test]
fn public_import_into_root_package_merges_with_the_origin_declaration() {
    // A root-package user file re-exporting the core `Option` under its
    // own name: the only same-name entry in the destination package is
    // the target's own origin, which merges instead of conflicting.
    let user = r#"public import Option

fun main() {}
"#;
    let module = lower(&[core_file(), parse(user)])
        .map(|output| output.export)
        .expect("lowers");
    let reexports = &module.semantic_surface.reexports;
    assert_eq!(reexports.len(), 1);
    assert_eq!(reexports[0].name, "Option");
    assert!(matches!(
        reexports[0].binding.targets[0].target,
        hir::ImportedTarget::Enum { .. }
    ));
}

#[test]
fn public_import_typealias_publishes_the_alias_target() {
    let user = r#"package dev.example

public import org.foo.Name
fun main() {}
"#;
    let module = lower_with_library(LIBRARY, user).expect("lowers");
    let reexport = &module.semantic_surface.reexports[0];
    assert_eq!(reexport.name, "Name");
    assert!(matches!(
        reexport.binding.targets[0].target,
        hir::ImportedTarget::TypeAlias { .. }
    ));
}

const COLORS: &str = r#"package colors

public enum Color {
    Red,
    Green(val code: Int)
}
"#;

#[test]
fn star_import_resolves_package_and_enum_surfaces() {
    let user = r#"package dev.example

import org.foo.*
import colors.Color.*
fun main() {}
"#;
    let module = lower(&[core_file(), parse(LIBRARY), parse(COLORS), parse(user)])
        .map(|output| output.export)
        .expect("lowers");
    let imports = &module.semantic_surface.files[3].imports;
    assert!(matches!(
        imports.star[0].binding,
        Some(hir::StarSurface::Package { .. })
    ));
    assert!(matches!(
        imports.star[1].binding,
        Some(hir::StarSurface::EnumOwner { .. })
    ));
}

#[test]
fn star_imported_variants_bind_without_qualification() {
    // Spec 4.2: `import pkg.E.*` admits the enum's variant short names.
    // A payload variant binds in call position, a unit variant in value
    // position.
    let user = r#"package dev.example

import colors.Color.*

fun main() {
    println(describe(Red))
    println(describe(Green(3)))
}

fun describe(color: Color): Int {
    return when (color) {
        Red -> 0
        Green(code) -> code
    }
}
"#;
    lower(&[core_file(), parse(COLORS), parse(user)]).expect("lowers");
}

#[test]
fn unresolved_star_import_is_diagnosed() {
    let user = r#"package dev.example

import org.missing.*
fun main() {}
"#;
    let diagnostics = lower_with_library(LIBRARY, user).expect_err("unresolved star");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("unresolved star import `org.missing.*`")),
        "{diagnostics:?}"
    );
}

#[test]
fn star_import_of_non_enum_nominal_is_diagnosed() {
    let user = r#"package dev.example

import org.foo.User.*
fun main() {}
"#;
    let diagnostics = lower_with_library(LIBRARY, user).expect_err("star of struct");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("star import `org.foo.User.*` must name a package or an enum")),
        "{diagnostics:?}"
    );
}

#[test]
fn local_declarations_shadow_star_imported_variants() {
    // The current-package layer wins over the star variant layer.
    let user = r#"package dev.example

import colors.Color.*

fun main() {
    val value = Green()
    println(value)
}

fun Green(): Int {
    return 7
}
"#;
    lower(&[core_file(), parse(COLORS), parse(user)]).expect("lowers");
}

#[test]
fn public_star_of_package_publishes_per_name() {
    let user = r#"package dev.example

public import org.foo.*
fun main() {}
"#;
    let module = lower_with_library(LIBRARY, user).expect("lowers");
    let user_package = module.semantic_surface.files[user_file_index(&module)].package;
    let names: Vec<&str> = module
        .semantic_surface
        .reexports
        .iter()
        .filter(|reexport| reexport.package == user_package)
        .map(|reexport| reexport.name.as_str())
        .collect();
    // Public members publish per name in name order; the internal
    // `assist` stays out of the snapshot.
    assert_eq!(names, vec!["Name", "User", "hello", "render"]);
    let render = module
        .semantic_surface
        .reexports
        .iter()
        .find(|reexport| reexport.name == "render")
        .expect("render publishes");
    assert_eq!(render.binding.targets.len(), 2, "overloads share a binding");
}

#[test]
fn public_star_of_current_cone_package_is_diagnosed() {
    // The star resolves (the package is declared by this very file) but
    // no member is provided by a direct dependency: one diagnostic for
    // the whole star.
    let user = r#"package dev.example

public import dev.example.*

public fun helper(): Int {
    return 1
}
fun main() {}
"#;
    let diagnostics = lower_with_library(LIBRARY, user).expect_err("empty public star");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic.message.contains(
            "public import `dev.example.*` publishes no target: none is provided by a direct dependency"
        )),
        "{diagnostics:?}"
    );
    assert_eq!(
        diagnostics.len(),
        1,
        "the empty surface is diagnosed once, not per member"
    );
}

#[test]
fn public_star_variant_conflicts_report_per_variant() {
    // `Red` collides with a local function of the destination package;
    // the diagnostic names the variant, and without the collision every
    // variant publishes as a Variant target.
    let user = r#"package dev.example

public import colors.Color.*

fun Red(): Int {
    return 7
}
fun main() {}
"#;
    let diagnostics = lower(&[core_file(), parse(COLORS), parse(user)])
        .map(|output| output.export)
        .expect_err("the conflicting variant is diagnosed");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(
                "public import re-export `Red` conflicts with a function in package `dev.example`"
            )),
        "{diagnostics:?}"
    );
    let clean = r#"package dev.example

public import colors.Color.*
fun main() {}
"#;
    let module = lower(&[core_file(), parse(COLORS), parse(clean)])
        .map(|output| output.export)
        .expect("lowers");
    let user_package = module.semantic_surface.files[user_file_index(&module)].package;
    let names: Vec<&str> = module
        .semantic_surface
        .reexports
        .iter()
        .filter(|reexport| reexport.package == user_package)
        .map(|reexport| reexport.name.as_str())
        .collect();
    // Variant re-exports follow the enum's declaration order.
    assert_eq!(names, vec!["Red", "Green"]);
    assert!(
        module
            .semantic_surface
            .reexports
            .iter()
            .all(|reexport| matches!(
                reexport.binding.targets[0].target,
                hir::ImportedTarget::Variant { .. }
            ))
    );
}

#[test]
fn duplicate_star_paths_deduplicate_variant_candidates() {
    let user = r#"package dev.example

import colors.Color.*
import colors.Color.*

fun main() {
    println(describe(Green(1)))
}

fun describe(color: Color): Int {
    return when (color) {
        Red -> 0
        Green(code) -> code
    }
}
"#;
    lower(&[core_file(), parse(COLORS), parse(user)]).expect("lowers");
}
