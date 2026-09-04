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

mod annotations;
mod argument_materialization;
mod call_resolution;
mod callable_modifiers;
mod class;
mod concretize;
mod constructor_resolution;
mod core_contract;
mod declarations;
mod defaults;
mod derived;
mod effects;
mod expr;
mod ffi;
mod generic_entities;
mod globals;
mod lowering_context;
mod model;
mod overload;
mod patterns;
mod pipeline;
mod properties;
mod scope;
mod signatures;
mod stmt;
#[cfg(test)]
mod tests;
mod types;
mod visibility;

use std::collections::{HashMap, HashSet};

use la_arena::Arena;
use scoop_ast as ast;
use scoop_hir as hir;

use annotations::FunctionTarget;
use ast::{Diagnostic, Span};
use hir::{
    ClassDecl, ClassId, EnumDecl, EnumId, Function, FunctionId, FunctionKind, GenericFunction,
    GenericFunctionId, InterfaceDecl, InterfaceId, ObjectId, StructDecl, StructId, Type, TypeId,
};
use model::*;
use scope::{LocalFunctionScopes, Scopes};

/// Lower parsed source files to HIR.
///
/// `files[..len - 1]` are the `scoop.core` library sources and the
/// last file is the user compilation unit (the driver's sysroot
/// convention, milestone4 DESIGN.md 1.2): all files share a single
/// declaration scope, so core declarations are visible to user code
/// without imports. Diagnostics carry the index of the file they
/// belong to (`Diagnostic::file`).
///
/// All semantic errors of the M5 subset are diagnosed here with spans;
/// downstream stages (MIR, LIR) never fail.
pub fn lower(files: &[ast::SourceFile]) -> Result<hir::Output, Vec<Diagnostic>> {
    if files.is_empty() {
        return Lowerer::new().run(files).map(|export| hir::Output {
            local: concretize::lower(&export),
            export,
        });
    }
    let core_provider = hir::IntrinsicProviderId::from_raw(0);
    let user_provider = hir::IntrinsicProviderId::from_raw(1);
    let unit = CompilationUnit {
        core: files[..files.len() - 1]
            .iter()
            .map(|source| ProviderSource {
                source,
                provider: core_provider,
                name: "<core>",
                source_text: "",
            })
            .collect(),
        user: ProviderSource {
            source: &files[files.len() - 1],
            provider: user_provider,
            name: "<user>",
            source_text: "",
        },
    };
    lower_compilation_unit(&unit, IntrinsicDeclarationPolicy::CoreOnly)
}

/// One parsed source and the non-source identity of its provider. Multiple
/// files of one Cone carry the same provider id.
#[derive(Clone, Copy)]
pub struct ProviderSource<'a> {
    pub source: &'a ast::SourceFile,
    pub provider: hir::IntrinsicProviderId,
    pub name: &'a str,
    pub source_text: &'a str,
}

/// Structurally complete single-Cone compilation input. Core and user sources
/// cannot be confused by file position inside HIR lowering.
pub struct CompilationUnit<'a> {
    pub core: Vec<ProviderSource<'a>>,
    pub user: ProviderSource<'a>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum IntrinsicDeclarationPolicy {
    #[default]
    CoreOnly,
    AllowListedForTesting {
        providers: HashSet<hir::IntrinsicProviderId>,
    },
}

/// Lower a provider-typed compilation unit. The testing policy only grants
/// source authority; registry target, signature, shape, uniqueness, and effect
/// checks remain unchanged.
pub fn lower_compilation_unit(
    unit: &CompilationUnit<'_>,
    policy: IntrinsicDeclarationPolicy,
) -> Result<hir::Output, Vec<Diagnostic>> {
    let mut files = Vec::with_capacity(unit.core.len() + 1);
    let mut sources = Vec::with_capacity(unit.core.len() + 1);
    for input in &unit.core {
        files.push(input.source.clone());
        sources.push(SourceProvider {
            provider: input.provider,
            core: true,
            name: input.name.to_string(),
            source: input.source_text.to_string(),
        });
    }
    files.push(unit.user.source.clone());
    sources.push(SourceProvider {
        provider: unit.user.provider,
        core: false,
        name: unit.user.name.to_string(),
        source: unit.user.source_text.to_string(),
    });
    let export = Lowerer::new()
        .with_intrinsic_sources(sources, policy)
        .run(&files)?;
    let local = concretize::lower(&export);
    Ok(hir::Output { export, local })
}

#[derive(Debug, Clone)]
struct SourceProvider {
    provider: hir::IntrinsicProviderId,
    core: bool,
    name: String,
    source: String,
}

/// Convert an already checked export-side graph into the local concrete graph.
/// Kept public so stage-boundary tests can feed handcrafted checked HIR through
/// the same fixed-point pass as the production pipeline.
pub fn concretize_export(export: &hir::ExportHir) -> hir::LocalConcreteHir {
    concretize::lower(export)
}

#[derive(Clone)]
pub(crate) struct Lowerer {
    pub(crate) source_contexts: Arena<hir::SourceContext>,
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
    pub(crate) next_constructor_parameter_identity: u32,
    pub(crate) local_functions: Arena<hir::LocalFunction>,
    pub(crate) local_function_by_function: HashMap<FunctionId, hir::LocalFunctionId>,
    pub(crate) callable_references: Arena<hir::CallableReference>,
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
    pub(crate) singleton_values: Arena<hir::SingletonValue>,
    pub(crate) singleton_published_roots: Arena<hir::SingletonPublishedRoot>,
    pub(crate) properties: Arena<hir::Property>,
    pub(crate) extension_properties: Arena<hir::ExtensionProperty>,
    pub(crate) property_getters: Arena<hir::PropertyGetter>,
    pub(crate) property_setters: Arena<hir::PropertySetter>,
    pub(crate) delegate_storages: Arena<hir::DelegateStorage>,
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
    pub(crate) int: TypeId,
    /// The `UInt` well-known type (M9, spec 11.2); lowerer-internal
    /// like `any` — `hir::Module`'s well-known list is unchanged.
    pub(crate) uint: TypeId,
    pub(crate) boolean: TypeId,
    pub(crate) string: TypeId,
    /// The built-in `Any` type (milestone6 DESIGN.md 5.5).
    pub(crate) any: TypeId,
    /// Function namespace: name → overload candidates in declaration
    /// order (M7). Struct and enum names live in separate namespaces:
    /// a struct and a function may share a name.
    pub(crate) functions_by_name: HashMap<String, Vec<FunctionId>>,
    /// Top-level extension namespace. Extension declarations do not enter the
    /// ordinary function layer: they are considered only with an explicit or
    /// lexical receiver, except for `::name` callable references.
    pub(crate) extensions_by_name: HashMap<String, Vec<FunctionId>>,
    /// Resolved extension receiver type for each extension function. The HIR
    /// body represents it structurally as the first immutable `this` param.
    pub(crate) extension_receivers: HashMap<FunctionId, TypeId>,
    /// The file each top-level function was declared in, for the
    /// layering of overload resolution (user file → core implicit
    /// imports, milestone7 DESIGN.md 1.2).
    pub(crate) function_files: HashMap<FunctionId, usize>,
    /// Property namespace in declaration order. Multiple entries are needed
    /// because file-private top-level properties in different source files
    /// own distinct namespaces.
    pub(crate) properties_by_name: HashMap<String, Vec<hir::PropertyId>>,
    /// Extension properties form their own receiver-applicable namespace.
    pub(crate) extension_properties_by_name: HashMap<String, Vec<hir::PropertyId>>,
    /// Direct typed relation used after getter overload resolution; accessor
    /// function names are never parsed to recover a logical property.
    pub(crate) extension_property_by_getter: HashMap<FunctionId, hir::PropertyId>,
    pub(crate) property_files: HashMap<hir::PropertyId, usize>,
    pub(crate) property_accessor_sources: Vec<properties::PropertyAccessorSource>,
    pub(crate) runtime_accessor_units: HashMap<FunctionId, hir::InitializationUnitId>,
    pub(crate) pending_runtime_initializers: Vec<globals::PendingRuntimeInitializer>,
    pub(crate) current_initialization_unit: Option<hir::InitializationUnitId>,
    pub(crate) local_delegate_plans: HashMap<hir::BindingId, properties::LocalDelegatePlan>,
    /// Index of the user compilation unit (`files.len() - 1`); every
    /// earlier file is implicitly imported `scoop.core`.
    pub(crate) user_file_index: usize,
    /// Struct namespace: name → (declaration, value type of the struct).
    pub(crate) structs_by_name: HashMap<String, (StructId, TypeId)>,
    /// Enum namespace.
    pub(crate) enums_by_name: HashMap<String, EnumId>,
    /// Class namespace: name → (declaration, reference type of the class).
    pub(crate) classes_by_name: HashMap<String, (ClassId, TypeId)>,
    /// Interface namespace: name → (declaration, interface type).
    pub(crate) interfaces_by_name: HashMap<String, (InterfaceId, TypeId)>,
    pub(crate) objects_by_name: HashMap<String, ObjectId>,
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
    pub(crate) option_enum: Option<EnumId>,
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
    /// Surface form of every variant, for pattern shape checks.
    pub(crate) variant_styles: HashMap<(EnumId, u32), VariantStyle>,
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
    pub(crate) current_source_context: hir::SourceContextId,
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
    intrinsic_policy: IntrinsicDeclarationPolicy,
    /// Locals of the body currently being lowered (taken into the
    /// finished `hir::Body`).
    pub(crate) locals: Arena<hir::Local>,
    pub(crate) scopes: Scopes,
    pub(crate) local_function_scopes: LocalFunctionScopes,
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
    pub(crate) diagnostics: Vec<Diagnostic>,
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
