//! Import binding resolution tests (DESIGN sections 2.2-2.3): package
//! prefixes, aliases, visibility, nested owner walks, overload sets and
//! the longest-package-prefix rule.

use super::*;
use crate::tests::core::core_file;

fn parse(text: &str) -> ast::SourceFile {
    scoop_parser::parse(text).expect("test source parses")
}

/// Lowers `core_file() + library + user` as one Cone; the library file
/// declares its own package.
fn lower_with_library(
    library: &str,
    user: &str,
) -> Result<hir::Module, Vec<scoop_ast::Diagnostic>> {
    lower(&[core_file(), parse(library), parse(user)]).map(|output| output.export)
}

fn user_imports(module: &hir::Module) -> &hir::FileImports {
    let file = module.semantic_surface.files.len() - 1;
    &module.semantic_surface.files[file].imports
}

const LIBRARY: &str = r#"package org.foo

public struct User(val name: String)

public class Outer {
    public class Nested
}

public fun render(value: Int): Int {
    return value
}

public fun render(value: String): String {
    return value
}

private struct Secret(val value: Int)
"#;

#[test]
fn cross_package_exact_import_resolves_with_alias() {
    let user = r#"package dev.example

import org.foo.User as U
fun main() {}
"#;
    let module = lower_with_library(LIBRARY, user).expect("lowers");
    let imports = user_imports(&module);
    assert_eq!(imports.exact.len(), 1);
    let binding = imports.exact[0].binding.as_ref().expect("resolved");
    assert_eq!(binding.local_name, "U");
    assert_eq!(binding.targets.len(), 1);
    assert!(matches!(
        binding.targets[0].target,
        hir::ImportedTarget::Struct { .. }
    ));
    assert!(matches!(
        binding.targets[0].sources[0],
        hir::ImportBindingSource::CurrentCone { .. }
    ));
}

#[test]
fn unresolved_import_is_diagnosed() {
    let user = r#"package dev.example

import org.missing.Thing
fun main() {}
"#;
    let diagnostics = lower_with_library(LIBRARY, user).expect_err("unresolved");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("unresolved import `org.missing.Thing`")),
        "{diagnostics:?}"
    );
}

#[test]
fn private_declarations_are_invisible_to_other_files() {
    let user = r#"package dev.example

import org.foo.Secret
fun main() {}
"#;
    let diagnostics = lower_with_library(LIBRARY, user).expect_err("invisible");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("not visible in this file")),
        "{diagnostics:?}"
    );
}

#[test]
fn private_declarations_are_importable_from_the_declaring_file() {
    // The library file itself imports its own private declaration: the
    // root-package hit for `Secret` stays inside the declaring file.
    let library = r#"package org.foo

import org.foo.Secret

private struct Secret(val value: Int)
"#;
    let user = "fun main() {}\n";
    let module = lower(&[core_file(), parse(library), parse(user)])
        .map(|output| output.export)
        .expect("lowers");
    let imports = &module.semantic_surface.files[1].imports;
    let binding = imports.exact[0].binding.as_ref().expect("resolved");
    assert!(matches!(
        binding.targets[0].target,
        hir::ImportedTarget::Struct { .. }
    ));
}

#[test]
fn nested_nominal_walks_the_owner_chain() {
    let user = r#"package dev.example

import org.foo.Outer.Nested
fun main() {}
"#;
    let module = lower_with_library(LIBRARY, user).expect("lowers");
    let imports = user_imports(&module);
    let binding = imports.exact[0].binding.as_ref().expect("resolved");
    assert_eq!(binding.local_name, "Nested");
    assert!(matches!(
        binding.targets[0].target,
        hir::ImportedTarget::Class { .. }
    ));
}

#[test]
fn function_overloads_bind_one_target_per_function() {
    let user = r#"package dev.example

import org.foo.render
fun main() {}
"#;
    let module = lower_with_library(LIBRARY, user).expect("lowers");
    let imports = user_imports(&module);
    let binding = imports.exact[0].binding.as_ref().expect("resolved");
    assert_eq!(binding.local_name, "render");
    assert_eq!(binding.targets.len(), 2, "one target per overload");
    assert!(
        binding
            .targets
            .iter()
            .all(|target| matches!(target.target, hir::ImportedTarget::Function { .. }))
    );
    // The two overloads are distinct entities.
    assert_ne!(binding.targets[0].target, binding.targets[1].target);
}

#[test]
fn longest_declared_package_prefix_wins() {
    // `org.foo.model` exists both as a declared package (with a
    // top-level struct User) and — through a shorter reading — as a
    // class `model` nested in package org.foo... the library declares
    // the longer package, so the import binds the package's struct.
    let library = r#"package org.foo.model

public struct User(val name: String)
"#;
    let user = r#"package dev.example

import org.foo.model.User
fun main() {}
"#;
    let module = lower_with_library(library, user).expect("lowers");
    let imports = user_imports(&module);
    let binding = imports.exact[0].binding.as_ref().expect("resolved");
    assert!(matches!(
        binding.targets[0].target,
        hir::ImportedTarget::Struct { .. }
    ));
}

#[test]
fn same_cone_name_in_function_and_type_namespaces_binds_both() {
    let library = r#"package org.foo

public struct Widget(val size: Int)

public fun Widget(): Widget {
    return Widget(1)
}
"#;
    let user = r#"package dev.example

import org.foo.Widget
fun main() {}
"#;
    let module = lower_with_library(library, user).expect("lowers");
    let imports = user_imports(&module);
    let binding = imports.exact[0].binding.as_ref().expect("resolved");
    assert_eq!(binding.targets.len(), 2, "one target per namespace");
    let has_struct = binding
        .targets
        .iter()
        .any(|target| matches!(target.target, hir::ImportedTarget::Struct { .. }));
    let has_function = binding
        .targets
        .iter()
        .any(|target| matches!(target.target, hir::ImportedTarget::Function { .. }));
    assert!(has_struct && has_function);
}
