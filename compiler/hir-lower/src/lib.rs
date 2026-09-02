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
//! class construction (`ExprKind::ClassInit`), class field reads and
//! `var` property stores (`FieldRef::ClassField`; object layout = base
//! fields prefix + own fields, indices consecutive), the `Any` type
//! with boxing at subtype crossings (`is_subtype` replaces equality
//! checks at assignment / argument / return / annotation /
//! array-element positions), `is` / `as` / `as?` / `===` (the latter
//! lowered to `BinOp::RefEq` / `RefNe`), and smart casts
//! (`if (x is T)` narrows an immutable local within the branch).
//!
//! Remaining M6 simplifications: base-constructor delegation arguments
//! are lowered in an empty scope (constructor properties are not in
//! scope there — HIR has no body to host their locals), and generic
//! member functions are diagnosed (they cannot participate in virtual
//! dispatch, spec 3.2).
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
mod class;
mod concretize;
mod core_contract;
mod declarations;
mod derived;
mod effects;
mod expr;
mod ffi;
mod generic_entities;
mod globals;
mod lowering_context;
mod overload;
mod patterns;
mod pipeline;
mod scope;
mod signatures;
mod stmt;
#[cfg(test)]
mod tests;
mod types;

use std::collections::{HashMap, HashSet};

use la_arena::Arena;
use scoop_ast as ast;
use scoop_hir as hir;

use annotations::FunctionTarget;
use ast::{Diagnostic, Span};
use hir::{
    ClassDecl, ClassId, EnumDecl, EnumId, Function, FunctionId, FunctionKind, GenericFunction,
    GenericFunctionId, InterfaceDecl, InterfaceId, StructDecl, StructId, Type, TypeId,
};
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
            })
            .collect(),
        user: ProviderSource {
            source: &files[files.len() - 1],
            provider: user_provider,
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
        });
    }
    files.push(unit.user.source.clone());
    sources.push(SourceProvider {
        provider: unit.user.provider,
        core: false,
    });
    let export = Lowerer::new()
        .with_intrinsic_sources(sources, policy)
        .run(&files)?;
    let local = concretize::lower(&export);
    Ok(hir::Output { export, local })
}

#[derive(Debug, Clone, Copy)]
struct SourceProvider {
    provider: hir::IntrinsicProviderId,
    core: bool,
}

/// Convert an already checked export-side graph into the local concrete graph.
/// Kept public so stage-boundary tests can feed handcrafted checked HIR through
/// the same fixed-point pass as the production pipeline.
pub fn concretize_export(export: &hir::ExportHir) -> hir::LocalConcreteHir {
    concretize::lower(export)
}

/// A resolved function signature. Kept separate from `hir::Function`
/// because parameter locals can only be allocated while the body (and
/// its `locals` arena) is being lowered; signatures must be known
/// before any body, so calls resolve regardless of declaration order.
#[derive(Clone)]
pub(crate) struct FnSig {
    pub(crate) is_suspend: bool,
    /// Validated language-level operator role. It participates in override
    /// and interface matching instead of being inferred from the name.
    pub(crate) operator: Option<hir::OperatorKind>,
    pub(crate) attributes: hir::FunctionAttributes,
    /// Number of owner parameters at the front of `type_params`.
    pub(crate) owner_type_param_count: usize,
    pub(crate) type_params: Vec<hir::TypeParamDecl>,
    pub(crate) params: Vec<FnParam>,
    pub(crate) return_ty: TypeId,
}

#[derive(Clone)]
pub(crate) struct FnParam {
    pub(crate) name: ast::Ident,
    pub(crate) ty: TypeId,
}

/// How a variant was declared (spec 4.2). `hir::Variant` normalizes
/// the four surface forms into a field list, so the lowerer keeps the
/// form on the side: patterns must use the matching shape (positional
/// patterns for positional variants, field patterns for named ones;
/// constructor-style variants accept both).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum VariantStyle {
    Unit,
    Positional,
    Named,
    Constructor,
}

/// The type a member function belongs to (M6). Method `Function`s are
/// registered directly on their nominal owner and carry the
/// owner's type and modality in `Function::method`; the receiver is `params[0]`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Owner {
    Class(ClassId),
    Interface(InterfaceId),
    Struct(StructId),
    Enum(EnumId),
}

#[derive(Clone, Copy)]
enum IntrinsicTypeOwner {
    Struct(StructId),
    Class(ClassId),
}

/// A member declaration together with the complete host substitution at the
/// lookup site. Inherited generic members may have host arguments different
/// from the receiver's own arguments, so carrying this relation is mandatory.
#[derive(Clone)]
pub(crate) struct CallableCandidate {
    pub(crate) function: FunctionId,
    pub(crate) owner: CallableCandidateOwner,
    pub(crate) source: CallableCandidateSource,
}

impl CallableCandidate {
    pub(crate) fn function(function: FunctionId, owner_arguments: Vec<TypeId>) -> Self {
        Self {
            function,
            owner: CallableCandidateOwner::Function { owner_arguments },
            source: CallableCandidateSource::Direct,
        }
    }

    pub(crate) fn method(function: FunctionId, owner: hir::MethodOwnerApplication) -> Self {
        Self {
            function,
            owner: CallableCandidateOwner::Method(owner),
            source: CallableCandidateSource::Direct,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) enum CallableCandidateOwner {
    Function { owner_arguments: Vec<TypeId> },
    Method(hir::MethodOwnerApplication),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum CallableCandidateSource {
    Direct,
    Bound {
        receiver_parameter: hir::TypeParamId,
        bound: hir::InterfaceApplicationId,
        member: hir::InterfaceMethodId,
    },
}

/// Lexical permission to invoke a suspend callable. Keeping an explicit,
/// non-empty stack prevents declaration-owned initialization code from
/// accidentally inheriting permission from a surrounding suspend body.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SuspensionContext {
    Forbidden(ForbiddenSuspendContext),
    SuspendFunction,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ForbiddenSuspendContext {
    TopLevel,
    Function,
    ConstructorDelegation,
}

fn lower_type_param_decl(param: &ast::TypeParamDecl, id: hir::TypeParamId) -> hir::TypeParamDecl {
    hir::TypeParamDecl {
        id,
        name: param.name.text.clone(),
        variance: match param.variance {
            ast::Variance::Invariant => hir::Variance::Invariant,
            ast::Variance::In => hir::Variance::In,
            ast::Variance::Out => hir::Variance::Out,
        },
        // Bounds are resolved after every nominal declaration has entered the
        // type namespace. This temporary lowerer state never crosses the HIR
        // output boundary; successful lowering replaces it completely.
        bounds: hir::TypeParamBounds::Unconstrained,
        span: param.span,
    }
}

impl Owner {
    /// A human-readable host description for diagnostics
    /// ("class `C`", "struct `S`", ...).
    pub(crate) fn describe(&self, lowerer: &Lowerer) -> String {
        match *self {
            Owner::Class(id) => format!("class `{}`", lowerer.classes[id].name),
            Owner::Interface(id) => format!("interface `{}`", lowerer.interfaces[id].name),
            Owner::Struct(id) => format!("struct `{}`", lowerer.structs[id].name),
            Owner::Enum(id) => format!("enum `{}`", lowerer.enums[id].name),
        }
    }

    /// The bare host name, used to qualify method symbols
    /// (`Owner.method`).
    pub(crate) fn describe_name(&self, lowerer: &Lowerer) -> String {
        match *self {
            Owner::Class(id) => lowerer.classes[id].name.clone(),
            Owner::Interface(id) => lowerer.interfaces[id].name.clone(),
            Owner::Struct(id) => lowerer.structs[id].name.clone(),
            Owner::Enum(id) => lowerer.enums[id].name.clone(),
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum CaptureSource {
    /// A local in the immediately enclosing callable body.
    Local(hir::LocalId),
    /// A binding already supplied by the immediately enclosing closure.
    Capture(hir::BindingId),
}

#[derive(Clone)]
pub(crate) struct AvailableCapture {
    pub(crate) binding: hir::BindingId,
    pub(crate) ty: TypeId,
    pub(crate) mutable: bool,
    pub(crate) source: CaptureSource,
    pub(crate) declaration_depth: usize,
}

#[derive(Clone)]
pub(crate) struct PendingCapture {
    pub(crate) binding: hir::BindingId,
    pub(crate) name: String,
    pub(crate) ty: TypeId,
    pub(crate) first_use_span: Span,
    pub(crate) source: CaptureSource,
    pub(crate) declaration_depth: usize,
}

#[derive(Clone)]
pub(crate) struct CaptureContext {
    pub(crate) available: HashMap<String, AvailableCapture>,
    pub(crate) captures: Vec<PendingCapture>,
    pub(crate) by_binding: HashMap<hir::BindingId, usize>,
}

#[derive(Clone, Default)]
pub(crate) struct ReturnInference {
    pub(crate) value_types: Vec<TypeId>,
    pub(crate) saw_bare: bool,
}

#[derive(Clone)]
pub(crate) struct Lowerer {
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
    pub(crate) local_functions: Arena<hir::LocalFunction>,
    pub(crate) local_function_by_function: HashMap<FunctionId, hir::LocalFunctionId>,
    pub(crate) callable_references: Arena<hir::CallableReference>,
    pub(crate) bound_callable_refs: Arena<hir::BoundCallableRef>,
    pub(crate) function_coercions: Arena<hir::FunctionCoercion>,
    pub(crate) foreign_callback_registrations: Arena<hir::ForeignCallbackRegistration>,
    pub(crate) function_coercion_by_types:
        HashMap<(hir::FunctionTypeId, hir::FunctionTypeId), hir::FunctionCoercionId>,
    pub(crate) structs: Arena<StructDecl>,
    pub(crate) struct_applications: Arena<hir::StructApplication>,
    pub(crate) struct_application_by_key:
        HashMap<(StructId, Vec<TypeId>), hir::StructApplicationId>,
    pub(crate) enums: Arena<EnumDecl>,
    pub(crate) enum_applications: Arena<hir::EnumApplication>,
    pub(crate) enum_application_by_key: HashMap<(EnumId, Vec<TypeId>), hir::EnumApplicationId>,
    pub(crate) classes: Arena<ClassDecl>,
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
    pub(crate) globals_by_name: HashMap<String, hir::GlobalId>,
    pub(crate) global_files: HashMap<hir::GlobalId, usize>,
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
    /// Source-file ownership for validating compiler-known core contracts.
    pub(crate) struct_files: HashMap<StructId, usize>,
    pub(crate) enum_files: HashMap<EnumId, usize>,
    pub(crate) class_files: HashMap<ClassId, usize>,
    pub(crate) interface_files: HashMap<InterfaceId, usize>,
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
    pub(crate) constructor_params_in_scope: HashMap<String, (hir::ConstructorParamId, TypeId)>,
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
