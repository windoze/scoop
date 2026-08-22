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

mod expr;
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
    EnumDecl, EnumId, Function, FunctionId, FunctionKind, StructDecl, StructId, Type, TypeId,
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
/// All semantic errors of the M4 subset are diagnosed here with spans;
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

pub(crate) struct Lowerer {
    pub(crate) types: Arena<Type>,
    pub(crate) structs: Arena<StructDecl>,
    pub(crate) enums: Arena<EnumDecl>,
    pub(crate) functions: Arena<Function>,
    pub(crate) top_level: Vec<FunctionId>,
    pub(crate) unit: TypeId,
    pub(crate) int: TypeId,
    pub(crate) boolean: TypeId,
    pub(crate) string: TypeId,
    /// Function namespace. Struct and enum names live in separate
    /// namespaces: a struct and a function may share a name.
    pub(crate) functions_by_name: HashMap<String, FunctionId>,
    /// Struct namespace: name → (declaration, value type of the struct).
    pub(crate) structs_by_name: HashMap<String, (StructId, TypeId)>,
    /// Enum namespace.
    pub(crate) enums_by_name: HashMap<String, EnumId>,
    /// Enums named `Option` declared in core files:
    /// (declaration, file index, span, type parameter count). Validated
    /// after pass 1 (`validate_option_enum`).
    option_candidates: Vec<(EnumId, usize, Span, usize)>,
    /// The validated `Option<T>` enum of `scoop.core`; `None` only
    /// when the core library is misconfigured (diagnosed, so the
    /// module is rejected anyway).
    pub(crate) option_enum: Option<EnumId>,
    /// Surface form of every variant, for pattern shape checks.
    pub(crate) variant_styles: HashMap<(EnumId, u32), VariantStyle>,
    /// Resolved signatures of all functions (pass 2.5), consulted by
    /// call lowering and body lowering.
    pub(crate) signatures: HashMap<FunctionId, FnSig>,
    /// Type parameter names of the function or enum whose signature,
    /// variants or body is currently being lowered; empty elsewhere.
    pub(crate) type_params_in_scope: Vec<String>,
    /// Return type of the function whose body is being lowered.
    pub(crate) current_return_ty: TypeId,
    /// Name of the function whose body is being lowered (diagnostics).
    pub(crate) current_fn_name: String,
    /// Index of the file currently being processed (diagnostics).
    pub(crate) current_file: usize,
    /// Locals of the body currently being lowered (taken into the
    /// finished `hir::Body`).
    pub(crate) locals: Arena<hir::Local>,
    pub(crate) scopes: Scopes,
    /// Deduplicated monomorphization requests, in first-use order.
    pub(crate) instantiations: Vec<hir::Instantiation>,
    /// Counter for hidden `$opt.N` / `$res.N` desugaring temporaries.
    pub(crate) hidden_count: u32,
    pub(crate) diagnostics: Vec<Diagnostic>,
}

impl Lowerer {
    fn new() -> Self {
        // Well-known types are allocated first, in a fixed order
        // (impl spec 2.2): Unit, Int, Boolean, String.
        let mut types = Arena::new();
        let unit = types.alloc(Type::Unit);
        let int = types.alloc(Type::Int);
        let boolean = types.alloc(Type::Boolean);
        let string = types.alloc(Type::String);

        Lowerer {
            types,
            structs: Arena::new(),
            enums: Arena::new(),
            functions: Arena::new(),
            top_level: Vec::new(),
            unit,
            int,
            boolean,
            string,
            functions_by_name: HashMap::new(),
            structs_by_name: HashMap::new(),
            enums_by_name: HashMap::new(),
            option_candidates: Vec::new(),
            option_enum: None,
            variant_styles: HashMap::new(),
            signatures: HashMap::new(),
            type_params_in_scope: Vec::new(),
            current_return_ty: unit,
            current_fn_name: String::new(),
            current_file: 0,
            locals: Arena::new(),
            scopes: Scopes::new(),
            instantiations: Vec::new(),
            hidden_count: 0,
            diagnostics: Vec::new(),
        }
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

        // Pass 1: declare structs, enums and functions across all
        // files (core first), so bodies and field types resolve
        // regardless of declaration order. Structs, enums and
        // functions occupy separate namespaces; struct and enum names
        // share the *type* namespace and must not collide.
        let mut pending_structs = Vec::new();
        let mut pending_enums = Vec::new();
        let mut pending_functions = Vec::new();
        for (file_index, file) in files.iter().enumerate() {
            self.current_file = file_index;
            let is_core = file_index < user_file_index;
            for decl in &file.declarations {
                match decl {
                    ast::Decl::Struct(decl) => {
                        self.declare_struct(decl, &mut pending_structs, file_index)
                    }
                    ast::Decl::Enum(decl) => {
                        self.declare_enum(decl, is_core, &mut pending_enums, file_index)
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

        // Pass 2: resolve struct fields and enum variants (all type
        // names are known now, so fields may reference later-declared
        // types).
        for (id, decl, file_index) in pending_structs {
            self.current_file = file_index;
            self.resolve_fields(id, decl);
        }
        for (id, decl, file_index) in pending_enums {
            self.current_file = file_index;
            self.resolve_variants(id, decl);
        }

        // Pass 2.5: resolve function signatures, so calls in any body
        // see parameter and return types regardless of declaration
        // order.
        for &(id, decl, file_index) in &pending_functions {
            self.current_file = file_index;
            self.resolve_signature(id, decl);
        }

        // Pass 3: lower bodies. Intrinsics have no body to lower (the
        // parser guarantees it is omitted); their `kind` was set at
        // declaration time.
        for (id, decl, file_index) in pending_functions {
            if matches!(self.functions[id].kind, FunctionKind::Intrinsic(_)) {
                continue;
            }
            self.current_file = file_index;
            let body = self.lower_body(id, decl);
            self.functions[id].kind = FunctionKind::User(body);
        }

        // A module without `main` never reaches HIR (hir docs); it is a
        // diagnostic here, attributed to the user file.
        self.current_file = user_file_index;
        let entry = match self.functions_by_name.get("main") {
            Some(&id) => {
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
            structs: self.structs,
            enums: self.enums,
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

    fn declare_struct<'a>(
        &mut self,
        decl: &'a ast::StructDecl,
        pending: &mut Vec<(StructId, &'a ast::StructDecl, usize)>,
        file_index: usize,
    ) {
        if self.structs_by_name.contains_key(&decl.name.text) {
            self.error(
                decl.name.span,
                format!("duplicate struct `{}`", decl.name.text),
            );
            return;
        }
        if self.enums_by_name.contains_key(&decl.name.text) {
            self.error(
                decl.name.span,
                format!(
                    "duplicate type `{}` (already declared as an enum)",
                    decl.name.text
                ),
            );
            return;
        }
        let id = self.structs.alloc(StructDecl {
            name: decl.name.text.clone(),
            fields: Vec::new(),
            span: decl.span,
        });
        let ty = self.types.alloc(Type::Struct(id));
        self.structs_by_name
            .insert(decl.name.text.clone(), (id, ty));
        pending.push((id, decl, file_index));
    }

    fn declare_enum<'a>(
        &mut self,
        decl: &'a ast::EnumDecl,
        is_core: bool,
        pending: &mut Vec<(EnumId, &'a ast::EnumDecl, usize)>,
        file_index: usize,
    ) {
        if self.enums_by_name.contains_key(&decl.name.text) {
            self.error(
                decl.name.span,
                format!("duplicate enum `{}`", decl.name.text),
            );
            return;
        }
        if self.structs_by_name.contains_key(&decl.name.text) {
            self.error(
                decl.name.span,
                format!(
                    "duplicate type `{}` (already declared as a struct)",
                    decl.name.text
                ),
            );
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
            span: decl.span,
        });
        self.enums_by_name.insert(decl.name.text.clone(), id);
        if is_core && decl.name.text == "Option" {
            self.option_candidates
                .push((id, file_index, decl.span, decl.type_params.len()));
        }
        pending.push((id, decl, file_index));
    }

    fn declare_function<'a>(
        &mut self,
        decl: &'a ast::FunctionDecl,
        is_core: bool,
        pending: &mut Vec<(FunctionId, &'a ast::FunctionDecl, usize)>,
        file_index: usize,
    ) {
        if self.functions_by_name.contains_key(&decl.name.text) {
            self.error(
                decl.name.span,
                format!("duplicate function `{}`", decl.name.text),
            );
            return;
        }
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
            span: decl.span,
        });
        self.top_level.push(id);
        self.functions_by_name.insert(decl.name.text.clone(), id);
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
            if !hir::INTRINSIC_REGISTRY.contains(&name.as_str()) {
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

    /// Resolve the field types of a struct declaration. Fields with
    /// duplicate names or unresolvable types are diagnosed and dropped;
    /// the module is rejected anyway once any diagnostic is recorded.
    fn resolve_fields(&mut self, id: StructId, decl: &ast::StructDecl) {
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
    /// `params` list; their calls are checked against the intrinsic
    /// registry's signature rules instead.
    fn resolve_signature(&mut self, id: FunctionId, decl: &ast::FunctionDecl) {
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
                type_params,
                params,
                return_ty,
            },
        );
    }

    /// Record a monomorphization request, deduplicated by
    /// (function, type arguments). Type ids are interned, so id
    /// equality is structural equality. Requests from inside generic
    /// function bodies may still mention `Type::Param`; mir-lower
    /// concretizes those when the requesting instance is materialized.
    pub(crate) fn record_instantiation(&mut self, function: FunctionId, type_args: Vec<TypeId>) {
        let request = hir::Instantiation {
            function,
            type_args,
        };
        if !self.instantiations.contains(&request) {
            self.instantiations.push(request);
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
