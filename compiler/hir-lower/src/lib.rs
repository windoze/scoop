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
//! registry only backs the single `rt_write` primitive and no
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
//! generic structs `PinHandle<T>` / `GcHandle<T>` and the `pin` /
//! `unpin` / `getGcHandle`
//! / `releaseGcHandle` intrinsics, whose inferred type argument must
//! be a reference type — spec 14.1's `T : ref` in its pre-M12 form.
//! `gcCollect` / `gcStats` are ordinary test-only intrinsics.

mod class;
mod expr;
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

use ast::{Diagnostic, Span};
use hir::{
    ClassDecl, ClassId, EnumDecl, EnumId, Function, FunctionId, FunctionKind, GenericFunction,
    GenericFunctionId, InterfaceDecl, InterfaceId, StructDecl, StructId, Type, TypeId,
};
use scope::Scopes;

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
pub fn lower(files: &[ast::SourceFile]) -> Result<hir::Module, Vec<Diagnostic>> {
    Lowerer::new().run(files)
}

/// A resolved function signature. Kept separate from `hir::Function`
/// because parameter locals can only be allocated while the body (and
/// its `locals` arena) is being lowered; signatures must be known
/// before any body, so calls resolve regardless of declaration order.
#[derive(Clone)]
pub(crate) struct FnSig {
    /// Number of owner parameters at the front of `type_params`.
    pub(crate) owner_type_param_count: usize,
    pub(crate) type_params: Vec<String>,
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

pub(crate) struct Lowerer {
    pub(crate) types: Arena<Type>,
    pub(crate) structs: Arena<StructDecl>,
    pub(crate) enums: Arena<EnumDecl>,
    pub(crate) classes: Arena<ClassDecl>,
    pub(crate) interfaces: Arena<InterfaceDecl>,
    pub(crate) functions: Arena<Function>,
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
    /// The file each top-level function was declared in, for the
    /// layering of overload resolution (user file → core implicit
    /// imports, milestone7 DESIGN.md 1.2).
    pub(crate) function_files: HashMap<FunctionId, usize>,
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
    pub(crate) type_params_in_scope: Vec<String>,
    /// Return type of the function whose body is being lowered.
    pub(crate) current_return_ty: TypeId,
    /// Name of the function whose body is being lowered (diagnostics).
    pub(crate) current_fn_name: String,
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
    /// Deduplicated monomorphization requests, in first-use order.
    pub(crate) instantiations: Arena<hir::ResolvedGenericFunction>,
    /// Counter for hidden `$opt.N` / `$res.N` desugaring temporaries.
    pub(crate) hidden_count: u32,
    pub(crate) diagnostics: Vec<Diagnostic>,
}

impl Lowerer {
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
            structs: Arena::new(),
            enums: Arena::new(),
            classes: Arena::new(),
            interfaces: Arena::new(),
            functions: Arena::new(),
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
            function_files: HashMap::new(),
            user_file_index: 0,
            structs_by_name: HashMap::new(),
            enums_by_name: HashMap::new(),
            classes_by_name: HashMap::new(),
            interfaces_by_name: HashMap::new(),
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
            current_fn_name: String::new(),
            current_this: None,
            current_owner: None,
            smart_casts: HashMap::new(),
            current_file: 0,
            locals: Arena::new(),
            scopes: Scopes::new(),
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
            let this = locals.alloc(hir::Local {
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
                let local = locals.alloc(hir::Local {
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
                type_params: Vec::new(),
                params: fn_params,
                return_ty,
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
        let mut pending_methods: Vec<(FunctionId, &ast::FunctionDecl, usize, Owner)> = Vec::new();
        for (file_index, file) in files.iter().enumerate() {
            self.current_file = file_index;
            let is_core = file_index < user_file_index;
            for decl in &file.declarations {
                match decl {
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
            self.resolve_fields(id, decl);
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
            if matches!(self.functions[id].kind, FunctionKind::Intrinsic(_)) {
                continue;
            }
            self.current_file = file_index;
            let body = self.lower_body(id, decl);
            self.functions[id].kind = FunctionKind::User(body);
        }
        for (id, decl, file_index, owner) in pending_methods {
            // Interface and abstract methods are bodyless; their
            // parameter-only body was built in pass 2.5.
            if decl.modifier == ast::MethodModifier::Abstract
                || matches!(owner, Owner::Interface(_))
            {
                continue;
            }
            self.current_file = file_index;
            let body = self.lower_body(id, decl);
            self.functions[id].kind = FunctionKind::User(body);
        }

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
        Ok(hir::Module {
            types: self.types,
            functions: self.functions,
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
            entry,
            instantiations: self.instantiations,
        })
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
            if type_params.contains(&param.text) {
                self.error(
                    param.span,
                    format!("duplicate type parameter `{}`", param.text),
                );
                continue;
            }
            type_params.push(param.text.clone());
        }
        let id = self.structs.alloc(StructDecl {
            name: decl.name.text.clone(),
            type_params: type_params.clone(),
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
            if type_params.contains(&param.text) {
                self.error(
                    param.span,
                    format!("duplicate type parameter `{}`", param.text),
                );
                continue;
            }
            type_params.push(param.text.clone());
        }
        let id = self.enums.alloc(EnumDecl {
            name: decl.name.text.clone(),
            type_params,
            // Filled in pass 2; a resolution failure is diagnosed, so
            // empty variants never reach the output.
            variants: Vec::new(),
            interfaces: Vec::new(),
            span: decl.span,
        });
        self.enums_by_name.insert(decl.name.text.clone(), id);
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
            let variance = match param.variance {
                ast::Variance::Invariant => hir::Variance::Invariant,
                ast::Variance::In => hir::Variance::In,
                ast::Variance::Out => hir::Variance::Out,
            };
            type_params.push(hir::TypeParamDecl {
                name: param.name.text.clone(),
                variance,
                span: param.span,
            });
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
        if self.check_annotations(decl, false).is_some() {
            // The diagnostic was already recorded (`@Intrinsic` is
            // core-library top-level only); drop the method.
            return;
        }
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
        let owner_type_param_count = self.owner_type_param_names(owner).len() as u32;
        let id = self.functions.alloc(Function {
            name: format!("{}.{}", owner.describe_name(self), decl.name.text),
            // Filled in pass 2.5 (signature) and pass 3 (body and
            // parameter locals).
            type_params: Vec::new(),
            params: Vec::new(),
            return_ty: self.unit,
            kind: FunctionKind::User(hir::Body {
                locals: Arena::new(),
                statements: Vec::new(),
            }),
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
        let kind = match self.check_annotations(decl, is_core) {
            Some(intrinsic) => FunctionKind::Intrinsic(intrinsic),
            None => FunctionKind::User(hir::Body {
                locals: Arena::new(),
                statements: Vec::new(),
            }),
        };
        let id = self.functions.alloc(Function {
            name: decl.name.text.clone(),
            // Filled in pass 2.5 (signature) and pass 3 (parameter
            // locals); a resolution failure is diagnosed, so these
            // never reach the output.
            type_params: Vec::new(),
            params: Vec::new(),
            return_ty: self.unit,
            kind,
            method: None,
            span: decl.span,
        });
        self.top_level.push(id);
        self.functions_by_name
            .entry(decl.name.text.clone())
            .or_default()
            .push(id);
        self.function_files.insert(id, file_index);
        pending.push((id, decl, file_index));
    }

    /// Check a function's annotations (M4: only `@Intrinsic("name")`,
    /// spec 13.1 / milestone4 DESIGN.md 1.3) and return the intrinsic
    /// name on success. Intrinsics are core-library only and the name
    /// must be in the compiler's registry.
    fn check_annotations(&mut self, decl: &ast::FunctionDecl, is_core: bool) -> Option<String> {
        let mut intrinsic = None;
        for annotation in &decl.annotations {
            if annotation.name.text != "Intrinsic" {
                self.error(
                    annotation.span,
                    format!("unsupported annotation `@{}`", annotation.name.text),
                );
                continue;
            }
            let Some(name) = &annotation.value else {
                self.error(
                    annotation.span,
                    "`@Intrinsic` requires a name argument".to_string(),
                );
                continue;
            };
            if !hir::INTRINSIC_REGISTRY.iter().any(|spec| spec.name == name) {
                self.error(annotation.span, format!("unknown intrinsic `{name}`"));
                continue;
            }
            if !is_core {
                self.error(
                    annotation.span,
                    "`@Intrinsic` is only allowed in the core library".to_string(),
                );
                continue;
            }
            intrinsic = Some(name.clone());
        }
        intrinsic
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
        a_sig.params.len() == b_sig.params.len()
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

    /// Resolve a function signature: type parameter names, parameter
    /// types and the return type (absent means `Unit`). Parameter
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
            && !matches!(self.functions[id].kind, FunctionKind::Intrinsic(_))
        {
            self.error(
                decl.name.span,
                format!("function `{}` must have a body", decl.name.text),
            );
        }
        let mut type_params = Vec::new();
        for param in &decl.type_params {
            if type_params.contains(&param.text) {
                self.error(
                    param.span,
                    format!("duplicate type parameter `{}`", param.text),
                );
                continue;
            }
            type_params.push(param.text.clone());
        }
        if !type_params.is_empty() {
            self.register_generic(id);
        }
        self.type_params_in_scope = type_params.clone();

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
                owner_type_param_count: 0,
                type_params,
                params,
                return_ty,
            },
        );
    }

    /// Register a generic definition once and return its typed id.
    pub(crate) fn register_generic(&mut self, function: FunctionId) -> GenericFunctionId {
        if let Some(&generic) = self.generic_by_function.get(&function) {
            return generic;
        }
        let generic = self.generic_functions.alloc(GenericFunction { function });
        self.generic_by_function.insert(function, generic);
        generic
    }

    /// Record a resolved generic application, deduplicated by
    /// (generic definition, type arguments), and return the entity id
    /// carried by the HIR call. Requests from inside generic bodies may
    /// still mention `Type::Param`; mir-lower concretizes them when the
    /// requesting instance is materialized.
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
    pub(crate) fn owner_type_param_names(&self, owner: Owner) -> Vec<String> {
        match owner {
            Owner::Class(_) => Vec::new(),
            Owner::Struct(id) => self.structs[id].type_params.clone(),
            Owner::Enum(id) => self.enums[id].type_params.clone(),
            Owner::Interface(id) => self.interfaces[id]
                .type_params
                .iter()
                .map(|param| param.name.clone())
                .collect(),
        }
    }

    /// Allocate a hidden desugaring temporary (`$opt.N` / `$res.N`).
    /// The `$` prefix keeps it out of the source namespace (the parser
    /// never produces `$` identifiers), so it is not registered in
    /// `scopes`; generated code references it by `LocalId` directly.
    pub(crate) fn alloc_hidden(&mut self, prefix: &str, ty: TypeId) -> hir::LocalId {
        let name = format!("${prefix}.{}", self.hidden_count);
        self.hidden_count += 1;
        self.locals.alloc(hir::Local {
            name,
            ty,
            mutable: false,
        })
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
