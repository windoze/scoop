//! Export surface split tests (DESIGN 4.3).

use super::*;
use crate::tests::core::core_file;

fn parse(text: &str) -> ast::SourceFile {
    scoop_parser::parse(text).expect("test source parses")
}

const LIBRARY: &str = r#"package org.lib

public struct Plain(val value: Int)

public struct Box<T>(val item: T)

public fun <T> identity(value: T): T {
    return value
}

public fun visible(value: Int): Int {
    return value
}

public fun plainOf(value: Plain): Plain {
    return value
}

internal fun hidden(): Int {
    return 0
}

public open class Base public constructor() {
    protected fun shield(): Int {
        return 1
    }

    public fun openFun(): Int {
        return 2
    }
}
"#;

#[test]
fn binding_index_lists_public_declarations_only() {
    let user = "fun main() {}\n";
    let module = lower(&[core_file(), parse(LIBRARY), parse(user)])
        .map(|output| output.export)
        .expect("lowers");
    let surfaces = &module.export_surfaces;
    let org_lib = module
        .semantic_surface
        .packages
        .iter()
        .find(|(_, package)| package.segments == ["org", "lib"])
        .map(|(id, _)| id)
        .expect("org.lib interned");
    let visible = surfaces
        .binding_index
        .entry(org_lib, "visible", hir::BindingNamespace::Value)
        .expect("public function indexed");
    assert_eq!(
        visible.provenance,
        hir::BindingProvenance::PublicDeclaration
    );
    assert!(visible.roots.len() == 1);
    assert!(
        surfaces
            .binding_index
            .entry(org_lib, "hidden", hir::BindingNamespace::Value)
            .is_none(),
        "internal declarations carry no binding entry"
    );
    let plain = surfaces
        .binding_index
        .entry(org_lib, "Plain", hir::BindingNamespace::Type)
        .expect("public struct indexed");
    assert_eq!(plain.provenance, hir::BindingProvenance::PublicDeclaration);
}

#[test]
fn reexport_bindings_publish_with_reexport_provenance() {
    let user = r#"package dev.example

public import org.lib.Plain
fun main() {}
"#;
    let module = lower(&[core_file(), parse(LIBRARY), parse(user)])
        .map(|output| output.export)
        .expect("lowers");
    let dev_example = *module
        .semantic_surface
        .files
        .last()
        .map(|file| &file.package)
        .expect("user file");
    let entry = module
        .export_surfaces
        .binding_index
        .entry(dev_example, "Plain", hir::BindingNamespace::Type)
        .expect("re-export indexed under the destination package");
    assert_eq!(entry.provenance, hir::BindingProvenance::ReExport);
}

#[test]
fn protected_members_reach_inheritance_surface_only() {
    let user = "fun main() {}\n";
    let module = lower(&[core_file(), parse(LIBRARY), parse(user)])
        .map(|output| output.export)
        .expect("lowers");
    let inheritance = &module.export_surfaces.inheritance;
    let base = module
        .classes
        .iter()
        .find(|(_, declaration)| declaration.name == "Base")
        .map(|(id, _)| id)
        .expect("Base declared");
    assert!(inheritance.open_classes.contains(&base));
    assert_eq!(inheritance.protected_methods.len(), 1);
    assert_eq!(
        inheritance.protected_methods[0].0,
        hir::InheritanceHost::Class(base)
    );
    // Protected members have no ordinary binding entry anywhere.
    for entry in &module.export_surfaces.binding_index.entries {
        for root in &entry.roots {
            if let hir::ExportEntity::Function(function) = root {
                assert_ne!(
                    module.functions[*function].name, "shield",
                    "protected members carry no binding entry"
                );
            }
        }
    }
}

#[test]
fn generic_templates_carry_both_purposes() {
    let user = "fun main() {}\n";
    let module = lower(&[core_file(), parse(LIBRARY), parse(user)])
        .map(|output| output.export)
        .expect("lowers");
    let box_id = module
        .structs
        .iter()
        .find(|(_, declaration)| declaration.name == "Box")
        .map(|(id, _)| id)
        .expect("Box declared");
    let template_support = &module.export_surfaces.template_support;
    assert!(template_support.generic_structs.contains(&box_id));
    assert!(
        !template_support
            .generic_structs
            .iter()
            .any(|id| module.structs[*id].name == "Plain"),
        "non-generic templates stay out"
    );
    let purposes = module
        .export_surfaces
        .purposes
        .get(&hir::ExportEntity::Struct(box_id))
        .expect("Box carries purposes");
    assert!(purposes.public_lookup);
    assert!(purposes.template_support);
}

#[test]
fn public_signature_closure_reaches_referenced_nominals() {
    let user = "fun main() {}\n";
    let module = lower(&[core_file(), parse(LIBRARY), parse(user)])
        .map(|output| output.export)
        .expect("lowers");
    let closure = &module.export_surfaces.interface_dependency;
    // `plainOf(value: Plain): Plain` reaches the `Plain` nominal
    // transitively; intrinsic integer types are core relations, not
    // nominal applications, and stay out of the nominal closure.
    assert!(
        closure
            .structs
            .iter()
            .any(|id| module.structs[*id].name == "Plain")
    );
    assert!(
        !closure
            .structs
            .iter()
            .any(|id| module.structs[*id].name == "Int")
    );
}
