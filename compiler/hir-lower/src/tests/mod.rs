//! Test builders (M4 AST) and the M1 test suite, adapted to the M4
//! AST/HIR contracts and the multi-file `lower` entry point.
//!
//! Every test compiles the user file together with a minimal
//! `scoop.core` (`core_file()`: the `Option<T>` enum, the `Throwable`
//! exception root, the M10 coroutine and M22 iteration protocols, plus
//! the M7 `io.scoop` overloads and their backing intrinsic), mirroring
//! the driver's sysroot convention — core files first, the user file last.

mod basics;
mod builders;
mod core;
mod m10;
mod m11;
mod m12;
mod m14;
mod m17;
mod m18;
mod m19;
mod m19_materialization;
mod m19_safety;
mod m2;
mod m20;
mod m21_companions;
mod m21_consts;
mod m21_delegates;
mod m21_interfaces;
mod m21_nested;
mod m21_objects;
mod m21_properties;
mod m21_top_level_storage;
mod m21_visibility;
mod m22_aliases;
mod m22_binding_patterns;
mod m22_contextual_inference;
mod m22_contextual_variants;
mod m22_copy_updates;
mod m22_integer_diagnostics;
mod m22_integer_exhaustiveness;
mod m22_integers;
mod m22_iteration;
mod m22_joint_integer_inference;
mod m22_loop_targets;
mod m22_pattern_warnings;
mod m22_ranges;
mod m22_recursive_named_fields;
mod m23_address_of_globals;
mod m23_any_dependency_calls;
mod m23_c_projection;
mod m23_callable_applications;
mod m23_callable_interface_production;
mod m23_callable_reference_identities;
mod m23_callable_references;
mod m23_callable_source_interface_production;
mod m23_const_interface_production;
mod m23_core_bootstrap;
mod m23_default_template_production;
mod m23_definition_origins;
mod m23_definition_source_production;
mod m23_delegate_operator_layers;
mod m23_enum_member_identities;
mod m23_exact_types;
mod m23_explicit_receiver_calls;
mod m23_expression_qualifiers;
mod m23_extension_properties;
mod m23_generic_body_consumption;
mod m23_hir_foundation;
mod m23_intrinsic_source_shapes;
mod m23_local_value_identities;
mod m23_local_value_selectors;
mod m23_named_calls;
mod m23_nominal_interface_production;
mod m23_ordinary_core_only;
mod m23_ordinary_dependencies;
mod m23_ordinary_dependency_constants;
mod m23_ordinary_dependency_defaults;
mod m23_ordinary_dependency_properties;
mod m23_ordinary_dependency_type_aliases;
mod m23_output_kind;
mod m23_pattern_paths;
mod m23_property_interface_production;
mod m23_source_call_receivers;
mod m23_source_model;
mod m23_type_alias_interface_production;
mod m23_type_alias_targets;
mod m23_type_semantics_production;
mod m24_release_blocks;
mod m24_release_cfg;
mod m24_release_effects;
mod m24_release_generics;
mod m27_context;
mod m29_annotations;
mod m29_class_shapes;
mod m29_companions;
mod m29_container_encoding;
mod m29_derived_decoding;
mod m29_derived_encoding;
mod m29_static_shapes;
mod m29_unit_encoding;
mod m3;
mod m4;
mod m5;
mod m6;
mod m7;
mod m8;
mod m9;

use super::*;
use ast::{
    BinOp, Block, CallExpr, Decl, Expr, FieldAccess, FieldDecl, FieldSelector, FunctionBody,
    FunctionDecl, Ident, Param, SourceFile, Statement, StatementKind, StructDecl as AstStructDecl,
    TypeRef, TypeRefKind, UnOp, ValDecl, VariantDecl, VariantDeclKind, VariantFieldDecl,
};

pub(crate) use builders::*;
pub(crate) use core::*;

pub(crate) fn defined_export_core(module: &hir::Module) -> &hir::DefinedCoreProtocols {
    let hir::CoreProtocols::Defined(protocols) = &module.core_protocols else {
        panic!("test Export HIR carries locally defined core protocols")
    };
    protocols
}

pub(crate) fn defined_concrete_core(
    module: &hir::concrete::Module,
) -> &hir::concrete::DefinedConcreteCoreProtocols {
    let hir::concrete::ConcreteCoreProtocols::Defined(protocols) = &module.core_protocols else {
        panic!("test LocalConcrete HIR carries locally defined core protocols")
    };
    protocols
}

trait TestExecutableEntry {
    type FunctionId: Copy;

    fn entry(&self) -> Self::FunctionId;
}

impl TestExecutableEntry for hir::ExportHirOutput {
    type FunctionId = hir::FunctionId;

    fn entry(&self) -> Self::FunctionId {
        let hir::ConeOutputKind::Executable { local_entry } = self.output_kind() else {
            panic!("test expected executable Export HIR")
        };
        local_entry.local_function().function()
    }
}

impl TestExecutableEntry for hir::LocalConcreteHirOutput {
    type FunctionId = hir::concrete::FunctionId;

    fn entry(&self) -> Self::FunctionId {
        let hir::LocalConeOutputKind::Executable { local_entry } = self.output_kind() else {
            panic!("test expected executable LocalConcrete HIR")
        };
        local_entry.local_function().function()
    }
}

fn lower(files: &[ast::SourceFile]) -> Result<hir::Output, Vec<ast::Diagnostic>> {
    const CORE_PATHS: [&str; 4] = [
        "src/core.scoop",
        "src/core-extra.scoop",
        "src/core-third.scoop",
        "src/core-fourth.scoop",
    ];
    let (user, core) = files
        .split_last()
        .expect("HIR lowering tests always supply a user source");
    let core = core
        .iter()
        .enumerate()
        .map(|(index, source)| ProviderSource {
            source,
            identity: core_source_identity(
                &CORE_PATHS
                    .get(index)
                    .map(|path| (*path).to_owned())
                    .unwrap_or_else(|| format!("src/core-{index}.scoop")),
            ),
            provider: hir::IntrinsicProviderId::from_raw(0),
            name: "<core>",
            source_text: "",
        })
        .collect();
    let parsed = ast::AllParsedSources::try_new(ast::NonEmptyVec::new(
        ast::IdentifiedParsedSource::new(
            scoop_identity::SourceIdentity::single_file(),
            user.clone(),
        ),
        Vec::new(),
    ))
    .expect("the single-file test input has one source identity");
    let input =
        DefinedTestSources::try_new(core, hir::IntrinsicProviderId::from_raw(1), parsed, |_| {
            CurrentSourceDetails {
                display_locator: "<user>",
                source_text: "",
            }
        })
        .expect("explicit test source identities are valid");
    lower_defined_for_test(
        scoop_identity::RequestedConeKind::Executable,
        &input,
        IntrinsicDeclarationPolicy::CoreOnly,
    )
}

pub(crate) fn test_source_identity(path: &str) -> scoop_identity::SourceIdentity {
    scoop_identity::SourceIdentity::new(
        scoop_identity::ConeCoordinate::new("test", "scoop-hir-lower", "0.0.0")
            .unwrap()
            .identity()
            .unwrap(),
        scoop_identity::NormalizedSourcePath::new(path).unwrap(),
    )
    .unwrap()
}

pub(crate) fn core_source_identity(path: &str) -> scoop_identity::SourceIdentity {
    scoop_identity::SourceIdentity::new(
        scoop_identity::ConeIdentity::CORE,
        scoop_identity::NormalizedSourcePath::new(path).unwrap(),
    )
    .unwrap()
}

pub(crate) fn identified_test_sources(sources: Vec<ast::SourceFile>) -> ast::AllParsedSources {
    const PATHS: [&str; 8] = [
        "src/first.scoop",
        "src/second.scoop",
        "src/third.scoop",
        "src/fourth.scoop",
        "src/fifth.scoop",
        "src/sixth.scoop",
        "src/seventh.scoop",
        "src/eighth.scoop",
    ];
    assert!(
        sources.len() <= PATHS.len(),
        "add explicit test source paths"
    );
    let mut sources = sources
        .into_iter()
        .zip(PATHS)
        .map(|(source, path)| ast::IdentifiedParsedSource::new(test_source_identity(path), source));
    ast::AllParsedSources::try_new(ast::NonEmptyVec::new(
        sources.next().expect("test source list is nonempty"),
        sources.collect(),
    ))
    .expect("explicit test source identities are unique")
}

pub(crate) fn lower_test_sources(
    core: Vec<ProviderSource<'_>>,
    user: &ast::SourceFile,
    user_provider: hir::IntrinsicProviderId,
    user_display_locator: &str,
    user_source_text: &str,
    policy: IntrinsicDeclarationPolicy,
) -> Result<hir::Output, Vec<ast::Diagnostic>> {
    let parsed = ast::AllParsedSources::try_new(ast::NonEmptyVec::new(
        ast::IdentifiedParsedSource::new(test_source_identity("src/user.scoop"), user.clone()),
        Vec::new(),
    ))
    .expect("the test supplies one explicit user source identity");
    let input =
        DefinedTestSources::try_new(core, user_provider, parsed, |_| CurrentSourceDetails {
            display_locator: user_display_locator,
            source_text: user_source_text,
        })
        .expect("explicit test source identities are valid");
    lower_defined_for_test(
        scoop_identity::RequestedConeKind::Executable,
        &input,
        policy,
    )
}

fn integer_syntax(magnitude: u64) -> ast::IntegerLiteralSyntax {
    ast::IntegerLiteralSyntax {
        magnitude,
        radix: ast::IntegerRadix::Decimal,
        suffix: ast::IntegerSuffix::None,
        span: sp(),
    }
}

fn integer_type(module: &hir::Module, kind: hir::IntegerKind) -> hir::TypeId {
    module
        .types
        .iter()
        .find_map(|(id, ty)| (*ty == hir::Type::Integer(kind)).then_some(id))
        .expect("lowered module contains every canonical integer type")
}

fn int_type(module: &hir::Module) -> hir::TypeId {
    integer_type(module, hir::IntegerKind::SIGNED_32)
}

fn concrete_integer_type(
    module: &hir::concrete::Module,
    kind: hir::IntegerKind,
) -> hir::concrete::TypeId {
    module
        .types
        .iter()
        .find_map(|(id, ty)| (ty.kind == hir::concrete::TypeKind::Integer(kind)).then_some(id))
        .expect("concrete module contains every canonical integer type")
}

fn concrete_int_type(module: &hir::concrete::Module) -> hir::concrete::TypeId {
    concrete_integer_type(module, hir::IntegerKind::SIGNED_32)
}

fn definition_path(
    path: &scoop_identity::StructuralDefinitionPath,
) -> Vec<(scoop_identity::StructuralDefinitionSiteRole, u32)> {
    path.segments()
        .iter()
        .map(|segment| (segment.site_role(), segment.ordinal()))
        .collect()
}

fn binding_local(pattern: &hir::Pattern) -> Option<hir::LocalId> {
    match pattern {
        hir::Pattern::Binding { local } => Some(*local),
        _ => None,
    }
}

fn local_init<'body>(body: &'body hir::Body, name: &str) -> &'body hir::Expr {
    body.statements
        .iter()
        .find_map(|statement| {
            let hir::StatementKind::ValDecl { pattern, init } = &statement.kind else {
                return None;
            };
            let local = binding_local(pattern)?;
            (body.locals[local].name == name).then_some(init)
        })
        .unwrap_or_else(|| panic!("local `{name}` must have an initializer"))
}

fn expression_statement(body: &hir::Body, index: usize) -> &hir::Expr {
    body.statements
        .iter()
        .filter_map(|statement| match &statement.kind {
            hir::StatementKind::Expr(expr) => Some(expr),
            _ => None,
        })
        .nth(index)
        .unwrap_or_else(|| panic!("source expression statement {index} must exist"))
}

fn return_value(statements: &[hir::Statement]) -> &hir::Expr {
    statements
        .iter()
        .rev()
        .find_map(|statement| match &statement.kind {
            hir::StatementKind::Return { value: Some(value) } => Some(value),
            _ => None,
        })
        .expect("a return with a value must exist")
}

fn local_method_callable(module: &hir::Module, callee: hir::MethodCallee) -> hir::Callable {
    if let hir::MethodCallee::DerivedEquality(application) = callee {
        return hir::Callable::Function(module.derived_equality_applications[application].function);
    }
    let Some(hir::CallableTarget::Local(callable)) =
        callee.declared_callable(&module.bound_callable_refs)
    else {
        panic!("expected a local method declaration")
    };
    callable
}
