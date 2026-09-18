//! HIR stage: desugaring, type check, overload resolution, instantiation
//! requests. All compile-time errors are reported here.
//!
//! See `docs/specs/SCOOP-IMPL-SPEC.md` section 2.2.
//!
//! M2 (milestone2 DESIGN.md 2.2): struct declarations, type annotation
//! resolution, expression checking (every `Expr` leaves with its `ty`
//! filled in, every field access with a resolved `FieldRef`), and
//! block-level lexical scopes for local `val` / `var` declarations.
//!
//! M3 (milestone3 DESIGN.md 2.2): function signatures (parameters,
//! return types, `return`, expression bodies), generic functions —
//! checked once at the definition site under parameterized types, with
//! type-argument inference at call sites producing deduplicated
//! instantiation requests — and `Option<T>` with `Some` / `None` /
//! `?.` / `?:` / `!!` (desugared to statement-level control flow over
//! hidden temporaries).
//!
//! M4 (milestone4 DESIGN.md): multi-file input under the sysroot
//! framework (`scoop.core` sources first, the user file last, all in
//! one declaration scope); enum declarations with four variant forms;
//! `Option<T>` migrated from a builtin to the core library's generic
//! enum (`T?` resolves to `Type::Enum`, surface `Some(x)` / `None`
//! resolve as variant constructions); statement-level `when` with
//! pattern checking and exhaustiveness; destructuring `val` / `var`
//! declarations; and `@Intrinsic("name")` functions (core only, names
//! checked against `hir::INTRINSIC_REGISTRY`).
//!
//! M5 (milestone5 DESIGN.md 2.2): the compiler-built-in array types
//! `Array<T>` / `MutableArray<T>` (invariant in `T`), array literals
//! with context-sensitive inference (the expected type picks the kind;
//! without one every element must share a type and the result is
//! `Array<T>`), subscript reads, the `.size` pseudo-property,
//! subscript writes (`MutableArray` only), and both explicit conversion
//! forms: `Array(m)` / `MutableArray(a)` plus `m.toArray()` /
//! `a.toMutableArray()`; all four produce an independent snapshot.
//!
//! M6 (milestone6 DESIGN.md 2.2): the reference-type hierarchy — class
//! declarations (modifiers, primary-constructor properties, single
//! inheritance with base-constructor delegation, interface lists),
//! interface declarations (method signatures only), member functions
//! on classes / interfaces / structs / enums (`this` is parameter 0,
//! `Function::method` records the host type and dispatch modality), override and
//! implementation checks (value types implement interfaces too, spec
//! 4.4.3), method calls resolved against the receiver's static type,
//! typed class construction, class field reads and
//! `var` property stores (`FieldRef::ClassField`; object layout = base
//! fields prefix + own fields, indices consecutive), the `Any` type
//! with boxing at subtype crossings (`is_subtype` replaces equality
//! checks at assignment / argument / return / annotation /
//! array-element positions), `is` / `as` / `as?` / `===` (the latter
//! lowered to `BinOp::RefEq` / `RefNe`), and smart casts
//! (`if (x is T)` narrows an immutable local within the branch).
//!
//! Generic member functions remain final-only because they cannot participate
//! in virtual dispatch (spec 3.2).
//!
//! M7 (milestone7 DESIGN.md): function overloading. Top-level functions
//! and members of one host may share a name as long as their signatures
//! are distinguishable (parameter count or at least one parameter type
//! differs); calls resolve by the Kotlin-aligned two-step algorithm —
//! scope layering (host members → the call site's own side of the
//! core/user boundary → the other, implicitly imported side; the first
//! layer containing any candidate wins whole) and then the
//! most-specific candidate inside the layer (exact arity,
//! per-argument subtyping, pairwise dominance, non-generic candidates
//! preferred on ties). `print` / `println`, `ToString`, `Hash`, and
//! operator `equals` are ordinary core declarations. `@Intrinsic`
//! identifies only the minimal compiler-provided representation and
//! primitive operations; no capability has a call-site special path.
//!
//! M8 (milestone8 DESIGN.md 3.2): exceptions. `scoop.core` must define
//! a class `Throwable` (the root of the exception hierarchy, spec
//! 11.7); `throw` operands and catch parameter types must be subtypes
//! of it, catches are checked for shadowing in declaration order (a
//! catch covered by an earlier one is unreachable — an error in M8,
//! DESIGN.md 5.1), and the catch local scopes over its clause body.
//! HIR validates the complete compiler exception core and exports typed
//! zero-argument constructor targets; MIR never resolves exception classes
//! by source name.
//!
//! M9 (milestone9 DESIGN.md section 1): the `UInt` basic type (spec
//! 11.2 — a distinct type from `Int` with no implicit conversion;
//! arithmetic and comparisons follow the same rules as `Int`, with the
//! unsigned semantics risks deferred) and the core GC facilities:
//! generic structs `PinnedPtr<T>` / `GcHandle<T>` and the `pin` /
//! `unpin` / `getGcHandle`
//! / `releaseGcHandle` intrinsics. M12 now expresses their reference
//! constraint through the ordinary typed `T : ref` kind bound rather
//! than a call-site intrinsic-name special case. `gcCollect` / `gcStats`
//! are ordinary test-only intrinsics.

mod aliases;
mod annotations;
mod argument_materialization;
mod call_resolution;
mod callable_modifiers;
mod class;
mod concretize;
mod constructor_resolution;
mod core_contract;
mod declaration_surface;
mod declarations;
mod defaults;
mod definition_paths;
mod derived;
mod effects;
mod expr;
mod ffi;
mod generic_entities;
mod globals;
mod imported_core;
mod imports;
mod lowering_context;
mod model;
mod namespace;
mod ordinary_input;
mod output_kind;
mod overload;
mod patterns;
mod persistent_accessors;
mod persistent_aliases;
mod persistent_callbacks;
mod persistent_constructor_identities;
mod persistent_definition_origins;
mod persistent_dispatch;
mod persistent_enum_members;
mod persistent_export_bindings;
mod persistent_fields;
mod persistent_functions;
mod persistent_initialization_units;
mod persistent_local_bindings;
mod persistent_native_boundary;
mod persistent_native_contracts;
mod persistent_nominals;
mod persistent_object_values;
mod persistent_properties;
mod persistent_source_contexts;
mod persistent_type_identities;
mod persistent_types;
mod pipeline;
mod properties;
mod scope;
mod signatures;
mod stmt;
#[cfg(test)]
mod tests;
mod types;
mod visibility;

pub use ordinary_input::*;
pub use output_kind::select_cone_output_kind;

use std::collections::{HashMap, HashSet};

use la_arena::Arena;
use scoop_ast as ast;
use scoop_hir as hir;
pub(crate) use scoop_hir::VariantStyle;

use annotations::FunctionTarget;
use ast::{Diagnostic, Span};
use hir::{
    ClassDecl, ClassId, EnumDecl, EnumId, Function, FunctionId, FunctionKind, GenericFunction,
    GenericFunctionId, InterfaceDecl, InterfaceId, ObjectId, StructDecl, StructId, Type, TypeId,
};
use model::*;
use scope::{LocalFunctionScopes, Scopes};

/// One defined-world source used by lowerer unit tests.
#[cfg(test)]
#[derive(Clone)]
pub(crate) struct ProviderSource<'a> {
    pub source: &'a ast::SourceFile,
    pub identity: scoop_identity::SourceIdentity,
    pub provider: hir::IntrinsicProviderId,
    pub name: &'a str,
    pub source_text: &'a str,
}

/// Test-only defined-world input. Production callers must use
/// `CoreBootstrapSources`, `OrdinaryCoreOnlySources`, or `OrdinarySources`.
#[cfg(test)]
pub(crate) struct DefinedTestSources<'a> {
    core: Vec<ProviderSource<'a>>,
    user_provider: hir::IntrinsicProviderId,
    user_sources: ast::AllParsedSources,
    source_details: Vec<CurrentSourceDetails<'a>>,
}

/// Transient diagnostic and source-text data supplied separately from identity.
#[cfg(test)]
#[derive(Clone, Copy)]
pub(crate) struct CurrentSourceDetails<'a> {
    pub display_locator: &'a str,
    pub source_text: &'a str,
}

/// Parsed source input authorized for the trusted core bootstrap branch.
///
/// Construction proves that the parser output belongs to the reserved core
/// Cone. The ordinary consumer path uses a different input type carrying an
/// imported prelude capability.
pub struct CoreBootstrapSources<'a> {
    sources: &'a ast::CurrentConeParsedSources,
}

impl<'a> CoreBootstrapSources<'a> {
    pub fn try_new(
        sources: &'a ast::CurrentConeParsedSources,
    ) -> Result<Self, CoreBootstrapSourceError> {
        if sources.cone() != scoop_identity::ConeIdentity::CORE {
            return Err(CoreBootstrapSourceError::NotCore(sources.cone()));
        }
        Ok(Self { sources })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoreBootstrapSourceError {
    NotCore(scoop_identity::ConeIdentity),
}

impl std::fmt::Display for CoreBootstrapSourceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotCore(cone) => write!(
                formatter,
                "trusted core bootstrap sources belong to Cone {cone}, expected the reserved core Cone"
            ),
        }
    }
}

impl std::error::Error for CoreBootstrapSourceError {}

#[derive(Clone)]
enum CoreLoweringAuthority {
    Defined,
    Imported(Box<ImportedCoreLoweringAuthority>),
}

#[derive(Clone)]
struct ImportedCoreLoweringAuthority {
    protocols: hir::ImportedCoreProtocols,
    selection: hir::ImportedCoreSelectionPlan,
    candidates: Vec<ImportedCoreLoweringCandidate>,
}

#[derive(Clone)]
struct ImportedCoreLoweringCandidate {
    reference: hir::ImportedCorePreludeRef,
    namespace: scoop_identity::BindingNamespace,
    name: String,
    target: hir::CoreCallableTargetV1,
}

enum LoweringCompletion {
    Defined,
    Imported {
        core: hir::ImportedCoreSelectionPlan,
        dependencies: hir::ImportedDependencySelectionPlan,
    },
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DefinedTestSourcesError {
    DuplicateSourceIdentity {
        first_index: usize,
        duplicate_index: usize,
        identity: scoop_identity::SourceIdentity,
    },
    MixedCurrentCones {
        first: scoop_identity::ConeIdentity,
        source_index: usize,
        actual: scoop_identity::ConeIdentity,
    },
}

#[cfg(test)]
impl std::fmt::Display for DefinedTestSourcesError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateSourceIdentity {
                first_index,
                duplicate_index,
                identity,
            } => write!(
                formatter,
                "source {duplicate_index} duplicates source {first_index} identity {}/{}",
                identity.cone(),
                identity.logical_path()
            ),
            Self::MixedCurrentCones {
                first,
                source_index,
                actual,
            } => write!(
                formatter,
                "current source {source_index} belongs to Cone {actual}, expected {first}",
            ),
        }
    }
}

#[cfg(test)]
impl std::error::Error for DefinedTestSourcesError {}

#[cfg(test)]
impl<'a> DefinedTestSources<'a> {
    /// Consume a validated parsed set. Details are requested by source identity,
    /// never matched by display locator or source container position.
    pub fn try_new(
        core: Vec<ProviderSource<'a>>,
        user_provider: hir::IntrinsicProviderId,
        user_sources: ast::AllParsedSources,
        mut source_details: impl FnMut(&scoop_identity::SourceIdentity) -> CurrentSourceDetails<'a>,
    ) -> Result<Self, DefinedTestSourcesError> {
        let first_cone = user_sources.sources().first().identity().cone();
        for (source_index, source) in user_sources.sources().iter().enumerate().skip(1) {
            let actual = source.identity().cone();
            if actual != first_cone {
                return Err(DefinedTestSourcesError::MixedCurrentCones {
                    first: first_cone,
                    source_index,
                    actual,
                });
            }
        }
        let mut seen = Vec::with_capacity(core.len() + user_sources.sources().len());
        for (index, identity) in core
            .iter()
            .map(|source| &source.identity)
            .chain(
                user_sources
                    .sources()
                    .iter()
                    .map(|source| source.identity()),
            )
            .enumerate()
        {
            if let Some(first_index) = seen
                .iter()
                .position(|seen: &&scoop_identity::SourceIdentity| *seen == identity)
            {
                return Err(DefinedTestSourcesError::DuplicateSourceIdentity {
                    first_index,
                    duplicate_index: index,
                    identity: identity.clone(),
                });
            }
            seen.push(identity);
        }
        let source_details = user_sources
            .sources()
            .iter()
            .map(|source| source_details(source.identity()))
            .collect();
        Ok(Self {
            core,
            user_provider,
            user_sources,
            source_details,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum IntrinsicDeclarationPolicy {
    #[default]
    CoreOnly,
    AllowListedForTesting {
        providers: HashSet<hir::IntrinsicProviderId>,
    },
}

/// Lower a defined-world source set for unit tests. This is deliberately not
/// compiled into the production crate API.
#[cfg(test)]
pub(crate) fn lower_defined_for_test(
    requested: scoop_identity::RequestedConeKind,
    input: &DefinedTestSources<'_>,
    policy: IntrinsicDeclarationPolicy,
) -> Result<hir::Output, Vec<Diagnostic>> {
    let (files, sources) = materialize_defined_test_sources(input);
    let (export, warnings) = Lowerer::new()
        .with_intrinsic_sources(sources, policy)
        .run_defined(&files)?;
    let output_kind = select_cone_output_kind(&export, requested)?;
    finish_output(export, output_kind, warnings)
}

/// Lowers the trusted core directly from the atomic current-Cone parser
/// product. This path does not construct or pass through a combined legacy
/// source set.
pub fn lower_core_bootstrap(
    input: &CoreBootstrapSources<'_>,
) -> Result<hir::Output, Vec<Diagnostic>> {
    let (files, sources) = materialize_core_bootstrap_sources(input.sources);
    let (export, warnings) = Lowerer::new()
        .with_intrinsic_sources(sources, IntrinsicDeclarationPolicy::CoreOnly)
        .run_defined(&files)?;
    finish_output(export, hir::ConeOutputKind::Library, warnings)
}

/// Lowers one ordinary Cone against the imported protocol and prelude
/// authority of its exact trusted-core artifact.
pub fn lower_ordinary_core_only<'core>(
    requested: scoop_identity::RequestedConeKind,
    input: &OrdinaryCoreOnlySources<'core>,
) -> Result<hir::OrdinaryHirOutput<'core>, Vec<Diagnostic>> {
    lower_ordinary_input(requested, input, None)
}

/// Lowers one ordinary Cone against its trusted core and validated ordinary
/// dependency semantic world. Dependency import/re-export resolution is
/// enabled here; executable dependency uses remain subject to the HIR
/// selection and capability gates.
pub fn lower_ordinary<'input>(
    requested: scoop_identity::RequestedConeKind,
    input: &OrdinarySources<'input>,
) -> Result<hir::OrdinaryHirOutput<'input>, Vec<Diagnostic>> {
    lower_ordinary_input(requested, input.core_only(), Some(input.semantic_world()))
}

fn lower_ordinary_input<'core>(
    requested: scoop_identity::RequestedConeKind,
    input: &OrdinaryCoreOnlySources<'core>,
    world: Option<&hir::ImportedSemanticWorld<'_>>,
) -> Result<hir::OrdinaryHirOutput<'core>, Vec<Diagnostic>> {
    let (files, sources) = materialize_ordinary_sources(input.sources());
    let dependency_selection = match world {
        Some(world) => {
            let classifier = input
                .core()
                .core_closed_exact_leaf_classifier()
                .map_err(|error| {
                    vec![Diagnostic::at(
                        Span { start: 0, end: 0 },
                        format!("failed to classify trusted core ABI leaves: {error}"),
                    )]
                })?;
            world
                .dependency_selection_plan(&classifier)
                .map_err(|error| {
                    vec![Diagnostic::at(
                        Span { start: 0, end: 0 },
                        format!("failed to prepare dependency selection: {error}"),
                    )]
                })?
        }
        None => hir::ImportedDependencySelectionPlan::empty(input.current_cone()),
    };
    let (module, warnings, core_selection, dependency_selection) = Lowerer::new()
        .with_intrinsic_sources(sources, IntrinsicDeclarationPolicy::CoreOnly)
        .with_imported_core(input.core())
        .with_imported_dependencies(dependency_selection)
        .run_imported(&files, world)?;
    let output_kind = select_cone_output_kind(&module, requested)?;
    let export = hir::ExportHirOutput::try_new(module, output_kind).map_err(|error| {
        vec![Diagnostic::at(
            Span { start: 0, end: 0 },
            format!("failed to seal ordinary Export HIR output: {error}"),
        )]
    })?;
    let local = concretize::lower_output(&export);
    let native_boundary_types = crate::persistent_native_boundary::build(
        export.module(),
        local.module(),
        hir::HirNativeBoundaryExternalTypes::TrustedCore(input.core().native_boundary_types()),
    )
    .map_err(native_boundary_diagnostic)?;
    let output =
        hir::Output::try_new(export, local, native_boundary_types, warnings).map_err(|error| {
            vec![Diagnostic::at(
                Span { start: 0, end: 0 },
                format!("failed to seal ordinary HIR output: {error}"),
            )]
        })?;
    let selected = input.bind_core_selection(core_selection).map_err(|error| {
        vec![Diagnostic::at(
            Span { start: 0, end: 0 },
            format!("failed to bind ordinary core selection: {error}"),
        )]
    })?;
    hir::OrdinaryHirOutput::try_new(output, selected, dependency_selection.finish()).map_err(
        |error| {
            vec![Diagnostic::at(
                Span { start: 0, end: 0 },
                format!("failed to seal ordinary imported-core HIR: {error}"),
            )]
        },
    )
}

fn finish_output(
    export: hir::ExportHir,
    output_kind: hir::ConeOutputKind,
    warnings: Vec<Diagnostic>,
) -> Result<hir::Output, Vec<Diagnostic>> {
    let export = hir::ExportHirOutput::try_new(export, output_kind).map_err(|error| {
        vec![Diagnostic::at(
            Span { start: 0, end: 0 },
            format!("failed to seal Export HIR output: {error}"),
        )]
    })?;
    let local = if export.module().cone == scoop_identity::ConeIdentity::CORE {
        let production =
            hir::CoreBootstrapInterfaceSectionV1::from_export(&export).map_err(|error| {
                vec![Diagnostic::at(
                    Span { start: 0, end: 0 },
                    format!("failed to project core HIR interface: {error}"),
                )]
            })?;
        let hir::CoreHirInterfaceBranchV1::Core(interface) = production.core_interface() else {
            unreachable!("the core HIR production section carries its core interface")
        };
        concretize::lower_core_output(&export, &interface.shape_support_requirements())
    } else {
        concretize::lower_output(&export)
    };
    let native_boundary_types = crate::persistent_native_boundary::build(
        export.module(),
        local.module(),
        hir::HirNativeBoundaryExternalTypes::CurrentArtifactOnly,
    )
    .map_err(native_boundary_diagnostic)?;
    hir::Output::try_new(export, local, native_boundary_types, warnings).map_err(|error| {
        vec![Diagnostic::at(
            Span { start: 0, end: 0 },
            format!("failed to seal HIR output: {error}"),
        )]
    })
}

fn native_boundary_diagnostic(
    error: persistent_native_boundary::PersistentNativeBoundaryTypeError,
) -> Vec<Diagnostic> {
    vec![Diagnostic::at(Span { start: 0, end: 0 }, error.to_string())]
}

#[cfg(test)]
fn materialize_defined_test_sources(
    input: &DefinedTestSources<'_>,
) -> (Vec<ast::SourceFile>, Vec<SourceProvider>) {
    let source_count = input.core.len() + input.user_sources.sources().len();
    let mut files = Vec::with_capacity(source_count);
    let mut sources = Vec::with_capacity(source_count);
    for source in &input.core {
        files.push(source.source.clone());
        sources.push(SourceProvider {
            provider: source.provider,
            kind: SourceKind::Core,
            identity: source.identity.clone(),
            name: source.name.to_string(),
            source: source.source_text.to_string(),
        });
    }
    for (source, details) in input
        .user_sources
        .sources()
        .iter()
        .zip(&input.source_details)
    {
        files.push(source.ast().clone());
        sources.push(SourceProvider {
            provider: input.user_provider,
            kind: SourceKind::CurrentUnit,
            identity: source.identity().clone(),
            name: details.display_locator.to_string(),
            source: details.source_text.to_string(),
        });
    }
    (files, sources)
}

fn materialize_core_bootstrap_sources(
    input: &ast::CurrentConeParsedSources,
) -> (Vec<ast::SourceFile>, Vec<SourceProvider>) {
    let mut files = Vec::with_capacity(input.sources().sources().len());
    let mut sources = Vec::with_capacity(input.sources().sources().len());
    for source in input.iter() {
        files.push(source.source().ast().clone());
        sources.push(SourceProvider {
            provider: hir::IntrinsicProviderId::from_raw(0),
            kind: SourceKind::Core,
            identity: source.source().identity().clone(),
            name: source.diagnostic().display_locator().display().to_string(),
            source: source.text().text().to_owned(),
        });
    }
    (files, sources)
}

fn materialize_ordinary_sources(
    input: &ast::CurrentConeParsedSources,
) -> (Vec<ast::SourceFile>, Vec<SourceProvider>) {
    let mut files = Vec::with_capacity(input.sources().sources().len());
    let mut sources = Vec::with_capacity(input.sources().sources().len());
    for source in input.iter() {
        files.push(source.source().ast().clone());
        sources.push(SourceProvider {
            provider: hir::IntrinsicProviderId::from_raw(0),
            kind: SourceKind::CurrentUnit,
            identity: source.source().identity().clone(),
            name: source.diagnostic().display_locator().display().to_string(),
            source: source.text().text().to_owned(),
        });
    }
    (files, sources)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceKind {
    Core,
    CurrentUnit,
}

#[derive(Debug, Clone)]
struct SourceProvider {
    provider: hir::IntrinsicProviderId,
    kind: SourceKind,
    identity: scoop_identity::SourceIdentity,
    name: String,
    source: String,
}

/// Source spelling retained for every source FunctionId.
/// Generated/accessor/initialization functions are deliberately absent.
#[derive(Debug, Clone)]
pub(crate) struct SourceFunctionDeclaration {
    pub(crate) name: String,
}

/// Convert an already checked export-side graph into the local concrete graph.
/// Kept public so stage-boundary tests can feed handcrafted checked HIR through
/// the same fixed-point pass as the production pipeline.
pub fn concretize_export(export: &hir::ExportHir) -> hir::LocalConcreteHir {
    concretize::lower(export)
}

/// Concretize a checked, output-sealed Export HIR graph while translating the
/// output branch into the LocalConcrete HIR id domain.
pub fn concretize_output(export: &hir::ExportHirOutput) -> hir::LocalConcreteHirOutput {
    concretize::lower_output(export)
}

#[derive(Clone)]
pub(crate) struct Lowerer {
    core: CoreLoweringAuthority,
    dependencies: Option<hir::ImportedDependencySelectionPlan>,
    pub(crate) source_contexts: Arena<hir::SourceContext>,
    source_context_by_value: HashMap<hir::SourceContext, hir::SourceContextId>,
    file_source_contexts: Vec<hir::SourceContextId>,
    /// Role-local structural paths for the definition owner currently being
    /// lowered. Nested callables replace this context and restore it on exit.
    pub(crate) definition_paths: definition_paths::DefinitionPathContext,
    /// Typed root of the active stable lexical traversal. It is present only
    /// while lowering a function, constructor, or declaration-bound default.
    pub(crate) definition_root: Option<hir::LexicalDefinitionRoot>,
    /// Constructor expressions are lowered in several semantic passes. Their
    /// owner-local counters persist between those regions so source-order
    /// sites never fall back to a pass-local or arena-local ordinal.
    pub(crate) constructor_definition_paths:
        HashMap<class::ConstructorSource, definition_paths::DefinitionPathContext>,
    pub(crate) imports: imports::CurrentUnitImports,
    /// Published once after every source callable signature is resolved.
    /// Rejected ids remain diagnostic-only and never enter body resolution.
    pub(crate) declaration_surface: declaration_surface::DeclarationSurface,
    pub(crate) types: Arena<Type>,
    pub(crate) function_types: Arena<hir::FunctionType>,
    pub(crate) lambdas: Arena<hir::Lambda>,
    pub(crate) anonymous_functions: Arena<hir::AnonymousFunction>,
    /// Generated callable body names are reserved before lowering their
    /// bodies so nested literals with the same signature cannot collide.
    pub(crate) next_lambda_function: u32,
    pub(crate) next_anonymous_function: u32,
    /// Cone-wide semantic type-parameter identity allocator. Substitution
    /// slots are assigned separately by each complete lexical signature.
    pub(crate) next_type_param_identity: u32,
    /// Cone-wide source identity allocator for class virtual method families.
    pub(crate) next_virtual_method_identity: u32,
    /// Declaration root captured when each virtual family is created.
    pub(crate) virtual_method_roots: HashMap<hir::VirtualMethodId, FunctionId>,
    pub(crate) next_constructor_parameter_identity: u32,
    /// Cone-wide identity allocator for structured loop occurrences. The
    /// active target stack below is callable-local, but identities remain
    /// unique when declaration-bound templates are materialized repeatedly.
    pub(crate) next_loop_identity: u32,
    pub(crate) local_functions: Arena<hir::LocalFunction>,
    pub(crate) local_function_by_function: HashMap<FunctionId, hir::LocalFunctionId>,
    pub(crate) callable_references: Arena<hir::CallableReference>,
    pub(crate) imported_core_callables: Arena<hir::ImportedCoreCallableUse>,
    pub(crate) imported_dependency_callables: Arena<hir::ImportedDependencyCallableUse>,
    pub(crate) imported_core_types: Arena<hir::ImportedCoreTypeUse>,
    pub(crate) imported_core_values: Arena<hir::ImportedCoreValueUse>,
    pub(crate) bound_callable_refs: Arena<hir::BoundCallableRef>,
    pub(crate) function_coercions: Arena<hir::FunctionCoercion>,
    pub(crate) foreign_callback_registrations: Arena<hir::ForeignCallbackRegistration>,
    pub(crate) source_parameter_interfaces: Vec<hir::ExportParameterInterface>,
    pub(crate) export_default_exprs: Arena<hir::ExportDefaultExpr>,
    pub(crate) export_default_sources: Arena<hir::ExportDefaultSource>,
    pub(crate) export_vararg_parameter_types: Arena<hir::ExportVarargParameterType>,
    pub(crate) local_default_exprs: Arena<defaults::LocalDefaultExpr>,
    pub(crate) default_templates:
        HashMap<(defaults::SourceParameterOwner, u32), defaults::DefaultExprTemplateRef>,
    /// True only while constructing a declaration-bound default template.
    /// Nested omissions remain definition-only until the outer template is
    /// instantiated at an actual call site.
    pub(crate) lowering_default_template: bool,
    pub(crate) function_coercion_by_types:
        HashMap<(hir::FunctionTypeId, hir::FunctionTypeId), hir::FunctionCoercionId>,
    pub(crate) structs: Arena<StructDecl>,
    /// Source locations aligned with the checked source-field refs. Field
    /// resolution may reject individual AST entries, so raw source ordinals
    /// are not a valid substitute for this typed relation.
    pub(crate) struct_field_spans: HashMap<hir::StructFieldRef, Span>,
    pub(crate) struct_constructors: Arena<hir::StructConstructor>,
    pub(crate) struct_constructor_applications: Arena<hir::StructConstructorApplication>,
    pub(crate) struct_constructor_application_by_key: HashMap<
        (hir::StructConstructorId, hir::StructApplicationId),
        hir::StructConstructorApplicationId,
    >,
    pub(crate) struct_applications: Arena<hir::StructApplication>,
    pub(crate) struct_application_by_key:
        HashMap<(StructId, Vec<TypeId>), hir::StructApplicationId>,
    pub(crate) enums: Arena<EnumDecl>,
    pub(crate) enum_variant_spans: HashMap<hir::EnumVariantRef, Span>,
    pub(crate) enum_variant_field_spans: HashMap<hir::EnumVariantFieldRef, Span>,
    pub(crate) enum_applications: Arena<hir::EnumApplication>,
    pub(crate) enum_application_by_key: HashMap<(EnumId, Vec<TypeId>), hir::EnumApplicationId>,
    pub(crate) classes: Arena<ClassDecl>,
    pub(crate) class_fields: Arena<hir::ClassField>,
    pub(crate) class_constructors: Arena<hir::ClassConstructor>,
    pub(crate) class_constructor_applications: Arena<hir::ClassConstructorApplication>,
    pub(crate) class_constructor_application_by_key: HashMap<
        (hir::ClassConstructorId, hir::ClassApplicationId),
        hir::ClassConstructorApplicationId,
    >,
    pub(crate) class_applications: Arena<hir::ClassApplication>,
    pub(crate) class_application_by_key: HashMap<(ClassId, Vec<TypeId>), hir::ClassApplicationId>,
    pub(crate) interfaces: Arena<InterfaceDecl>,
    pub(crate) interface_applications: Arena<hir::InterfaceApplication>,
    pub(crate) interface_application_by_key:
        HashMap<(InterfaceId, Vec<TypeId>), hir::InterfaceApplicationId>,
    pub(crate) functions: Arena<Function>,
    /// Typed source-declaration provenance. Display names cannot substitute
    /// for this relation because member and lifted-local names are decorated.
    pub(crate) source_function_declarations: HashMap<FunctionId, SourceFunctionDeclaration>,
    /// Complete source relation for every validated intrinsic kind. Duplicate
    /// declarations are diagnosed at insertion; core contract validation reads
    /// this map directly and never scans functions or compares names.
    pub(crate) intrinsic_functions:
        HashMap<hir::IntrinsicFunctionKind, (FunctionId, hir::IntrinsicProviderId)>,
    intrinsic_type_owners:
        HashMap<hir::IntrinsicTypeKind, (IntrinsicTypeOwner, hir::IntrinsicProviderId)>,
    pub(crate) extern_functions: Arena<hir::ExternFunction>,
    pub(crate) globals: Arena<hir::Global>,
    pub(crate) initialization_units: Arena<hir::InitializationUnit>,
    pub(crate) initialization_failure_roots: Arena<hir::InitializationFailureRoot>,
    pub(crate) objects: Arena<hir::ObjectDecl>,
    pub(crate) object_types: Arena<hir::ObjectType>,
    pub(crate) companion_relations: Arena<hir::CompanionRelation>,
    pub(crate) companion_by_host: HashMap<Owner, hir::CompanionRelationId>,
    pub(crate) singleton_values: Arena<hir::SingletonValue>,
    pub(crate) singleton_published_roots: Arena<hir::SingletonPublishedRoot>,
    pub(crate) properties: Arena<hir::Property>,
    pub(crate) extension_properties: Arena<hir::ExtensionProperty>,
    pub(crate) property_getters: Arena<hir::PropertyGetter>,
    pub(crate) property_setters: Arena<hir::PropertySetter>,
    pub(crate) delegate_storages: Arena<hir::DelegateStorage>,
    /// Resolver-only alias declarations. Their ids and resolution state never
    /// cross the Export HIR boundary.
    pub(crate) source_type_aliases: Arena<aliases::SourceTypeAlias>,
    pub(crate) top_level_namespaces: namespace::TopLevelNamespaces,
    pub(crate) type_aliases: Arena<hir::TypeAliasDecl>,
    pub(crate) type_alias_resolution_stack: Vec<aliases::SourceTypeAliasId>,
    /// Generic definitions are separate HIR entities. Every function carries
    /// the matching typed id in `Function::genericity`, so this arena is never
    /// reverse-scanned and no parallel reverse map can drift out of sync.
    pub(crate) generic_functions: Arena<GenericFunction>,
    pub(crate) method_applications: Arena<hir::MethodApplication>,
    pub(crate) method_application_by_key:
        HashMap<(FunctionId, hir::MethodOwnerApplication), hir::MethodApplicationId>,
    pub(crate) generic_methods: Arena<hir::GenericMethod>,
    pub(crate) generic_method_applications: Arena<hir::GenericMethodApplication>,
    pub(crate) generic_method_application_by_key: HashMap<
        (
            hir::GenericMethodId,
            hir::GenericMethodOwner,
            hir::NonEmptyVec<TypeId>,
        ),
        hir::GenericMethodApplicationId,
    >,
    pub(crate) derived_equality_applications: Arena<hir::DerivedEqualityApplication>,
    pub(crate) derived_equality_application_by_type:
        HashMap<TypeId, hir::DerivedEqualityApplicationId>,
    pub(crate) top_level: Vec<FunctionId>,
    pub(crate) unit: TypeId,
    /// Total lowering-time map for the eight canonical integer identities.
    /// Source spelling, width and signedness never need to be reconstructed
    /// from arena order or nominal names.
    pub(crate) integer_types: hir::IntegerTypeCore<TypeId>,
    pub(crate) boolean: TypeId,
    pub(crate) string: TypeId,
    /// The built-in `Any` type (milestone6 DESIGN.md 5.5).
    pub(crate) any: TypeId,
    /// Resolved extension receiver type for each extension function. The HIR
    /// body represents it structurally as the first immutable `this` param.
    pub(crate) extension_receivers: HashMap<FunctionId, TypeId>,
    /// The file each top-level function was declared in, for the
    /// layering of overload resolution (user file → core implicit
    /// imports, milestone7 DESIGN.md 1.2).
    pub(crate) function_files: HashMap<FunctionId, usize>,
    /// Direct typed relation used after getter overload resolution; accessor
    /// function names are never parsed to recover a logical property.
    pub(crate) extension_property_by_getter: HashMap<FunctionId, hir::PropertyId>,
    pub(crate) property_files: HashMap<hir::PropertyId, usize>,
    pub(crate) property_accessor_sources: Vec<properties::PropertyAccessorSource>,
    pub(crate) runtime_accessor_units: HashMap<FunctionId, hir::InitializationUnitId>,
    pub(crate) pending_runtime_initializers: Vec<globals::PendingRuntimeInitializer>,
    pub(crate) current_initialization_unit: Option<hir::InitializationUnitId>,
    pub(crate) local_delegate_plans: HashMap<hir::BindingId, properties::LocalDelegatePlan>,
    /// Physical class representation -> semantic singleton declaration.
    /// The relation is typed and established when the object is declared;
    /// constructor/body lowering never recovers it from a generated name.
    pub(crate) object_by_backing_class: HashMap<ClassId, ObjectId>,
    /// Owner-scoped static nested nominal namespace. The owner and target
    /// retain distinct typed ids; qualified lookup never flattens this key
    /// into an FQN string.
    pub(crate) nested_nominals_by_owner: HashMap<(Owner, String), NominalTarget>,
    /// Source-file ownership for validating compiler-known core contracts.
    pub(crate) struct_files: HashMap<StructId, usize>,
    pub(crate) enum_files: HashMap<EnumId, usize>,
    pub(crate) class_files: HashMap<ClassId, usize>,
    pub(crate) interface_files: HashMap<InterfaceId, usize>,
    pub(crate) object_files: HashMap<ObjectId, usize>,
    /// Core pointer declarations discovered after pass 1. Applications can
    /// then normalize while pass 2/2.5 resolves fields and signatures.
    pub(crate) ffi_ptr: Option<StructId>,
    pub(crate) ffi_fun_ptr: Option<StructId>,
    pub(crate) ffi_pinned_ptr: Option<StructId>,
    pub(crate) ffi_gc_handle: Option<StructId>,
    pub(crate) ffi_foreign_callback: Option<StructId>,
    /// Fully validated pointer core, available while lowering user bodies.
    pub(crate) ffi_core: Option<hir::FfiCore>,
    pub(crate) foreign_callback_core: Option<hir::ForeignCallbackCore>,
    pub(crate) allow_deferred_fun_ptr: bool,
    pub(crate) pointer_type_uses: Vec<(TypeId, usize, Span)>,
    pub(crate) fun_ptr_type_uses: Vec<(TypeId, usize, Span)>,
    /// Interface member functions in declaration order. Class/struct/enum
    /// declarations carry their member ids directly in export HIR.
    pub(crate) interface_methods: HashMap<InterfaceId, Vec<FunctionId>>,
    /// Typed export identities for the functions in `interface_methods`.
    /// The map above is a lowering-time lookup index; this arena is the
    /// authoritative relation emitted to ExportHir.
    pub(crate) interface_method_entities: Arena<hir::InterfaceMethod>,
    /// The owner of every member function.
    pub(crate) function_owner: HashMap<FunctionId, Owner>,
    pub(crate) override_sources: HashMap<FunctionId, Vec<FunctionId>>,
    /// Exact parent-parameter applications for every validated override edge.
    /// The values are ordered like the parent signature's type parameters and
    /// are expressed in the overriding declaration's type scope.
    pub(crate) override_default_type_arguments: HashMap<(FunctionId, FunctionId), Vec<TypeId>>,
    /// Enums named `Option` declared in core files:
    /// (declaration, file index, span, type parameter count). Validated
    /// after pass 1 (`validate_option_enum`).
    option_candidates: Vec<(EnumId, usize, Span, usize)>,
    /// The validated `Option<T>` enum of `scoop.core`; `None` only
    /// when the core library is misconfigured (diagnosed, so the
    /// module is rejected anyway).
    pub(crate) pending_option_enum: Option<EnumId>,
    /// Complete typed Option contract, established only after pass 2 has
    /// resolved and checked the `Some(T)` / `None` variant shapes.
    pub(crate) option_core: Option<hir::OptionCore>,
    /// Complete typed source-iteration contract, established after interface
    /// method signatures and the canonical Option relation are available.
    pub(crate) iteration_core: Option<hir::IterationCore>,
    /// Ordinary core-prelude variant bindings. Contextual enum lookup is a
    /// separate, lower-priority layer and never populates this table.
    pub(crate) core_prelude_variants: CorePreludeVariantBindings,
    /// Classes named `Throwable` declared in core files, in
    /// declaration order ((declaration, reference type)). Validated
    /// after pass 1 (`validate_throwable`); a second `Throwable` was
    /// already rejected as a duplicate class in pass 1.
    throwable_candidates: Vec<(ClassId, TypeId)>,
    /// The validated `Throwable` class of `scoop.core` and its
    /// reference type (M8); `None` only when the core library is
    /// misconfigured (diagnosed, so the module is rejected anyway). This
    /// lowering-time lookup feeds the complete typed `CompilerExceptionCore`
    /// emitted after class representations and inheritance are resolved.
    pub(crate) throwable: Option<(ClassId, TypeId)>,
    /// Source-call protocols for nominal constructor parameters. Layout
    /// fields intentionally do not carry call syntax; these typed owner maps
    /// preserve it until complete Export HIR parameter entities are built.
    pub(crate) struct_parameter_calling: HashMap<hir::StructConstructorId, Vec<FnParamCalling>>,
    pub(crate) class_parameter_calling: HashMap<hir::ClassConstructorId, Vec<FnParamCalling>>,
    pub(crate) variant_parameter_calling: HashMap<(EnumId, u32), Vec<FnParamCalling>>,
    /// Resolved signatures of all functions (pass 2.5), consulted by
    /// call lowering and body lowering. Method signatures exclude the
    /// implicit `this` parameter.
    pub(crate) signatures: HashMap<FunctionId, FnSig>,
    /// Type parameter names of the function or enum whose signature,
    /// variants or body is currently being lowered; empty elsewhere.
    pub(crate) type_params_in_scope: Vec<hir::TypeParamDecl>,
    /// Return type of the function whose body is being lowered.
    pub(crate) current_return_ty: TypeId,
    /// Active only while an anonymous function with neither an explicit nor
    /// expected return type is lowered.
    pub(crate) return_inference: Option<ReturnInference>,
    /// Name of the function whose body is being lowered (diagnostics).
    pub(crate) current_fn_name: String,
    /// Typed lexical context attached to every expression origin.
    pub(crate) current_source_context: Option<hir::SourceContextId>,
    /// Explicit suspension-permission stack; it is never empty.
    pub(crate) suspension_contexts: Vec<SuspensionContext>,
    /// Lexical permission for unsafe operations; independent of suspension.
    pub(crate) safety_contexts: Vec<hir::Safety>,
    /// `this` of the member function whose body is being lowered:
    /// its local and the host type. `None` in top-level functions.
    pub(crate) current_this: Option<(hir::LocalId, TypeId)>,
    /// Owner of the member function whose body is being lowered, for
    /// bare property / method resolution (`x` meaning `this.x`).
    pub(crate) current_owner: Option<Owner>,
    /// Typed primary-constructor parameters visible only while lowering a
    /// base-constructor delegation expression.
    pub(crate) constructor_params_in_scope:
        HashMap<String, (hir::ConstructorParamId, TypeId, hir::BindingId)>,
    /// Cone-wide lexical identities for constructor parameters. They are
    /// separate from the typed parameter ids so nested callables can capture
    /// parameter values without turning a constructor into a function.
    pub(crate) constructor_parameter_bindings: HashMap<hir::ConstructorParamId, hir::BindingId>,
    /// Non-escaping receiver capability used only while checking constructor
    /// initialization plans. Successful reads and writes are immediately
    /// converted to typed field identities.
    pub(crate) initialization_context: Option<InitializationContext>,
    pub(crate) backing_field_context: Option<properties::BackingFieldContext>,
    /// Active smart-cast narrowings (milestone6 DESIGN.md 5.4):
    /// immutable local → narrowed type, valid within the branch that
    /// established them. Saved and restored around branch lowering;
    /// the declared type of a local never changes.
    pub(crate) smart_casts: HashMap<hir::LocalId, TypeId>,
    /// Index of the file currently being processed (diagnostics).
    pub(crate) current_file: usize,
    intrinsic_sources: Vec<SourceProvider>,
    /// Authenticated dependency sources referenced by instantiated defaults.
    /// They are appended only after a winning candidate is committed and are
    /// never traversed as parser inputs.
    imported_source_files: Vec<hir::SourceFileMetadata>,
    imported_source_indices: HashMap<scoop_identity::SourceIdentity, u32>,
    intrinsic_policy: IntrinsicDeclarationPolicy,
    /// Locals of the body currently being lowered (taken into the
    /// finished `hir::Body`).
    pub(crate) locals: Arena<hir::Local>,
    pub(crate) scopes: Scopes,
    pub(crate) local_function_scopes: LocalFunctionScopes,
    /// Lexically active loop targets in the current callable or detached
    /// declaration-bound expression. Callable boundaries replace this with an
    /// empty stack and restore the enclosing stack on exit.
    pub(crate) loop_targets: Vec<hir::LoopId>,
    /// Active nested callable capture analyses. The outer callable remains
    /// on the stack while an inner one is lowered so transitive captures can
    /// be propagated without reading an exited native stack frame.
    pub(crate) capture_contexts: Vec<CaptureContext>,
    /// Monotonic Cone-wide lexical binding identity allocator.
    pub(crate) next_binding_id: u32,
    /// Deduplicated monomorphization requests, in first-use order.
    pub(crate) instantiations: Arena<hir::ResolvedGenericFunction>,
    /// Counter for hidden `$opt.N` / `$res.N` desugaring temporaries.
    pub(crate) hidden_count: u32,
    /// Fatal semantic diagnostics. Candidate transactions use this vector's
    /// length as their error baseline; warnings must remain separate.
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) warnings: Vec<Diagnostic>,
}

#[derive(Clone)]
pub(crate) struct InitializationContext {
    pub(crate) receiver: InitializingReceiver,
    pub(crate) step: String,
    /// Nested callables cannot retain the receiver capability. The depth at
    /// which initialization began distinguishes their bodies from the
    /// constructor expression itself.
    pub(crate) capture_depth: usize,
}

#[derive(Clone)]
pub(crate) enum InitializingReceiver {
    Class {
        application: hir::ClassApplicationId,
        initialized: HashSet<hir::ClassFieldId>,
    },
    Struct {
        application: hir::StructApplicationId,
    },
}

#[derive(Clone, Copy)]
enum GcIntrinsic {
    Pin,
    Unpin,
    GetHandle,
    ReleaseHandle,
}

impl GcIntrinsic {
    fn name(self) -> &'static str {
        match self {
            Self::Pin => "gc_pin_raw",
            Self::Unpin => "gc_unpin_raw",
            Self::GetHandle => "gc_get_handle_raw",
            Self::ReleaseHandle => "gc_release_handle_raw",
        }
    }
}
