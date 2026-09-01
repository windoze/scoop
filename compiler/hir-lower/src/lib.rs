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
//! preferred on ties). `print` / `println` are ordinary core-library
//! functions taking `Any` and dispatching `message.toString()` through
//! the synthesized `Any` members (vtable slots 0..2); the `@Intrinsic`
//! registry only backs compiler primitives and no
//! call-site special rules remain.
//!
//! M8 (milestone8 DESIGN.md 3.2): exceptions. `scoop.core` must define
//! a class `Throwable` (the root of the exception hierarchy, spec
//! 11.7); `throw` operands and catch parameter types must be subtypes
//! of it, catches are checked for shadowing in declaration order (a
//! catch covered by an earlier one is unreachable — an error in M8,
//! DESIGN.md 5.1), and the catch local scopes over its clause body.
//! The validated `Throwable` stays a lowerer-internal field (the HIR
//! `Module` is unchanged); mir-lower re-resolves the exception classes
//! by name when it rewrites the M3 trap paths.
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
mod effects;
mod expr;
mod ffi;
mod globals;
mod overload;
mod patterns;
mod scope;
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
    let export = Lowerer::new().run(files)?;
    let local = concretize::lower(&export);
    Ok(hir::Output { export, local })
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
/// registered per owner (`class_methods` and friends) and carry the
/// owner's type and modality in `Function::method`; the receiver is `params[0]`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Owner {
    Class(ClassId),
    Interface(InterfaceId),
    Struct(StructId),
    Enum(EnumId),
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

fn lower_type_param_decl(param: &ast::TypeParamDecl) -> hir::TypeParamDecl {
    hir::TypeParamDecl {
        name: param.name.text.clone(),
        variance: match param.variance {
            ast::Variance::Invariant => hir::Variance::Invariant,
            ast::Variance::In => hir::Variance::In,
            ast::Variance::Out => hir::Variance::Out,
        },
        kind: match param.kind_bound {
            None => hir::TypeParamKind::Any,
            Some(ast::TypeParamKindBound::Value) => hir::TypeParamKind::Value,
            Some(ast::TypeParamKindBound::Ref) => hir::TypeParamKind::Ref,
        },
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
    pub(crate) local_functions: Arena<hir::LocalFunction>,
    pub(crate) local_function_by_function: HashMap<FunctionId, hir::LocalFunctionId>,
    pub(crate) callable_references: Arena<hir::CallableReference>,
    pub(crate) function_coercions: Arena<hir::FunctionCoercion>,
    pub(crate) foreign_callback_registrations: Arena<hir::ForeignCallbackRegistration>,
    pub(crate) function_coercion_by_types:
        HashMap<(hir::FunctionTypeId, hir::FunctionTypeId), hir::FunctionCoercionId>,
    pub(crate) structs: Arena<StructDecl>,
    pub(crate) enums: Arena<EnumDecl>,
    pub(crate) classes: Arena<ClassDecl>,
    pub(crate) interfaces: Arena<InterfaceDecl>,
    pub(crate) functions: Arena<Function>,
    pub(crate) extern_functions: Arena<hir::ExternFunction>,
    pub(crate) globals: Arena<hir::Global>,
    /// Generic definitions are separate HIR entities. The reverse map
    /// is lowerer-only and lets call resolution turn a selected
    /// `FunctionId` into a typed generic identity.
    pub(crate) generic_functions: Arena<GenericFunction>,
    pub(crate) generic_by_function: HashMap<FunctionId, GenericFunctionId>,
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
    /// The synthesized `Any` members `equals` / `hashCode` /
    /// `toString`, in vtable-slot order (0..2, mir-lower's fixed
    /// prefix). Calls on an `Any` receiver resolve to these.
    pub(crate) any_methods: [FunctionId; 3],
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
    /// Member functions per owner, in declaration order (this is also
    /// declaration order used when building vtables.
    pub(crate) class_methods: HashMap<ClassId, Vec<FunctionId>>,
    pub(crate) interface_methods: HashMap<InterfaceId, Vec<FunctionId>>,
    pub(crate) struct_methods: HashMap<StructId, Vec<FunctionId>>,
    pub(crate) enum_methods: HashMap<EnumId, Vec<FunctionId>>,
    /// The owner of every member function.
    pub(crate) function_owner: HashMap<FunctionId, Owner>,
    /// Mutability of each class's own constructor properties
    /// (declaration order); `hir::Field` has no mutability slot.
    pub(crate) class_prop_mutability: HashMap<ClassId, Vec<bool>>,
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
    /// misconfigured (diagnosed, so the module is rejected anyway).
    /// Lowerer-internal on purpose: `hir::Module` is unchanged and
    /// mir-lower re-resolves the exception classes by name.
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
    /// Active smart-cast narrowings (milestone6 DESIGN.md 5.4):
    /// immutable local → narrowed type, valid within the branch that
    /// established them. Saved and restored around branch lowering;
    /// the declared type of a local never changes.
    pub(crate) smart_casts: HashMap<hir::LocalId, TypeId>,
    /// Index of the file currently being processed (diagnostics).
    pub(crate) current_file: usize,
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

impl Lowerer {
    pub(crate) fn fresh_binding(&mut self) -> hir::BindingId {
        let binding = hir::BindingId::from_raw(self.next_binding_id);
        self.next_binding_id += 1;
        binding
    }

    pub(crate) fn alloc_local(&mut self, name: String, ty: TypeId, mutable: bool) -> hir::LocalId {
        let binding = self.fresh_binding();
        self.locals.alloc(hir::Local {
            binding,
            name,
            ty,
            mutable,
        })
    }

    pub(crate) fn push_scope(&mut self) {
        self.scopes.push();
        self.local_function_scopes.push();
    }

    pub(crate) fn pop_scope(&mut self) {
        self.scopes.pop();
        self.local_function_scopes.pop();
    }

    fn new() -> Self {
        // Well-known types are allocated first, in a fixed order
        // (impl spec 2.2): Unit, Int, UInt (M9), Boolean, String.
        // `Any` (M6) follows them; it is not part of the `hir::Module`
        // well-known list, so hir-lower interns it once here.
        let mut types = Arena::new();
        let unit = types.alloc(Type::Unit);
        let int = types.alloc(Type::Int);
        let uint = types.alloc(Type::UInt);
        let boolean = types.alloc(Type::Boolean);
        let string = types.alloc(Type::String);
        let any = types.alloc(Type::Any);

        let mut lowerer = Lowerer {
            types,
            function_types: Arena::new(),
            lambdas: Arena::new(),
            anonymous_functions: Arena::new(),
            next_lambda_function: 0,
            next_anonymous_function: 0,
            local_functions: Arena::new(),
            local_function_by_function: HashMap::new(),
            callable_references: Arena::new(),
            function_coercions: Arena::new(),
            foreign_callback_registrations: Arena::new(),
            function_coercion_by_types: HashMap::new(),
            structs: Arena::new(),
            enums: Arena::new(),
            classes: Arena::new(),
            interfaces: Arena::new(),
            functions: Arena::new(),
            extern_functions: Arena::new(),
            globals: Arena::new(),
            generic_functions: Arena::new(),
            generic_by_function: HashMap::new(),
            top_level: Vec::new(),
            unit,
            int,
            uint,
            boolean,
            string,
            any,
            // Filled by `synthesize_any_members` below.
            any_methods: [hir::FunctionId::from_raw(0.into()); 3],
            functions_by_name: HashMap::new(),
            extensions_by_name: HashMap::new(),
            extension_receivers: HashMap::new(),
            function_files: HashMap::new(),
            globals_by_name: HashMap::new(),
            global_files: HashMap::new(),
            user_file_index: 0,
            structs_by_name: HashMap::new(),
            enums_by_name: HashMap::new(),
            classes_by_name: HashMap::new(),
            interfaces_by_name: HashMap::new(),
            struct_files: HashMap::new(),
            enum_files: HashMap::new(),
            class_files: HashMap::new(),
            interface_files: HashMap::new(),
            ffi_ptr: None,
            ffi_fun_ptr: None,
            ffi_pinned_ptr: None,
            ffi_gc_handle: None,
            ffi_foreign_callback: None,
            ffi_core: None,
            foreign_callback_core: None,
            allow_deferred_fun_ptr: false,
            pointer_type_uses: Vec::new(),
            fun_ptr_type_uses: Vec::new(),
            class_methods: HashMap::new(),
            interface_methods: HashMap::new(),
            struct_methods: HashMap::new(),
            enum_methods: HashMap::new(),
            function_owner: HashMap::new(),
            class_prop_mutability: HashMap::new(),
            option_candidates: Vec::new(),
            option_enum: None,
            throwable_candidates: Vec::new(),
            throwable: None,
            variant_styles: HashMap::new(),
            signatures: HashMap::new(),
            type_params_in_scope: Vec::new(),
            current_return_ty: unit,
            return_inference: None,
            current_fn_name: String::new(),
            suspension_contexts: vec![SuspensionContext::Forbidden(
                ForbiddenSuspendContext::TopLevel,
            )],
            safety_contexts: vec![hir::Safety::Safe],
            current_this: None,
            current_owner: None,
            smart_casts: HashMap::new(),
            current_file: 0,
            locals: Arena::new(),
            scopes: Scopes::new(),
            local_function_scopes: LocalFunctionScopes::new(),
            capture_contexts: Vec::new(),
            next_binding_id: 0,
            instantiations: Arena::new(),
            hidden_count: 0,
            diagnostics: Vec::new(),
        };
        lowerer.synthesize_any_members();
        lowerer
    }

    /// The three `Any` members — `equals(other: Any): Boolean`,
    /// `hashCode(): Int`, `toString(): String` — synthesized as
    /// bodyless members (parameter-only bodies, like interface
    /// methods): mir-lower dispatches calls on an `Any` receiver
    /// virtually through the fixed vtable prefix (slots 0..2, in this
    /// order), so their bodies never execute. They are not in
    /// `top_level` (they are members, not user declarations) and
    /// therefore never appear in HIR dumps.
    fn synthesize_any_members(&mut self) {
        let span = Span::new(0, 0);
        let mut methods = Vec::with_capacity(3);
        for (short, params, return_ty) in [
            ("equals", vec![("other", self.any)], self.boolean),
            ("hashCode", Vec::new(), self.int),
            ("toString", Vec::new(), self.string),
        ] {
            let mut locals = Arena::new();
            let this_binding = self.fresh_binding();
            let this = locals.alloc(hir::Local {
                binding: this_binding,
                name: "this".to_string(),
                ty: self.any,
                mutable: false,
            });
            let mut fn_params = vec![hir::Param {
                name: "this".to_string(),
                ty: self.any,
                local: this,
            }];
            let mut sig_params = Vec::new();
            for (param_name, param_ty) in params {
                let binding = self.fresh_binding();
                let local = locals.alloc(hir::Local {
                    binding,
                    name: param_name.to_string(),
                    ty: param_ty,
                    mutable: false,
                });
                fn_params.push(hir::Param {
                    name: param_name.to_string(),
                    ty: param_ty,
                    local,
                });
                sig_params.push(FnParam {
                    name: ast::Ident {
                        text: param_name.to_string(),
                        span,
                    },
                    ty: param_ty,
                });
            }
            let id = self.functions.alloc(Function {
                name: format!("Any.{short}"),
                is_suspend: false,
                type_params: Vec::new(),
                params: fn_params,
                return_ty,
                attributes: hir::FunctionAttributes::default(),
                kind: FunctionKind::User(hir::Body {
                    locals,
                    statements: Vec::new(),
                }),
                method: Some(hir::Method {
                    owner: self.any,
                    modifier: hir::MethodModifier::Open,
                    owner_type_param_count: 0,
                }),
                span,
            });
            self.signatures.insert(
                id,
                FnSig {
                    is_suspend: false,
                    attributes: hir::FunctionAttributes::default(),
                    owner_type_param_count: 0,
                    type_params: Vec::new(),
                    params: sig_params,
                    return_ty,
                },
            );
            methods.push(id);
        }
        self.any_methods = [methods[0], methods[1], methods[2]];
    }

    fn run(mut self, files: &[ast::SourceFile]) -> Result<hir::Module, Vec<Diagnostic>> {
        if files.is_empty() {
            return Err(vec![Diagnostic {
                file: 0,
                span: None,
                message: "no source files to compile".to_string(),
            }]);
        }
        let user_file_index = files.len() - 1;
        self.user_file_index = user_file_index;

        // Pass 1: declare structs, enums, classes, interfaces and
        // functions across all files (core first), so bodies and field
        // types resolve regardless of declaration order. Structs,
        // enums, classes and interfaces share the *type* namespace and
        // must not collide; functions occupy a separate namespace where
        // one name may collect several overloads (M7), and member
        // functions live in per-owner namespaces.
        let mut pending_structs = Vec::new();
        let mut pending_enums = Vec::new();
        let mut pending_classes = Vec::new();
        let mut pending_functions = Vec::new();
        let mut pending_globals = Vec::new();
        let mut pending_methods: Vec<(FunctionId, &ast::FunctionDecl, usize, Owner)> = Vec::new();
        for (file_index, file) in files.iter().enumerate() {
            self.current_file = file_index;
            let is_core = file_index < user_file_index;
            for decl in &file.declarations {
                match decl {
                    ast::Decl::Global(decl) => pending_globals.push((decl, file_index)),
                    ast::Decl::Struct(decl) => self.declare_struct(
                        decl,
                        &mut pending_structs,
                        &mut pending_methods,
                        file_index,
                    ),
                    ast::Decl::Enum(decl) => self.declare_enum(
                        decl,
                        is_core,
                        &mut pending_enums,
                        &mut pending_methods,
                        file_index,
                    ),
                    ast::Decl::Class(decl) => self.declare_class(
                        decl,
                        is_core,
                        &mut pending_classes,
                        &mut pending_methods,
                        file_index,
                    ),
                    ast::Decl::Interface(decl) => {
                        self.declare_interface(decl, &mut pending_methods, file_index)
                    }
                    ast::Decl::Function(decl) => {
                        self.declare_function(decl, is_core, &mut pending_functions, file_index)
                    }
                }
            }
        }

        self.ffi_ptr = self.require_core_struct("Ptr", files);
        self.ffi_fun_ptr = self.require_core_struct("FunPtr", files);
        self.ffi_pinned_ptr = self.require_core_struct("PinnedPtr", files);
        self.ffi_gc_handle = self.require_core_struct("GcHandle", files);
        self.ffi_foreign_callback = self.require_core_struct("ForeignCallback", files);

        // The core library's `Option<T>` must be validated before any
        // type annotation is resolved: `T?` desugars to it (spec 7.1).
        self.validate_option_enum(files);
        // The core library's `Throwable` is the root every `throw`
        // operand and catch parameter type is checked against (spec
        // 11.7).
        self.validate_throwable(files);

        // Pass 2: resolve struct fields, enum variants, class
        // constructor properties and inheritance clauses (all type
        // names are known now, so fields may reference later-declared
        // types).
        for &(id, decl, file_index) in &pending_structs {
            self.current_file = file_index;
            self.allow_deferred_fun_ptr = Some(id) == self.ffi_foreign_callback;
            self.resolve_fields(id, decl);
            self.allow_deferred_fun_ptr = false;
            self.type_params_in_scope = self.structs[id].type_params.clone();
            let interfaces = self.resolve_interface_list(&decl.interfaces);
            self.type_params_in_scope.clear();
            self.structs[id].interfaces = interfaces;
        }
        for &(id, decl, file_index) in &pending_enums {
            self.current_file = file_index;
            self.resolve_variants(id, decl);
            self.type_params_in_scope = self.enums[id].type_params.clone();
            let interfaces = self.resolve_interface_list(&decl.interfaces);
            self.type_params_in_scope.clear();
            self.enums[id].interfaces = interfaces;
        }
        for (id, decl, file_index) in &pending_classes {
            self.current_file = *file_index;
            self.resolve_class(*id, decl);
        }

        // Pass 2.5: resolve function and method signatures, so calls
        // in any body see parameter and return types regardless of
        // declaration order. Interface method signatures become
        // `hir::MethodSig`s; bodyless declarations (interface and
        // abstract methods) get their parameter-only body here.
        for &(id, decl, file_index) in &pending_functions {
            self.current_file = file_index;
            self.resolve_signature(id, decl);
        }
        for &(id, decl, file_index, owner) in &pending_methods {
            self.current_file = file_index;
            self.resolve_method_signature(id, decl, owner);
        }

        // Declaration-site variance is a property of the fully resolved
        // interface signatures, so validate it after every signature exists.
        self.check_interface_variance();

        // M10's coroutine protocol is compiler-known: MIR generation needs
        // these exact generic interfaces and intrinsic signatures rather than
        // guessing entities from names after HIR.
        let coroutine_core = self.validate_coroutine_core(files);
        let ffi_core = self.validate_ffi_core(files);
        self.ffi_core = ffi_core;
        let foreign_callback_core = self.validate_foreign_callback_core(files);
        self.foreign_callback_core = foreign_callback_core;
        self.validate_pointer_type_uses();
        self.resolve_globals(&pending_globals);
        self.validate_extern_functions();
        self.validate_extern_global_symbols();

        // Pass 2.6: overload declarations must be distinguishable —
        // within one name (top-level) or one host (members) no two
        // functions may share a signature (milestone7 DESIGN.md 1.1).
        self.check_duplicate_signatures(&pending_functions, &pending_methods);

        // Pass 2.75: inheritance checks (milestone6 DESIGN.md 2.2) —
        // cycles, property shadowing, override rules and interface
        // implementation (classes and value types alike). Needs every
        // signature and inheritance clause.
        self.check_inheritance(
            &pending_classes,
            &pending_structs,
            &pending_enums,
            &pending_methods,
        );

        // Pass 3: lower bodies. Intrinsics have no body to lower (the
        // parser guarantees it is omitted); their `kind` was set at
        // declaration time. Base-constructor delegation arguments are
        // lowered in an empty scope (constructor properties are not in
        // scope there, an M6 simplification: HIR has no body to host
        // their locals).
        for &(id, decl, file_index) in &pending_classes {
            self.current_file = file_index;
            self.lower_base_args(id, decl);
        }
        for (id, decl, file_index) in pending_functions {
            if matches!(
                self.functions[id].kind,
                FunctionKind::Intrinsic(_) | FunctionKind::Extern(_)
            ) {
                continue;
            }
            self.current_file = file_index;
            let body = self.lower_body(id, decl);
            self.functions[id].kind = FunctionKind::User(body);
        }
        for (id, decl, file_index, owner) in pending_methods {
            // Interface and abstract methods are bodyless; their
            // parameter-only body was built in pass 2.5.
            if matches!(self.functions[id].kind, FunctionKind::Intrinsic(_))
                || decl.modifier == ast::MethodModifier::Abstract
                || matches!(owner, Owner::Interface(_))
            {
                continue;
            }
            self.current_file = file_index;
            let body = self.lower_body(id, decl);
            self.functions[id].kind = FunctionKind::User(body);
        }

        // Effects consume fully resolved calls and types. Local functions and
        // callable literals lifted while lowering the bodies are visible now.
        self.validate_c_ffi_types();
        self.check_no_gc_types();
        self.check_no_gc_functions();

        // A module without `main` never reaches HIR (hir docs); it is a
        // diagnostic here, attributed to the user file. With overloads
        // (M7) several functions may be named `main`; the entry point
        // is the zero-parameter one.
        self.current_file = user_file_index;
        let zero_param_main = self.functions_by_name.get("main").and_then(|ids| {
            ids.iter().copied().find(|&id| {
                self.signatures
                    .get(&id)
                    .is_some_and(|sig| sig.params.is_empty())
            })
        });
        let entry = match zero_param_main {
            Some(id) => {
                // The entry point is monomorphic: there is no caller to
                // infer type arguments from.
                if !self.functions[id].type_params.is_empty() {
                    self.error(
                        self.functions[id].span,
                        "`main` must not be generic".to_string(),
                    );
                }
                if self.functions[id].is_suspend {
                    self.error(
                        self.functions[id].span,
                        "`main` must not be suspend".to_string(),
                    );
                }
                if matches!(self.functions[id].kind, FunctionKind::Extern(_)) {
                    self.error(
                        self.functions[id].span,
                        "`main` must be a Scoop-defined function".to_string(),
                    );
                }
                Some(id)
            }
            None => {
                self.error(
                    files[user_file_index].span,
                    "missing entry point: declare `fun main()`".to_string(),
                );
                None
            }
        };

        if !self.diagnostics.is_empty() {
            return Err(self.diagnostics);
        }
        // Invariant: empty diagnostics implies `main` was found and the
        // core `Option<T>` validated above.
        let entry = entry.expect("missing `main` is always diagnosed");
        let option_enum = self
            .option_enum
            .expect("a missing or invalid core `Option` is always diagnosed");
        let coroutine_core = coroutine_core
            .expect("a missing or invalid coroutine core protocol is always diagnosed");
        let ffi_core =
            ffi_core.expect("a missing or invalid FFI core protocol is always diagnosed");
        Ok(hir::Module {
            types: self.types,
            function_types: self.function_types,
            lambdas: self.lambdas,
            anonymous_functions: self.anonymous_functions,
            local_functions: self.local_functions,
            callable_references: self.callable_references,
            function_coercions: self.function_coercions,
            foreign_callback_registrations: self.foreign_callback_registrations,
            functions: self.functions,
            extern_functions: self.extern_functions,
            globals: self.globals,
            generic_functions: self.generic_functions,
            structs: self.structs,
            enums: self.enums,
            classes: self.classes,
            interfaces: self.interfaces,
            top_level: self.top_level,
            unit: self.unit,
            int: self.int,
            boolean: self.boolean,
            string: self.string,
            option_enum,
            coroutine_core,
            ffi_core,
            foreign_callback_core: foreign_callback_core
                .expect("a missing or invalid foreign callback core protocol is always diagnosed"),
            entry,
            instantiations: self.instantiations,
        })
    }

    fn require_core_struct(&mut self, name: &str, files: &[ast::SourceFile]) -> Option<StructId> {
        let candidate = self.structs_by_name.get(name).map(|(id, _)| *id);
        if let Some(id) = candidate
            && self
                .struct_files
                .get(&id)
                .copied()
                .unwrap_or(self.user_file_index)
                < self.user_file_index
        {
            return Some(id);
        }
        self.current_file = candidate
            .and_then(|id| self.struct_files.get(&id).copied())
            .unwrap_or(0);
        self.error(
            files[0].span,
            format!("scoop.core must define exactly one `{name}` struct"),
        );
        None
    }

    /// Whether a type name is already taken in the shared type
    /// namespace (structs, enums, classes, interfaces). Returns the
    /// kind of the existing declaration for diagnostics.
    fn type_namespace_conflict(&self, name: &str) -> Option<&'static str> {
        if self.structs_by_name.contains_key(name) {
            Some("a struct")
        } else if self.enums_by_name.contains_key(name) {
            Some("an enum")
        } else if self.classes_by_name.contains_key(name) {
            Some("a class")
        } else if self.interfaces_by_name.contains_key(name) {
            Some("an interface")
        } else {
            None
        }
    }

    fn declare_struct<'a>(
        &mut self,
        decl: &'a ast::StructDecl,
        pending: &mut Vec<(StructId, &'a ast::StructDecl, usize)>,
        pending_methods: &mut Vec<(FunctionId, &'a ast::FunctionDecl, usize, Owner)>,
        file_index: usize,
    ) {
        let attributes = self.check_struct_annotations(decl);
        if let Some(kind) = self.type_namespace_conflict(&decl.name.text) {
            let what = if kind == "a struct" {
                format!("duplicate struct `{}`", decl.name.text)
            } else {
                format!(
                    "duplicate type `{}` (already declared as {kind})",
                    decl.name.text
                )
            };
            self.error(decl.name.span, what);
            return;
        }
        let mut type_params = Vec::new();
        for param in &decl.type_params {
            if type_params
                .iter()
                .any(|existing: &hir::TypeParamDecl| existing.name == param.name.text)
            {
                self.error(
                    param.span,
                    format!("duplicate type parameter `{}`", param.name.text),
                );
                continue;
            }
            type_params.push(lower_type_param_decl(param));
        }
        let id = self.structs.alloc(StructDecl {
            name: decl.name.text.clone(),
            type_params: type_params.clone(),
            attributes,
            fields: Vec::new(),
            // Filled in pass 2 together with the fields.
            interfaces: Vec::new(),
            span: decl.span,
        });
        let type_args = (0..type_params.len())
            .map(|index| self.intern_type(Type::Param(hir::TypeParamId::from_raw(index as u32))))
            .collect();
        let ty = self.intern_type(Type::Struct(id, type_args));
        self.structs_by_name
            .insert(decl.name.text.clone(), (id, ty));
        self.struct_files.insert(id, file_index);
        self.struct_methods.insert(id, Vec::new());
        for method in &decl.methods {
            self.declare_method(method, Owner::Struct(id), pending_methods, file_index);
        }
        pending.push((id, decl, file_index));
    }

    fn declare_enum<'a>(
        &mut self,
        decl: &'a ast::EnumDecl,
        is_core: bool,
        pending: &mut Vec<(EnumId, &'a ast::EnumDecl, usize)>,
        pending_methods: &mut Vec<(FunctionId, &'a ast::FunctionDecl, usize, Owner)>,
        file_index: usize,
    ) {
        let no_gc = self.check_enum_annotations(decl);
        if let Some(kind) = self.type_namespace_conflict(&decl.name.text) {
            let what = if kind == "an enum" {
                format!("duplicate enum `{}`", decl.name.text)
            } else {
                format!(
                    "duplicate type `{}` (already declared as {kind})",
                    decl.name.text
                )
            };
            self.error(decl.name.span, what);
            return;
        }
        let mut type_params = Vec::new();
        for param in &decl.type_params {
            if type_params
                .iter()
                .any(|existing: &hir::TypeParamDecl| existing.name == param.name.text)
            {
                self.error(
                    param.span,
                    format!("duplicate type parameter `{}`", param.name.text),
                );
                continue;
            }
            type_params.push(lower_type_param_decl(param));
        }
        let id = self.enums.alloc(EnumDecl {
            name: decl.name.text.clone(),
            type_params,
            no_gc,
            // Filled in pass 2; a resolution failure is diagnosed, so
            // empty variants never reach the output.
            variants: Vec::new(),
            interfaces: Vec::new(),
            span: decl.span,
        });
        self.enums_by_name.insert(decl.name.text.clone(), id);
        self.enum_files.insert(id, file_index);
        self.enum_methods.insert(id, Vec::new());
        if is_core && decl.name.text == "Option" {
            self.option_candidates
                .push((id, file_index, decl.span, decl.type_params.len()));
        }
        for method in &decl.methods {
            self.declare_method(method, Owner::Enum(id), pending_methods, file_index);
        }
        pending.push((id, decl, file_index));
    }

    fn declare_class<'a>(
        &mut self,
        decl: &'a ast::ClassDecl,
        is_core: bool,
        pending: &mut Vec<(ClassId, &'a ast::ClassDecl, usize)>,
        pending_methods: &mut Vec<(FunctionId, &'a ast::FunctionDecl, usize, Owner)>,
        file_index: usize,
    ) {
        self.reject_type_annotations("a class", &decl.annotations);
        if let Some(kind) = self.type_namespace_conflict(&decl.name.text) {
            let what = if kind == "a class" {
                format!("duplicate class `{}`", decl.name.text)
            } else {
                format!(
                    "duplicate type `{}` (already declared as {kind})",
                    decl.name.text
                )
            };
            self.error(decl.name.span, what);
            return;
        }
        let modifier = match decl.modifier {
            ast::ClassModifier::Final => hir::ClassModifier::Final,
            ast::ClassModifier::Open => hir::ClassModifier::Open,
            ast::ClassModifier::Abstract => hir::ClassModifier::Abstract,
        };
        let id = self.classes.alloc(ClassDecl {
            modifier,
            name: decl.name.text.clone(),
            // Filled in pass 2; resolution failures are diagnosed, so
            // these never reach the output unfinished.
            constructor: Vec::new(),
            base_class: None,
            interfaces: Vec::new(),
            span: decl.span,
        });
        let ty = self.types.alloc(Type::Class(id));
        self.classes_by_name
            .insert(decl.name.text.clone(), (id, ty));
        self.class_files.insert(id, file_index);
        self.class_methods.insert(id, Vec::new());
        if is_core && decl.name.text == "Throwable" {
            self.throwable_candidates.push((id, ty));
        }
        for method in &decl.methods {
            self.declare_method(method, Owner::Class(id), pending_methods, file_index);
        }
        pending.push((id, decl, file_index));
    }

    fn declare_interface<'a>(
        &mut self,
        decl: &'a ast::InterfaceDecl,
        pending_methods: &mut Vec<(FunctionId, &'a ast::FunctionDecl, usize, Owner)>,
        file_index: usize,
    ) {
        self.reject_type_annotations("an interface", &decl.annotations);
        if let Some(kind) = self.type_namespace_conflict(&decl.name.text) {
            let what = if kind == "an interface" {
                format!("duplicate interface `{}`", decl.name.text)
            } else {
                format!(
                    "duplicate type `{}` (already declared as {kind})",
                    decl.name.text
                )
            };
            self.error(decl.name.span, what);
            return;
        }
        let mut type_params = Vec::new();
        for param in &decl.type_params {
            if type_params
                .iter()
                .any(|existing: &hir::TypeParamDecl| existing.name == param.name.text)
            {
                self.error(
                    param.span,
                    format!("duplicate type parameter `{}`", param.name.text),
                );
                continue;
            }
            type_params.push(lower_type_param_decl(param));
        }
        let id = self.interfaces.alloc(InterfaceDecl {
            name: decl.name.text.clone(),
            type_params: type_params.clone(),
            // Filled in pass 2.5 together with the method signatures.
            methods: Vec::new(),
            span: decl.span,
        });
        let type_args = (0..type_params.len())
            .map(|index| self.intern_type(Type::Param(hir::TypeParamId::from_raw(index as u32))))
            .collect();
        let ty = self.intern_type(Type::Interface(id, type_args));
        self.interfaces_by_name
            .insert(decl.name.text.clone(), (id, ty));
        self.interface_files.insert(id, file_index);
        self.interface_methods.insert(id, Vec::new());
        for method in &decl.methods {
            self.declare_method(method, Owner::Interface(id), pending_methods, file_index);
        }
    }

    /// Declare a member function (pass 1): methods live in per-owner
    /// namespaces where one name may collect several overloads (M7;
    /// same-signature duplicates are diagnosed in pass 2.6, once
    /// parameter types are known) and are named `Owner.method` for
    /// unambiguous symbols downstream; `Function::method` records the
    /// host type and effective modality. Signatures and bodies are filled in passes
    /// 2.5 / 3.
    fn declare_method<'a>(
        &mut self,
        decl: &'a ast::FunctionDecl,
        owner: Owner,
        pending: &mut Vec<(FunctionId, &'a ast::FunctionDecl, usize, Owner)>,
        file_index: usize,
    ) {
        let checked = self.check_function_annotations(
            decl,
            file_index < self.user_file_index,
            FunctionTarget::Member(owner),
        );
        let host_ty = self.owner_ty(owner);
        let modifier = match owner {
            Owner::Interface(_) => hir::MethodModifier::Abstract,
            Owner::Struct(_) | Owner::Enum(_) => hir::MethodModifier::Final,
            Owner::Class(class_id)
                if self.classes[class_id].modifier == hir::ClassModifier::Final
                    && decl.is_override
                    && decl.modifier == ast::MethodModifier::Open =>
            {
                // An override is open by default, but a final owner
                // makes it effectively final.
                hir::MethodModifier::Final
            }
            Owner::Class(_) => match decl.modifier {
                ast::MethodModifier::Final => hir::MethodModifier::Final,
                ast::MethodModifier::Open => hir::MethodModifier::Open,
                ast::MethodModifier::Abstract => hir::MethodModifier::Abstract,
            },
        };
        let owner_type_param_count = self.owner_type_params(owner).len() as u32;
        let kind = match checked.intrinsic {
            Some(intrinsic) => FunctionKind::Intrinsic(intrinsic),
            None => FunctionKind::User(hir::Body {
                locals: Arena::new(),
                statements: Vec::new(),
            }),
        };
        let id = self.functions.alloc(Function {
            name: format!("{}.{}", owner.describe_name(self), decl.name.text),
            is_suspend: decl.is_suspend,
            // Filled in pass 2.5 (signature) and pass 3 (body and
            // parameter locals).
            type_params: Vec::new(),
            params: Vec::new(),
            return_ty: self.unit,
            attributes: checked.attributes,
            kind,
            method: Some(hir::Method {
                owner: host_ty,
                modifier,
                owner_type_param_count,
            }),
            span: decl.span,
        });
        self.function_owner.insert(id, owner);
        self.function_files.insert(id, file_index);
        match owner {
            Owner::Class(id) => self.class_methods.get_mut(&id),
            Owner::Interface(id) => self.interface_methods.get_mut(&id),
            Owner::Struct(id) => self.struct_methods.get_mut(&id),
            Owner::Enum(id) => self.enum_methods.get_mut(&id),
        }
        .expect("the owner map was initialized above")
        .push(id);
        pending.push((id, decl, file_index, owner));
    }

    /// Declare a top-level function (pass 1). One name may collect
    /// several overloads (M7); same-signature duplicates are diagnosed
    /// in pass 2.6, once parameter types are known.
    fn declare_function<'a>(
        &mut self,
        decl: &'a ast::FunctionDecl,
        is_core: bool,
        pending: &mut Vec<(FunctionId, &'a ast::FunctionDecl, usize)>,
        file_index: usize,
    ) {
        let checked = self.check_function_annotations(decl, is_core, FunctionTarget::TopLevel);
        let kind = match (checked.intrinsic, checked.extern_) {
            (Some(intrinsic), _) => FunctionKind::Intrinsic(intrinsic),
            (None, Some(extern_)) => {
                let id = self.extern_functions.alloc(hir::ExternFunction {
                    source_name: decl.name.text.clone(),
                    native_symbol: extern_.native_symbol,
                    library: extern_.library,
                    abi: extern_.abi,
                    calling_convention: checked.attributes.calling_convention,
                    gc_effect: checked.attributes.gc_effect,
                    safety: checked.attributes.safety,
                    params: Vec::new(),
                    return_type: self.unit,
                });
                FunctionKind::Extern(id)
            }
            (None, None) => FunctionKind::User(hir::Body {
                locals: Arena::new(),
                statements: Vec::new(),
            }),
        };
        let id = self.functions.alloc(Function {
            name: decl.name.text.clone(),
            is_suspend: decl.is_suspend,
            // Filled in pass 2.5 (signature) and pass 3 (parameter
            // locals); a resolution failure is diagnosed, so these
            // never reach the output.
            type_params: Vec::new(),
            params: Vec::new(),
            return_ty: self.unit,
            attributes: checked.attributes,
            kind,
            method: None,
            span: decl.span,
        });
        self.top_level.push(id);
        let namespace = if decl.receiver_ty.is_some() {
            &mut self.extensions_by_name
        } else {
            &mut self.functions_by_name
        };
        namespace
            .entry(decl.name.text.clone())
            .or_default()
            .push(id);
        self.function_files.insert(id, file_index);
        pending.push((id, decl, file_index));
    }

    fn validate_coroutine_core(&mut self, files: &[ast::SourceFile]) -> Option<hir::CoroutineCore> {
        let continuation = self.require_core_interface("Continuation", files);
        let suspend_task = self.require_core_interface("SuspendTask", files);
        let suspend_registration = self.require_core_interface("SuspendRegistration", files);
        let throwable = self.throwable.map(|(id, _)| id);

        if let Some(id) = continuation {
            self.validate_continuation_contract(id);
        }
        if let Some(id) = suspend_task {
            self.validate_suspend_task_contract(id);
        }
        if let (Some(id), Some(continuation)) = (suspend_registration, continuation) {
            self.validate_suspend_registration_contract(id, continuation);
        }
        let illegal_state_exception = self.validate_illegal_state_exception(files);
        let start_coroutine = self.require_intrinsic("coroutine_start", files);
        let suspend_coroutine = self.require_intrinsic("coroutine_suspend", files);

        if let (Some(id), Some(continuation), Some(suspend_task)) =
            (start_coroutine, continuation, suspend_task)
        {
            self.validate_coroutine_start(id, continuation, suspend_task);
        }
        if let (Some(id), Some(suspend_registration)) = (suspend_coroutine, suspend_registration) {
            self.validate_coroutine_suspend(id, suspend_registration);
        }

        let continuation_methods = continuation
            .and_then(|id| self.interface_methods.get(&id))
            .and_then(|methods| match methods.as_slice() {
                [resume, resume_with_exception] => Some((*resume, *resume_with_exception)),
                _ => None,
            });
        let suspend_task_run = suspend_task
            .and_then(|id| self.interface_methods.get(&id))
            .and_then(|methods| matches!(methods.as_slice(), [_]).then_some(methods[0]));
        let suspend_registration_register = suspend_registration
            .and_then(|id| self.interface_methods.get(&id))
            .and_then(|methods| matches!(methods.as_slice(), [_]).then_some(methods[0]));
        let (
            Some(throwable),
            Some(illegal_state_exception),
            Some(continuation),
            Some((continuation_resume, continuation_resume_with_exception)),
            Some(suspend_task),
            Some(suspend_task_run),
            Some(suspend_registration),
            Some(suspend_registration_register),
            Some(start_coroutine),
            Some(suspend_coroutine),
        ) = (
            throwable,
            illegal_state_exception,
            continuation,
            continuation_methods,
            suspend_task,
            suspend_task_run,
            suspend_registration,
            suspend_registration_register,
            start_coroutine,
            suspend_coroutine,
        )
        else {
            return None;
        };
        Some(hir::CoroutineCore {
            throwable,
            illegal_state_exception,
            continuation,
            continuation_resume,
            continuation_resume_with_exception,
            suspend_task,
            suspend_task_run,
            suspend_registration,
            suspend_registration_register,
            start_coroutine,
            suspend_coroutine,
        })
    }

    fn validate_ffi_core(&mut self, files: &[ast::SourceFile]) -> Option<hir::FfiCore> {
        let ptr = self.ffi_ptr;
        let fun_ptr = self.ffi_fun_ptr;
        let pinned_ptr = self.ffi_pinned_ptr;
        let gc_handle = self.ffi_gc_handle;
        if let Some(id) = ptr {
            self.validate_ptr_struct(id);
        }
        if let Some(id) = fun_ptr {
            self.validate_fun_ptr_struct(id);
        }
        if let Some(id) = pinned_ptr {
            self.validate_ffi_handle_struct(id, "PinnedPtr");
        }
        if let Some(id) = gc_handle {
            self.validate_ffi_handle_struct(id, "GcHandle");
        }

        let ptr_to_uint = self.require_intrinsic("ptr_to_uint", files);
        let ptr_cast = self.require_intrinsic("ptr_cast", files);
        let ptr_load = self.require_intrinsic("ptr_load", files);
        let ptr_load_offset = self.require_intrinsic("ptr_load_offset", files);
        let ptr_store = self.require_intrinsic("ptr_store", files);
        let ptr_store_offset = self.require_intrinsic("ptr_store_offset", files);
        let ptr_plus = self.require_intrinsic("ptr_plus", files);
        let ptr_minus = self.require_intrinsic("ptr_minus", files);
        let address_of = self.require_intrinsic("address_of", files);
        let size_of = self.require_intrinsic("size_of", files);
        let align_of = self.require_intrinsic("align_of", files);
        let gc_pin_raw = self.require_intrinsic("gc_pin_raw", files);
        let gc_unpin_raw = self.require_intrinsic("gc_unpin_raw", files);
        let gc_get_handle_raw = self.require_intrinsic("gc_get_handle_raw", files);
        let gc_release_handle_raw = self.require_intrinsic("gc_release_handle_raw", files);

        for (id, kind) in [
            (gc_pin_raw, GcIntrinsic::Pin),
            (gc_unpin_raw, GcIntrinsic::Unpin),
            (gc_get_handle_raw, GcIntrinsic::GetHandle),
            (gc_release_handle_raw, GcIntrinsic::ReleaseHandle),
        ] {
            if let Some(id) = id {
                self.validate_gc_intrinsic(id, kind);
            }
        }

        if let Some(ptr) = ptr {
            for (id, kind) in [
                (ptr_to_uint, hir::PointerIntrinsic::ToUInt),
                (ptr_cast, hir::PointerIntrinsic::Cast),
                (ptr_load, hir::PointerIntrinsic::Load),
                (ptr_load_offset, hir::PointerIntrinsic::LoadOffset),
                (ptr_store, hir::PointerIntrinsic::Store),
                (ptr_store_offset, hir::PointerIntrinsic::StoreOffset),
                (ptr_plus, hir::PointerIntrinsic::Plus),
                (ptr_minus, hir::PointerIntrinsic::Minus),
            ] {
                if let Some(id) = id {
                    self.validate_ptr_method_intrinsic(id, ptr, kind);
                }
            }
        }
        if let Some(id) = address_of {
            self.validate_pointer_top_level_intrinsic(id, hir::PointerIntrinsic::AddressOf);
        }
        if let Some(id) = size_of {
            self.validate_pointer_top_level_intrinsic(id, hir::PointerIntrinsic::SizeOf);
        }
        if let Some(id) = align_of {
            self.validate_pointer_top_level_intrinsic(id, hir::PointerIntrinsic::AlignOf);
        }

        Some(hir::FfiCore {
            ptr: ptr?,
            fun_ptr: fun_ptr?,
            pinned_ptr: pinned_ptr?,
            gc_handle: gc_handle?,
            ptr_to_uint: ptr_to_uint?,
            ptr_cast: ptr_cast?,
            ptr_load: ptr_load?,
            ptr_load_offset: ptr_load_offset?,
            ptr_store: ptr_store?,
            ptr_store_offset: ptr_store_offset?,
            ptr_plus: ptr_plus?,
            ptr_minus: ptr_minus?,
            address_of: address_of?,
            size_of: size_of?,
            align_of: align_of?,
            gc_pin_raw: gc_pin_raw?,
            gc_unpin_raw: gc_unpin_raw?,
            gc_get_handle_raw: gc_get_handle_raw?,
            gc_release_handle_raw: gc_release_handle_raw?,
        })
    }

    fn validate_foreign_callback_core(
        &mut self,
        files: &[ast::SourceFile],
    ) -> Option<hir::ForeignCallbackCore> {
        // The exception-core validator owns the missing-Throwable diagnostic.
        // Callback failure typing cannot be validated until that prerequisite
        // exists, so do not manufacture a second error or unwrap incomplete
        // upstream state here.
        let (_, throwable) = self.throwable?;
        let callback = self.ffi_foreign_callback?;
        let mode = self.require_core_enum("ForeignCallbackMode", files)?;
        let state = self.require_core_enum("ForeignCallbackState", files)?;
        let register = self.require_intrinsic("foreign_callback_register", files);
        let retain = self.require_intrinsic("foreign_callback_retain", files);
        let release = self.require_intrinsic("foreign_callback_release", files);
        let query_state = self.require_intrinsic("foreign_callback_state", files);
        let failure = self.require_intrinsic("foreign_callback_failure", files);

        self.current_file = self.struct_files[&callback];
        let callback_decl = &self.structs[callback];
        let callback_valid = callback_decl.type_params.len() == 1
            && callback_decl.type_params[0].kind == hir::TypeParamKind::Any
            && callback_decl.interfaces.is_empty()
            && callback_decl.attributes.c_layout.is_none()
            && !callback_decl.attributes.interior_mutable
            && matches!(callback_decl.fields.as_slice(), [function, context]
                if function.name == "function"
                    && matches!(&self.types[function.ty], hir::Type::Struct(id, args)
                        if *id == self.ffi_fun_ptr.expect("FunPtr core exists")
                            && matches!(args.as_slice(), [arg] if self.is_type_param(*arg, 0)))
                    && context.name == "context"
                    && matches!(self.types[context.ty], hir::Type::Ptr(pointee) if pointee == self.unit));
        if !callback_valid {
            self.error(
                callback_decl.span,
                "core `ForeignCallback<F>` must contain `function: FunPtr<F>` and `context: Ptr<Unit>`"
                    .to_string(),
            );
        }

        self.validate_unit_enum(mode, &["Reusable", "OneShot"]);
        self.validate_unit_enum(state, &["Registered", "Active", "Completed", "Failed"]);

        for (id, operation) in [
            (register, "register"),
            (retain, "retain"),
            (release, "release"),
            (query_state, "state"),
            (failure, "failure"),
        ] {
            let Some(id) = id else { continue };
            self.current_file = self.function_files[&id];
            let function = &self.functions[id];
            let signature = &self.signatures[&id];
            let common = !signature.is_suspend
                && signature.type_params.len() == 1
                && signature.type_params[0].kind == hir::TypeParamKind::Any
                && signature.attributes.safety == hir::Safety::Unsafe
                && signature.attributes.gc_effect == hir::GcEffect::Managed
                && function.method.is_none();
            let callback_param = |ty| {
                matches!(&self.types[ty], hir::Type::Struct(found, args)
                    if *found == callback
                        && matches!(args.as_slice(), [arg] if self.is_type_param(*arg, 0)))
            };
            let signature_valid = match operation {
                "register" => {
                    matches!(signature.params.as_slice(), [closure, index, mode_param]
                        if closure.ty == self.any
                            && index.ty == self.int
                            && mode_param.ty == self.interned_enum_type(mode))
                        && callback_param(signature.return_ty)
                }
                "retain" => matches!(signature.params.as_slice(), [param]
                    if callback_param(param.ty) && callback_param(signature.return_ty)),
                "release" => matches!(signature.params.as_slice(), [param]
                    if callback_param(param.ty) && signature.return_ty == self.unit),
                "state" => matches!(signature.params.as_slice(), [param]
                    if callback_param(param.ty)
                        && signature.return_ty == self.interned_enum_type(state)),
                "failure" => {
                    matches!(signature.params.as_slice(), [param]
                        if callback_param(param.ty)
                            && matches!(&self.types[signature.return_ty], hir::Type::Enum(id, args)
                                if *id == self.option_enum.expect("Option core exists")
                                    && args.as_slice() == [throwable]))
                }
                _ => unreachable!(),
            };
            if !common || !signature_valid {
                self.error(
                    function.span,
                    format!(
                        "intrinsic `foreign_callback_{operation}` has an invalid core signature"
                    ),
                );
            }
        }

        Some(hir::ForeignCallbackCore {
            callback,
            mode,
            state,
            register: register?,
            retain: retain?,
            release: release?,
            query_state: query_state?,
            failure: failure?,
        })
    }

    fn require_core_enum(&mut self, name: &str, files: &[ast::SourceFile]) -> Option<EnumId> {
        let candidate = self.enums_by_name.get(name).copied();
        if let Some(id) = candidate
            && self
                .enum_files
                .get(&id)
                .copied()
                .unwrap_or(self.user_file_index)
                < self.user_file_index
        {
            return Some(id);
        }
        self.current_file = 0;
        self.error(
            files[0].span,
            format!("scoop.core must define exactly one `{name}` enum"),
        );
        None
    }

    fn validate_unit_enum(&mut self, id: EnumId, names: &[&str]) {
        self.current_file = self.enum_files[&id];
        let declaration = &self.enums[id];
        let valid = declaration.type_params.is_empty()
            && declaration.interfaces.is_empty()
            && declaration.variants.len() == names.len()
            && declaration
                .variants
                .iter()
                .zip(names)
                .all(|(variant, name)| variant.name == *name && variant.fields.is_empty());
        if !valid {
            self.error(
                declaration.span,
                format!(
                    "core `{}` must declare unit variants `{}` in order",
                    declaration.name,
                    names.join("`, `")
                ),
            );
        }
    }

    fn interned_enum_type(&self, enum_id: EnumId) -> TypeId {
        self.types
            .iter()
            .find_map(|(id, ty)| {
                matches!(ty, Type::Enum(found, args) if *found == enum_id && args.is_empty())
                    .then_some(id)
            })
            .expect("a declared non-generic enum has one interned type")
    }

    fn validate_ffi_handle_struct(&mut self, id: StructId, name: &str) {
        self.current_file = self.struct_files[&id];
        let declaration = &self.structs[id];
        let valid = declaration.type_params.len() == 1
            && declaration.type_params[0].kind == hir::TypeParamKind::Ref
            && matches!(declaration.fields.as_slice(), [field] if field.name == "raw" && field.ty == self.uint)
            && declaration.interfaces.is_empty()
            && !declaration.attributes.interior_mutable
            && declaration.attributes.c_layout.is_none();
        if !valid {
            self.error(
                declaration.span,
                format!("core `{name}` must be `struct {name}<T : ref>(val raw: UInt)`"),
            );
        }
    }

    fn validate_gc_intrinsic(&mut self, id: FunctionId, kind: GcIntrinsic) {
        self.current_file = self.function_files[&id];
        let function = &self.functions[id];
        let signature = &self.signatures[&id];
        let parameter_matches = match kind {
            GcIntrinsic::Pin | GcIntrinsic::GetHandle => {
                matches!(signature.params.as_slice(), [param] if self.is_type_param(param.ty, 0))
                    && signature.return_ty == self.uint
            }
            GcIntrinsic::Unpin | GcIntrinsic::ReleaseHandle => {
                matches!(signature.params.as_slice(), [param] if param.ty == self.uint)
                    && self.is_type_param(signature.return_ty, 0)
            }
        };
        let valid = !signature.is_suspend
            && signature.type_params.len() == 1
            && signature.type_params[0].kind == hir::TypeParamKind::Ref
            && signature.attributes.safety == hir::Safety::Unsafe
            && signature.attributes.gc_effect == hir::GcEffect::Managed
            && function.method.is_none()
            && parameter_matches;
        if !valid {
            self.error(
                function.span,
                format!(
                    "intrinsic `{}` has an invalid core GC primitive signature",
                    kind.name()
                ),
            );
        }
    }

    fn validate_ptr_struct(&mut self, id: StructId) {
        self.current_file = self.struct_files[&id];
        let declaration = &self.structs[id];
        let valid = declaration.type_params.len() == 1
            && declaration.type_params[0].kind == hir::TypeParamKind::Value
            && matches!(declaration.fields.as_slice(), [field] if field.name == "_rawPointer" && field.ty == self.uint)
            && declaration.interfaces.is_empty()
            && !declaration.attributes.interior_mutable
            && declaration.attributes.c_layout.is_none();
        if !valid {
            self.error(
                declaration.span,
                "core `Ptr` must be `struct Ptr<T : value>(val _rawPointer: UInt)`".to_string(),
            );
        }
    }

    fn validate_fun_ptr_struct(&mut self, id: StructId) {
        self.current_file = self.struct_files[&id];
        let declaration = &self.structs[id];
        let valid = declaration.type_params.len() == 1
            && declaration.type_params[0].kind == hir::TypeParamKind::Any
            && matches!(declaration.fields.as_slice(), [field] if field.name == "_rawPointer" && field.ty == self.uint)
            && declaration.interfaces.is_empty()
            && self.struct_methods[&id].is_empty()
            && !declaration.attributes.interior_mutable
            && declaration.attributes.c_layout.is_none();
        if !valid {
            self.error(
                declaration.span,
                "core `FunPtr` must be `struct FunPtr<F>(val _rawPointer: UInt)`".to_string(),
            );
        }
    }

    fn validate_ptr_method_intrinsic(
        &mut self,
        id: FunctionId,
        ptr: StructId,
        kind: hir::PointerIntrinsic,
    ) {
        self.current_file = self.function_files[&id];
        let function = &self.functions[id];
        let sig = &self.signatures[&id];
        let owner_matches = self.function_owner.get(&id) == Some(&Owner::Struct(ptr));
        let base = owner_matches
            && !sig.is_suspend
            && sig.owner_type_param_count == 1
            && sig
                .type_params
                .first()
                .is_some_and(|param| param.kind == hir::TypeParamKind::Value && param.name == "T")
            && function.attributes.safety == hir::Safety::Unsafe
            && function.attributes.gc_effect == hir::GcEffect::NoGc;
        let valid = base
            && match kind {
                hir::PointerIntrinsic::ToUInt => {
                    function.name.ends_with(".toUInt")
                        && sig.type_params.len() == 1
                        && sig.params.is_empty()
                        && sig.return_ty == self.uint
                }
                hir::PointerIntrinsic::Cast => {
                    function.name.ends_with(".cast")
                        && sig.type_params.len() == 2
                        && sig.type_params[1].kind == hir::TypeParamKind::Value
                        && sig.params.is_empty()
                        && self.is_ptr_param(sig.return_ty, 1)
                }
                hir::PointerIntrinsic::Load => {
                    function.name.ends_with(".load")
                        && sig.type_params.len() == 1
                        && sig.params.is_empty()
                        && self.is_type_param(sig.return_ty, 0)
                }
                hir::PointerIntrinsic::LoadOffset => {
                    function.name.ends_with(".load")
                        && sig.type_params.len() == 1
                        && matches!(sig.params.as_slice(), [param] if param.ty == self.int)
                        && self.is_type_param(sig.return_ty, 0)
                }
                hir::PointerIntrinsic::Store => {
                    function.name.ends_with(".store")
                        && sig.type_params.len() == 1
                        && matches!(sig.params.as_slice(), [param] if self.is_type_param(param.ty, 0))
                        && sig.return_ty == self.unit
                }
                hir::PointerIntrinsic::StoreOffset => {
                    function.name.ends_with(".store")
                        && sig.type_params.len() == 1
                        && matches!(sig.params.as_slice(), [offset, value] if offset.ty == self.int && self.is_type_param(value.ty, 0))
                        && sig.return_ty == self.unit
                }
                hir::PointerIntrinsic::Plus | hir::PointerIntrinsic::Minus => {
                    function
                        .name
                        .ends_with(if kind == hir::PointerIntrinsic::Plus {
                            ".plus"
                        } else {
                            ".minus"
                        })
                        && sig.type_params.len() == 1
                        && matches!(sig.params.as_slice(), [param] if param.ty == self.int)
                        && self.is_ptr_param(sig.return_ty, 0)
                }
                _ => false,
            };
        if !valid {
            let intrinsic = match &function.kind {
                FunctionKind::Intrinsic(name) => name.as_str(),
                FunctionKind::User(_) => "pointer",
                FunctionKind::Extern(_) => "extern",
            };
            self.error(
                function.span,
                format!("malformed core pointer intrinsic `{intrinsic}`"),
            );
        }
    }

    fn validate_pointer_top_level_intrinsic(
        &mut self,
        id: FunctionId,
        kind: hir::PointerIntrinsic,
    ) {
        self.current_file = self.function_files[&id];
        let function = &self.functions[id];
        let sig = &self.signatures[&id];
        let one_value_param = sig.type_params.len() == 1
            && sig.type_params[0].kind == hir::TypeParamKind::Value
            && sig.owner_type_param_count == 0;
        let valid = function.method.is_none()
            && !sig.is_suspend
            && one_value_param
            && match kind {
                hir::PointerIntrinsic::AddressOf => {
                    function.name == "addressOf"
                        && function.attributes.safety == hir::Safety::Unsafe
                        && function.attributes.gc_effect == hir::GcEffect::Managed
                        && matches!(sig.params.as_slice(), [param] if self.is_type_param(param.ty, 0))
                        && self.is_ptr_param(sig.return_ty, 0)
                }
                hir::PointerIntrinsic::SizeOf | hir::PointerIntrinsic::AlignOf => {
                    function.name
                        == if kind == hir::PointerIntrinsic::SizeOf {
                            "sizeOf"
                        } else {
                            "alignOf"
                        }
                        && function.attributes.safety == hir::Safety::Safe
                        && function.attributes.gc_effect == hir::GcEffect::NoGc
                        && sig.params.is_empty()
                        && sig.return_ty == self.uint
                }
                _ => false,
            };
        if !valid {
            let intrinsic = match &function.kind {
                FunctionKind::Intrinsic(name) => name.as_str(),
                FunctionKind::User(_) => "pointer",
                FunctionKind::Extern(_) => "extern",
            };
            self.error(
                function.span,
                format!("malformed core pointer intrinsic `{intrinsic}`"),
            );
        }
    }

    fn validate_pointer_type_uses(&mut self) {
        let uses = self.pointer_type_uses.clone();
        for (ty, file, span) in uses {
            let Type::Ptr(pointee) = self.types[ty] else {
                continue;
            };
            if self.type_contains_param(pointee) {
                continue;
            }
            if !self.is_gc_free(pointee) {
                self.current_file = file;
                self.error(
                    span,
                    format!(
                        "`Ptr` pointee must be GC-free, found {}",
                        self.type_name(pointee)
                    ),
                );
            }
        }
    }

    fn require_core_interface(
        &mut self,
        name: &str,
        files: &[ast::SourceFile],
    ) -> Option<InterfaceId> {
        let candidate = self
            .interfaces_by_name
            .get(name)
            .map(|(id, _)| *id)
            .filter(|id| self.interface_files[id] < self.user_file_index);
        if candidate.is_none() {
            self.current_file = 0;
            self.error(
                files[0].span,
                format!("scoop.core must define interface `{name}`"),
            );
        }
        candidate
    }

    fn validate_continuation_contract(&mut self, id: InterfaceId) {
        self.current_file = self.interface_files[&id];
        let interface = &self.interfaces[id];
        let throwable = self.throwable.map(|(_, ty)| ty);
        let valid_type_param = matches!(
            interface.type_params.as_slice(),
            [hir::TypeParamDecl {
                variance: hir::Variance::In,
                ..
            }]
        );
        let valid_methods = match interface.methods.as_slice() {
            [resume, resume_exception] => {
                resume.name == "resume"
                    && !resume.is_suspend
                    && resume.type_params.is_empty()
                    && resume.params.len() == 1
                    && self.is_type_param(resume.params[0].ty, 0)
                    && resume.return_ty == self.unit
                    && resume_exception.name == "resumeWithException"
                    && !resume_exception.is_suspend
                    && resume_exception.type_params.is_empty()
                    && resume_exception.params.len() == 1
                    && throwable.is_some_and(|ty| resume_exception.params[0].ty == ty)
                    && resume_exception.return_ty == self.unit
            }
            _ => false,
        };
        if !valid_type_param || !valid_methods {
            self.error(
                interface.span,
                "interface `Continuation<in T>` in scoop.core must declare exactly `fun resume(value: T)` followed by `fun resumeWithException(exception: Throwable)`"
                    .to_string(),
            );
        }
    }

    fn validate_suspend_task_contract(&mut self, id: InterfaceId) {
        self.current_file = self.interface_files[&id];
        let interface = &self.interfaces[id];
        let valid_type_param = matches!(
            interface.type_params.as_slice(),
            [hir::TypeParamDecl {
                variance: hir::Variance::Out,
                ..
            }]
        );
        let valid_method = match interface.methods.as_slice() {
            [run] => {
                run.name == "run"
                    && run.is_suspend
                    && run.type_params.is_empty()
                    && run.params.is_empty()
                    && self.is_type_param(run.return_ty, 0)
            }
            _ => false,
        };
        if !valid_type_param || !valid_method {
            self.error(
                interface.span,
                "interface `SuspendTask<out T>` in scoop.core must declare exactly `suspend fun run(): T`"
                    .to_string(),
            );
        }
    }

    fn validate_suspend_registration_contract(
        &mut self,
        id: InterfaceId,
        continuation: InterfaceId,
    ) {
        self.current_file = self.interface_files[&id];
        let interface = &self.interfaces[id];
        let valid_type_param = matches!(
            interface.type_params.as_slice(),
            [hir::TypeParamDecl {
                variance: hir::Variance::Out,
                ..
            }]
        );
        let valid_method = match interface.methods.as_slice() {
            [register] => {
                register.name == "register"
                    && !register.is_suspend
                    && register.type_params.is_empty()
                    && register.params.len() == 1
                    && self.is_interface_param(register.params[0].ty, continuation, 0)
                    && register.return_ty == self.unit
            }
            _ => false,
        };
        if !valid_type_param || !valid_method {
            self.error(
                interface.span,
                "interface `SuspendRegistration<out T>` in scoop.core must declare exactly `fun register(continuation: Continuation<T>)`"
                    .to_string(),
            );
        }
    }

    fn validate_illegal_state_exception(&mut self, files: &[ast::SourceFile]) -> Option<ClassId> {
        let candidate = self
            .classes_by_name
            .get("IllegalStateException")
            .map(|(id, _)| *id)
            .filter(|id| self.class_files[id] < self.user_file_index);
        let Some(id) = candidate else {
            self.current_file = 0;
            self.error(
                files[0].span,
                "scoop.core must define class `IllegalStateException`".to_string(),
            );
            return None;
        };
        self.current_file = self.class_files[&id];
        let throwable = self.throwable.map(|(id, _)| id);
        let valid = self.classes[id].modifier == hir::ClassModifier::Final
            && throwable.is_some_and(|root| self.class_descends_from(id, root));
        if !valid {
            self.error(
                self.classes[id].span,
                "class `IllegalStateException` in scoop.core must be a final subtype of `Throwable`"
                    .to_string(),
            );
        }
        Some(id)
    }

    fn validate_coroutine_start(
        &mut self,
        id: FunctionId,
        continuation: InterfaceId,
        suspend_task: InterfaceId,
    ) {
        self.current_file = self.function_files[&id];
        let function = &self.functions[id];
        let sig = &self.signatures[&id];
        let valid = function.name == "startCoroutine"
            && !sig.is_suspend
            && sig.type_params.len() == 1
            && sig.params.len() == 2
            && self.is_interface_param(sig.params[0].ty, suspend_task, 0)
            && self.is_interface_param(sig.params[1].ty, continuation, 0)
            && sig.return_ty == self.unit;
        if !valid {
            self.error(
                function.span,
                "intrinsic `coroutine_start` must have signature `fun <T> startCoroutine(task: SuspendTask<T>, completion: Continuation<T>): Unit`"
                    .to_string(),
            );
        }
    }

    fn validate_coroutine_suspend(&mut self, id: FunctionId, suspend_registration: InterfaceId) {
        self.current_file = self.function_files[&id];
        let function = &self.functions[id];
        let sig = &self.signatures[&id];
        let valid = function.name == "suspendCoroutine"
            && sig.is_suspend
            && sig.type_params.len() == 1
            && sig.params.len() == 1
            && self.is_interface_param(sig.params[0].ty, suspend_registration, 0)
            && self.is_type_param(sig.return_ty, 0);
        if !valid {
            self.error(
                function.span,
                "intrinsic `coroutine_suspend` must have signature `suspend fun <T> suspendCoroutine(registration: SuspendRegistration<T>): T`"
                    .to_string(),
            );
        }
    }

    fn require_intrinsic(&mut self, name: &str, files: &[ast::SourceFile]) -> Option<FunctionId> {
        let candidates: Vec<_> = self
            .functions
            .iter()
            .filter_map(|(id, function)| {
                matches!(&function.kind, FunctionKind::Intrinsic(found) if found == name)
                    .then_some(id)
            })
            .collect();
        if let [id] = candidates.as_slice() {
            return Some(*id);
        }
        self.current_file = candidates.first().map_or(0, |id| self.function_files[id]);
        self.error(
            files[0].span,
            format!("scoop.core must define exactly one `{name}` intrinsic"),
        );
        None
    }

    fn is_type_param(&self, ty: TypeId, index: u32) -> bool {
        matches!(
            self.types[ty],
            Type::Param(param) if param == hir::TypeParamId::from_raw(index)
        )
    }

    fn is_ptr_param(&self, ty: TypeId, index: u32) -> bool {
        matches!(
            self.types[ty],
            Type::Ptr(pointee) if self.is_type_param(pointee, index)
        )
    }

    fn is_interface_param(&self, ty: TypeId, interface: InterfaceId, index: u32) -> bool {
        matches!(
            &self.types[ty],
            Type::Interface(found, args)
                if *found == interface
                    && matches!(args.as_slice(), [arg] if self.is_type_param(*arg, index))
        )
    }

    fn class_descends_from(&self, class: ClassId, root: ClassId) -> bool {
        let mut current = Some(class);
        let mut visited = HashSet::new();
        while let Some(id) = current {
            if id == root {
                return true;
            }
            if !visited.insert(id) {
                return false;
            }
            current = self.classes[id].base_class.as_ref().map(|(base, _)| *base);
        }
        false
    }

    /// `scoop.core` must define exactly one enum named `Option` with
    /// exactly one type parameter (hir docs, spec 7.2). A second
    /// `Option` was already rejected as a duplicate enum in pass 1, so
    /// at most one candidate reaches here.
    fn validate_option_enum(&mut self, files: &[ast::SourceFile]) {
        let Some(&(id, file_index, span, type_param_count)) = self.option_candidates.first() else {
            // Attribute to the first file: with a core library present
            // that is a core file; without one it is the user file.
            self.current_file = 0;
            self.error(
                files[0].span,
                "scoop.core must define an enum `Option<T>`".to_string(),
            );
            return;
        };
        if type_param_count != 1 {
            self.current_file = file_index;
            self.error(
                span,
                format!(
                    "enum `Option` in scoop.core must have exactly one type parameter, found {type_param_count}"
                ),
            );
            return;
        }
        self.option_enum = Some(id);
    }

    /// `scoop.core` must define a class named `Throwable` (spec 11.7,
    /// milestone8 DESIGN.md 2.1): the root of the exception hierarchy
    /// that `throw` operands and catch parameter types are checked
    /// against. A `Throwable` declared as another type kind, or only
    /// in the user file, is a core configuration error attributed to
    /// the first file (with a core library present that is a core
    /// file). A second core `Throwable` was already rejected as a
    /// duplicate class in pass 1, so at most one candidate reaches
    /// here.
    fn validate_throwable(&mut self, files: &[ast::SourceFile]) {
        let Some(&candidate) = self.throwable_candidates.first() else {
            self.current_file = 0;
            self.error(
                files[0].span,
                "scoop.core must define a class `Throwable`".to_string(),
            );
            return;
        };
        self.throwable = Some(candidate);
    }

    /// The `Throwable` reference type of `scoop.core`, when validated.
    /// `throw` / catch lowering skips its subtype check when this is
    /// `None` (the misconfigured core was already diagnosed, so the
    /// module is rejected anyway).
    pub(crate) fn throwable_ty(&self) -> Option<TypeId> {
        self.throwable.map(|(_, ty)| ty)
    }

    /// Overload declaration check (pass 2.6, milestone7 DESIGN.md 1.1):
    /// within one name (top-level) or one host (members) two functions
    /// may share a name only when their signatures are distinguishable
    /// — a different parameter count or at least one different
    /// parameter type. Differing only in the return type is a
    /// duplicate. Override / interface-implementation matching is
    /// unaffected (it compares full signatures, pass 2.75). Iterates
    /// in declaration order so diagnostics are deterministic.
    fn check_duplicate_signatures(
        &mut self,
        pending_functions: &[(FunctionId, &ast::FunctionDecl, usize)],
        pending_methods: &[(FunctionId, &ast::FunctionDecl, usize, Owner)],
    ) {
        for (index, &(id, decl, file_index)) in pending_functions.iter().enumerate() {
            let duplicate = pending_functions[..index].iter().any(|&(other, _, _)| {
                self.functions[other].name == decl.name.text
                    && self.same_parameter_signature(id, other)
            });
            if duplicate {
                self.current_file = file_index;
                self.error(
                    decl.name.span,
                    format!(
                        "function `{}` is already declared with the same signature",
                        decl.name.text
                    ),
                );
            }
        }
        for (index, &(id, decl, file_index, owner)) in pending_methods.iter().enumerate() {
            let duplicate = pending_methods[..index]
                .iter()
                .any(|&(other, _, _, other_owner)| {
                    other_owner == owner
                        && self.functions[other].name == self.functions[id].name
                        && self.same_parameter_signature(id, other)
                });
            if duplicate {
                let host = owner.describe(self);
                self.current_file = file_index;
                self.error(
                    decl.name.span,
                    format!(
                        "function `{}` in {host} is already declared with the same signature",
                        decl.name.text
                    ),
                );
            }
        }
    }

    /// Whether two functions have indistinguishable parameter
    /// signatures: same parameter count and pairwise-equal parameter
    /// types (the return type is not part of it).
    fn same_parameter_signature(&self, a: FunctionId, b: FunctionId) -> bool {
        let (Some(a_sig), Some(b_sig)) = (self.signatures.get(&a), self.signatures.get(&b)) else {
            return false;
        };
        let receivers_match = match (
            self.extension_receivers.get(&a),
            self.extension_receivers.get(&b),
        ) {
            (Some(&a), Some(&b)) => self.types_equal(a, b),
            (None, None) => true,
            _ => false,
        };
        receivers_match
            && a_sig.params.len() == b_sig.params.len()
            && a_sig
                .params
                .iter()
                .zip(&b_sig.params)
                .all(|(x, y)| self.types_equal(x.ty, y.ty))
    }

    /// Resolve the field types of a struct declaration. Fields with
    /// duplicate names or unresolvable types are diagnosed and dropped;
    /// the module is rejected anyway once any diagnostic is recorded.
    fn resolve_fields(&mut self, id: StructId, decl: &ast::StructDecl) {
        self.type_params_in_scope = self.structs[id].type_params.clone();
        let mut seen = HashSet::new();
        let mut fields = Vec::new();
        for field in &decl.fields {
            if !seen.insert(field.name.text.clone()) {
                self.error(
                    field.name.span,
                    format!(
                        "duplicate field `{}` in struct `{}`",
                        field.name.text, decl.name.text
                    ),
                );
                continue;
            }
            let Some(ty) = self.resolve_type_ref(&field.ty) else {
                continue; // diagnostic already recorded
            };
            fields.push(hir::Field {
                name: field.name.text.clone(),
                ty,
            });
        }
        self.structs[id].fields = fields;
        self.type_params_in_scope.clear();
    }

    /// Resolve the variants of an enum declaration (pass 2): duplicate
    /// variant and field names are diagnosed, field types resolve in
    /// the enum's type-parameter scope, and constructor-style defaults
    /// must be literals matching the field type (milestone4 DESIGN.md
    /// 5.4).
    fn resolve_variants(&mut self, id: EnumId, decl: &ast::EnumDecl) {
        self.type_params_in_scope = self.enums[id].type_params.clone();
        let mut seen = HashSet::new();
        let mut variants = Vec::new();
        for (index, variant) in decl.variants.iter().enumerate() {
            if !seen.insert(variant.name.text.clone()) {
                self.error(
                    variant.name.span,
                    format!(
                        "duplicate variant `{}` in enum `{}`",
                        variant.name.text, decl.name.text
                    ),
                );
                continue;
            }
            let style = match &variant.kind {
                ast::VariantDeclKind::Unit => VariantStyle::Unit,
                ast::VariantDeclKind::Positional(_) => VariantStyle::Positional,
                ast::VariantDeclKind::Named(_) => VariantStyle::Named,
                ast::VariantDeclKind::Constructor(_) => VariantStyle::Constructor,
            };
            let Some(resolved) = self.resolve_variant_fields(variant) else {
                continue; // diagnostic already recorded
            };
            self.variant_styles.insert((id, index as u32), style);
            variants.push(hir::Variant {
                name: variant.name.text.clone(),
                fields: resolved.fields,
                defaults: resolved.defaults,
            });
        }
        self.enums[id].variants = variants;
        self.type_params_in_scope.clear();
    }

    /// The fields (and constructor-style defaults) of one variant.
    /// Returns `None` after recording a diagnostic.
    fn resolve_variant_fields(&mut self, variant: &ast::VariantDecl) -> Option<ResolvedFields> {
        match &variant.kind {
            ast::VariantDeclKind::Unit => Some(ResolvedFields::default()),
            // Positional fields get `_1`-style names (hir docs).
            ast::VariantDeclKind::Positional(types) => {
                let mut resolved = ResolvedFields::default();
                for (index, ty_ref) in types.iter().enumerate() {
                    let ty = self.resolve_type_ref(ty_ref)?;
                    resolved.fields.push(hir::Field {
                        name: format!("_{}", index + 1),
                        ty,
                    });
                    resolved.defaults.push(None);
                }
                Some(resolved)
            }
            ast::VariantDeclKind::Named(fields) | ast::VariantDeclKind::Constructor(fields) => {
                let constructor = matches!(variant.kind, ast::VariantDeclKind::Constructor(_));
                let mut seen = HashSet::new();
                let mut resolved = ResolvedFields::default();
                for field in fields {
                    if !seen.insert(field.name.text.clone()) {
                        self.error(
                            field.name.span,
                            format!(
                                "duplicate field `{}` in variant `{}`",
                                field.name.text, variant.name.text
                            ),
                        );
                        return None;
                    }
                    let ty = self.resolve_type_ref(&field.ty)?;
                    let default = match &field.default {
                        Some(default) if constructor => {
                            Some(self.resolve_variant_default(variant, field, ty, default)?)
                        }
                        // The parser only produces defaults on
                        // constructor-style variants; reject the shape
                        // here so every AST form is handled.
                        Some(_) => {
                            self.error(
                                field.span,
                                format!(
                                    "default value of field `{}` in variant `{}` is only allowed on constructor-style variants",
                                    field.name.text, variant.name.text
                                ),
                            );
                            return None;
                        }
                        None => None,
                    };
                    resolved.fields.push(hir::Field {
                        name: field.name.text.clone(),
                        ty,
                    });
                    resolved.defaults.push(default);
                }
                Some(resolved)
            }
        }
    }

    /// A constructor-style variant field default (M4: literals only,
    /// milestone4 DESIGN.md 5.4), checked against the field type.
    fn resolve_variant_default(
        &mut self,
        variant: &ast::VariantDecl,
        field: &ast::VariantFieldDecl,
        field_ty: TypeId,
        default: &ast::Expr,
    ) -> Option<hir::Expr> {
        let span = default.span();
        let (kind, ty) = match default {
            ast::Expr::IntLiteral { value, .. } => (hir::ExprKind::IntLiteral(*value), self.int),
            ast::Expr::StringLiteral { value, .. } => {
                (hir::ExprKind::StringLiteral(value.clone()), self.string)
            }
            ast::Expr::BoolLiteral { value, .. } => {
                (hir::ExprKind::BoolLiteral(*value), self.boolean)
            }
            // A negative integer literal (`-1`) parses as unary minus.
            ast::Expr::Unary {
                op: ast::UnOp::Neg,
                operand,
                ..
            } => match &**operand {
                ast::Expr::IntLiteral { value, span } => {
                    let operand = Box::new(hir::Expr {
                        kind: hir::ExprKind::IntLiteral(*value),
                        ty: self.int,
                        span: *span,
                    });
                    (
                        hir::ExprKind::Unary {
                            op: hir::UnOp::Neg,
                            operand,
                        },
                        self.int,
                    )
                }
                _ => return self.invalid_variant_default(variant, field, span),
            },
            _ => return self.invalid_variant_default(variant, field, span),
        };
        if !self.types_equal(field_ty, ty) {
            let expected = self.type_name(field_ty);
            let found = self.type_name(ty);
            self.error(
                span,
                format!(
                    "default value of field `{}` in variant `{}` must be of type {expected}, found {found}",
                    field.name.text, variant.name.text
                ),
            );
            return None;
        }
        Some(hir::Expr { kind, ty, span })
    }

    fn invalid_variant_default(
        &mut self,
        variant: &ast::VariantDecl,
        field: &ast::VariantFieldDecl,
        span: Span,
    ) -> Option<hir::Expr> {
        self.error(
            span,
            format!(
                "default value of field `{}` in variant `{}` must be a literal",
                field.name.text, variant.name.text
            ),
        );
        None
    }

    /// Resolve a function signature: typed parameters, value parameters
    /// and the return type (absent means `Unit`). Parameter
    /// locals are only allocated when the body is lowered (pass 3), so
    /// intrinsic functions — which have no body — keep an empty
    /// `params` list on the `hir::Function`; calls check against this
    /// resolved signature like any other function's (M7).
    fn resolve_signature(&mut self, id: FunctionId, decl: &ast::FunctionDecl) {
        // Member-only flags on a top-level function (M6).
        if decl.modifier == ast::MethodModifier::Abstract {
            self.error(
                decl.name.span,
                format!(
                    "abstract function `{}` is only allowed in abstract classes",
                    decl.name.text
                ),
            );
        }
        if decl.is_override {
            self.error(
                decl.name.span,
                format!(
                    "`{}` is marked `override` but does not override any method",
                    decl.name.text
                ),
            );
        }
        if matches!(decl.body, ast::FunctionBody::None)
            && matches!(self.functions[id].kind, FunctionKind::User(_))
            && !decl
                .annotations
                .iter()
                .any(|annotation| matches!(annotation.name.text.as_str(), "Intrinsic" | "Extern"))
        {
            self.error(
                decl.name.span,
                format!("function `{}` must have a body", decl.name.text),
            );
        }
        let mut type_params = Vec::new();
        for param in &decl.type_params {
            if type_params
                .iter()
                .any(|existing: &hir::TypeParamDecl| existing.name == param.name.text)
            {
                self.error(
                    param.span,
                    format!("duplicate type parameter `{}`", param.name.text),
                );
                continue;
            }
            type_params.push(lower_type_param_decl(param));
        }
        if !type_params.is_empty() {
            self.register_generic(id);
        }
        self.type_params_in_scope = type_params.clone();

        if let Some(receiver) = &decl.receiver_ty
            && let Some(receiver_ty) = self.resolve_type_ref(receiver)
        {
            self.extension_receivers.insert(id, receiver_ty);
        }

        let mut params = Vec::with_capacity(decl.params.len());
        for param in &decl.params {
            // On failure the diagnostic is already recorded and the
            // module is rejected; the parameter is simply dropped.
            if let Some(ty) = self.resolve_type_ref(&param.ty) {
                params.push(FnParam {
                    name: param.name.clone(),
                    ty,
                });
            }
        }
        let return_ty = match &decl.return_ty {
            Some(ty_ref) => self.resolve_type_ref(ty_ref).unwrap_or(self.unit),
            None => self.unit,
        };
        self.type_params_in_scope.clear();

        self.functions[id].type_params = type_params.clone();
        self.functions[id].return_ty = return_ty;
        self.signatures.insert(
            id,
            FnSig {
                is_suspend: decl.is_suspend,
                attributes: self.functions[id].attributes,
                owner_type_param_count: 0,
                type_params,
                params,
                return_ty,
            },
        );
        if let FunctionKind::Extern(extern_id) = self.functions[id].kind {
            let signature = &self.signatures[&id];
            self.extern_functions[extern_id].params =
                signature.params.iter().map(|param| param.ty).collect();
            self.extern_functions[extern_id].return_type = return_ty;
        }
    }

    /// Register a generic definition once and return its typed id.
    pub(crate) fn register_generic(&mut self, function: FunctionId) -> GenericFunctionId {
        if let Some(&generic) = self.generic_by_function.get(&function) {
            return generic;
        }
        let generic = self.generic_functions.alloc(GenericFunction {
            function,
            no_gc_type_params: Vec::new(),
        });
        self.generic_by_function.insert(function, generic);
        generic
    }

    /// Record a resolved generic application, deduplicated by
    /// (generic definition, type arguments), and return the entity id
    /// carried by the export HIR call. Requests from inside generic bodies
    /// may still mention `Type::Param`; the local-concrete HIR pass resolves
    /// them when the requesting instance is materialized.
    pub(crate) fn record_instantiation(
        &mut self,
        function: FunctionId,
        type_args: Vec<TypeId>,
    ) -> hir::ResolvedGenericFunctionId {
        let generic = self.generic_by_function[&function];
        if let Some((id, _)) = self
            .instantiations
            .iter()
            .find(|(_, request)| request.generic == generic && request.type_args == type_args)
        {
            return id;
        }
        self.instantiations
            .alloc(hir::ResolvedGenericFunction { generic, type_args })
    }

    /// The host type of a member-function owner: the class / interface
    /// / struct type, or the enum applied to its own type parameters
    /// (the form `this` has inside the enum's methods).
    pub(crate) fn owner_ty(&mut self, owner: Owner) -> TypeId {
        match owner {
            Owner::Class(id) => self.classes_by_name[&self.classes[id].name].1,
            Owner::Interface(id) => self.interfaces_by_name[&self.interfaces[id].name].1,
            Owner::Struct(id) => self.structs_by_name[&self.structs[id].name].1,
            Owner::Enum(id) => {
                let params = (0..self.enums[id].type_params.len() as u32)
                    .map(|index| self.intern_type(Type::Param(hir::TypeParamId::from_raw(index))))
                    .collect();
                self.intern_type(Type::Enum(id, params))
            }
        }
    }

    /// Type parameters contributed by a member's owning declaration. They
    /// form the prefix of the member function's combined parameter space.
    pub(crate) fn owner_type_params(&self, owner: Owner) -> Vec<hir::TypeParamDecl> {
        match owner {
            Owner::Class(_) => Vec::new(),
            Owner::Struct(id) => self.structs[id].type_params.clone(),
            Owner::Enum(id) => self.enums[id].type_params.clone(),
            Owner::Interface(id) => self.interfaces[id].type_params.clone(),
        }
    }

    /// Allocate a hidden desugaring temporary (`$opt.N` / `$res.N`).
    /// The `$` prefix keeps it out of the source namespace (the parser
    /// never produces `$` identifiers), so it is not registered in
    /// `scopes`; generated code references it by `LocalId` directly.
    pub(crate) fn alloc_hidden(&mut self, prefix: &str, ty: TypeId) -> hir::LocalId {
        let name = format!("${prefix}.{}", self.hidden_count);
        self.hidden_count += 1;
        self.alloc_local(name, ty, false)
    }

    /// Allocate the branch-result local used when a structured control
    /// expression is lowered into statements. Every normally completing
    /// branch assigns it before the resulting local read is reachable.
    pub(crate) fn alloc_hidden_result(&mut self, ty: TypeId) -> hir::LocalId {
        let name = format!("$result.{}", self.hidden_count);
        self.hidden_count += 1;
        self.alloc_local(name, ty, true)
    }

    /// The `Option<T>` enum of `scoop.core` and the variant index of
    /// `name`, when `name` is one of its variants. These names
    /// (`Some` / `None`) are the globally visible constructors the core
    /// library's default import provides (spec 7.2).
    pub(crate) fn option_variant(&self, name: &str) -> Option<(EnumId, u32)> {
        let id = self.option_enum?;
        let index = self.enums[id]
            .variants
            .iter()
            .position(|v| v.name == name)?;
        Some((id, index as u32))
    }

    /// The variant index of `name` in `enum_id`, if it exists.
    pub(crate) fn find_variant(&self, enum_id: EnumId, name: &str) -> Option<u32> {
        self.enums[enum_id]
            .variants
            .iter()
            .position(|v| v.name == name)
            .map(|index| index as u32)
    }

    /// Whether `ty` is `Option<T>`; returns `T`.
    pub(crate) fn as_option(&self, ty: TypeId) -> Option<TypeId> {
        match &self.types[ty] {
            Type::Enum(id, args) if Some(*id) == self.option_enum && args.len() == 1 => {
                Some(args[0])
            }
            _ => None,
        }
    }

    /// `Option<inner>` (interned). Only called when the core `Option`
    /// validated successfully.
    pub(crate) fn option_type(&mut self, inner: TypeId) -> TypeId {
        let id = self
            .option_enum
            .expect("Option types only exist after core validation");
        self.intern_type(Type::Enum(id, vec![inner]))
    }

    pub(crate) fn push_suspension_context(&mut self, context: SuspensionContext) {
        self.suspension_contexts.push(context);
    }

    pub(crate) fn pop_suspension_context(&mut self) {
        assert!(
            self.suspension_contexts.len() > 1,
            "the root suspension context must remain present"
        );
        self.suspension_contexts.pop();
    }

    pub(crate) fn push_safety_context(&mut self, safety: hir::Safety) {
        self.safety_contexts.push(safety);
    }

    pub(crate) fn pop_safety_context(&mut self) {
        assert!(
            self.safety_contexts.len() > 1,
            "the root safety context must remain present"
        );
        self.safety_contexts.pop();
    }

    /// Diagnose a suspend call made from a declaration body whose ABI has
    /// no continuation. The resolved callable, including a generic
    /// instantiation, always leads back to exactly one function entity.
    pub(crate) fn check_suspend_call(&mut self, callable: hir::Callable, span: Span) {
        let function = match callable {
            hir::Callable::Function(function) => function,
            hir::Callable::Generic(instantiation) => {
                let generic = self.instantiations[instantiation].generic;
                self.generic_functions[generic].function
            }
        };
        if !self.functions[function].is_suspend {
            return;
        }
        let context = *self
            .suspension_contexts
            .last()
            .expect("the suspension context stack is initialized non-empty");
        let SuspensionContext::Forbidden(reason) = context else {
            return;
        };
        let callee = self.functions[function].name.clone();
        let location = match reason {
            ForbiddenSuspendContext::TopLevel => "a non-suspend declaration".to_string(),
            ForbiddenSuspendContext::Function => {
                format!("non-suspend function `{}`", self.current_fn_name)
            }
            ForbiddenSuspendContext::ConstructorDelegation => "constructor delegation".to_string(),
        };
        self.error(
            span,
            format!("suspend function `{callee}` cannot be called from {location}"),
        );
    }

    pub(crate) fn check_call_effects(&mut self, callable: hir::Callable, span: Span) {
        self.check_suspend_call(callable, span);
        let function = match callable {
            hir::Callable::Function(function) => function,
            hir::Callable::Generic(instantiation) => {
                let generic = self.instantiations[instantiation].generic;
                self.generic_functions[generic].function
            }
        };
        if self.functions[function].attributes.safety != hir::Safety::Unsafe {
            return;
        }
        let context = self
            .safety_contexts
            .last()
            .copied()
            .expect("the safety context stack is initialized non-empty");
        if context == hir::Safety::Safe {
            self.error(
                span,
                format!(
                    "unsafe function `{}` may only be called from an unsafe context",
                    self.functions[function].name
                ),
            );
        }
    }

    pub(crate) fn require_unsafe_operation(&mut self, span: Span, operation: &str) -> bool {
        let context = self
            .safety_contexts
            .last()
            .copied()
            .expect("the safety context stack is initialized non-empty");
        if context == hir::Safety::Unsafe {
            return true;
        }
        self.error(span, format!("{operation} requires an unsafe context"));
        false
    }

    pub(crate) fn error(&mut self, span: Span, message: String) {
        let mut diagnostic = Diagnostic::at(span, message);
        diagnostic.file = self.current_file;
        self.diagnostics.push(diagnostic);
    }
}

/// Intermediate result of variant field resolution.
#[derive(Default)]
struct ResolvedFields {
    fields: Vec<hir::Field>,
    defaults: Vec<Option<hir::Expr>>,
}
