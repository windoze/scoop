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
//! instantiation requests — and the builtin `Option<T>` with `Some` /
//! `None` / `?.` / `?:` / `!!` (desugared to statement-level control
//! flow over hidden temporaries).

mod expr;
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
use hir::{Builtin, Function, FunctionId, FunctionKind, StructDecl, StructId, Type, TypeId};
use scope::Scopes;

/// Lower a parsed source file to HIR.
///
/// All semantic errors of the M3 subset are diagnosed here with spans;
/// downstream stages (MIR, LIR) never fail.
pub fn lower(file: &ast::SourceFile) -> Result<hir::Module, Vec<Diagnostic>> {
    Lowerer::new(file.span).run(file)
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

pub(crate) struct Lowerer {
    pub(crate) types: Arena<Type>,
    pub(crate) structs: Arena<StructDecl>,
    pub(crate) functions: Arena<Function>,
    pub(crate) top_level: Vec<FunctionId>,
    pub(crate) unit: TypeId,
    pub(crate) int: TypeId,
    pub(crate) boolean: TypeId,
    pub(crate) string: TypeId,
    pub(crate) print: FunctionId,
    pub(crate) println: FunctionId,
    /// Function namespace (builtins included). Struct names live in a
    /// separate namespace: a struct and a function may share a name.
    pub(crate) functions_by_name: HashMap<String, FunctionId>,
    /// Struct namespace: name → (declaration, value type of the struct).
    pub(crate) structs_by_name: HashMap<String, (StructId, TypeId)>,
    /// Resolved signatures of user functions (pass 2.5), consulted by
    /// call lowering and body lowering.
    pub(crate) signatures: HashMap<FunctionId, FnSig>,
    /// Type parameter names of the function whose signature or body is
    /// currently being lowered; empty elsewhere.
    pub(crate) type_params_in_scope: Vec<String>,
    /// Return type of the function whose body is being lowered.
    pub(crate) current_return_ty: TypeId,
    /// Name of the function whose body is being lowered (diagnostics).
    pub(crate) current_fn_name: String,
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
    fn new(file_span: Span) -> Self {
        // Well-known types are allocated first, in a fixed order
        // (impl spec 2.2): Unit, Int, Boolean, String.
        let mut types = Arena::new();
        let unit = types.alloc(Type::Unit);
        let int = types.alloc(Type::Int);
        let boolean = types.alloc(Type::Boolean);
        let string = types.alloc(Type::String);

        // Builtin output functions (temporary until M11, DESIGN.md 5.2).
        // They have no source span; attribute them to the whole file.
        // Their argument is checked structurally at the call site
        // (String / Int / Boolean), so they carry no parameter list.
        let mut functions = Arena::new();
        let print = functions.alloc(Function {
            name: "print".to_string(),
            type_params: Vec::new(),
            params: Vec::new(),
            return_ty: unit,
            kind: FunctionKind::Builtin(Builtin::Print),
            span: file_span,
        });
        let println = functions.alloc(Function {
            name: "println".to_string(),
            type_params: Vec::new(),
            params: Vec::new(),
            return_ty: unit,
            kind: FunctionKind::Builtin(Builtin::Println),
            span: file_span,
        });

        let functions_by_name = HashMap::from([
            ("print".to_string(), print),
            ("println".to_string(), println),
        ]);

        Lowerer {
            types,
            structs: Arena::new(),
            functions,
            top_level: vec![print, println],
            unit,
            int,
            boolean,
            string,
            print,
            println,
            functions_by_name,
            structs_by_name: HashMap::new(),
            signatures: HashMap::new(),
            type_params_in_scope: Vec::new(),
            current_return_ty: unit,
            current_fn_name: String::new(),
            locals: Arena::new(),
            scopes: Scopes::new(),
            instantiations: Vec::new(),
            hidden_count: 0,
            diagnostics: Vec::new(),
        }
    }

    fn run(mut self, file: &ast::SourceFile) -> Result<hir::Module, Vec<Diagnostic>> {
        // Pass 1: declare structs and functions, so bodies and field
        // types resolve regardless of declaration order. Structs and
        // functions occupy separate namespaces.
        let mut user_structs = Vec::new();
        let mut user_functions = Vec::new();
        for decl in &file.declarations {
            match decl {
                ast::Decl::Struct(decl) => {
                    if self.structs_by_name.contains_key(&decl.name.text) {
                        self.error(
                            decl.name.span,
                            format!("duplicate struct `{}`", decl.name.text),
                        );
                        continue;
                    }
                    let id = self.structs.alloc(StructDecl {
                        name: decl.name.text.clone(),
                        fields: Vec::new(),
                        span: decl.span,
                    });
                    let ty = self.types.alloc(Type::Struct(id));
                    self.structs_by_name
                        .insert(decl.name.text.clone(), (id, ty));
                    user_structs.push((id, decl));
                }
                ast::Decl::Function(decl) => {
                    if self.functions_by_name.contains_key(&decl.name.text) {
                        self.error(
                            decl.name.span,
                            format!("duplicate function `{}`", decl.name.text),
                        );
                        continue;
                    }
                    let id = self.functions.alloc(Function {
                        name: decl.name.text.clone(),
                        // Filled in pass 2.5 (signature) and pass 3
                        // (parameter locals); a resolution failure is
                        // diagnosed, so these never reach the output.
                        type_params: Vec::new(),
                        params: Vec::new(),
                        return_ty: self.unit,
                        kind: FunctionKind::User(hir::Body {
                            locals: Arena::new(),
                            statements: Vec::new(),
                        }),
                        span: decl.span,
                    });
                    self.top_level.push(id);
                    self.functions_by_name.insert(decl.name.text.clone(), id);
                    user_functions.push((id, decl));
                }
            }
        }

        // Pass 2: resolve struct fields (all struct names are known now,
        // so fields may reference later-declared structs).
        for (id, decl) in user_structs {
            self.resolve_fields(id, decl);
        }

        // Pass 2.5: resolve function signatures, so calls in any body
        // see parameter and return types regardless of declaration
        // order.
        for &(id, decl) in &user_functions {
            self.resolve_signature(id, decl);
        }

        // Pass 3: lower bodies.
        for (id, decl) in user_functions {
            let body = self.lower_body(id, decl);
            self.functions[id].kind = FunctionKind::User(body);
        }

        // A module without `main` never reaches HIR (hir docs); it is a
        // diagnostic here, attributed to the whole file.
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
                    file.span,
                    "missing entry point: declare `fun main()`".to_string(),
                );
                None
            }
        };

        if !self.diagnostics.is_empty() {
            return Err(self.diagnostics);
        }
        // Invariant: empty diagnostics implies `main` was found above.
        let entry = entry.expect("missing `main` is always diagnosed");
        Ok(hir::Module {
            types: self.types,
            functions: self.functions,
            structs: self.structs,
            top_level: self.top_level,
            unit: self.unit,
            int: self.int,
            boolean: self.boolean,
            string: self.string,
            print: self.print,
            println: self.println,
            entry,
            instantiations: self.instantiations,
        })
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

    /// Resolve a function signature: type parameter names, parameter
    /// types and the return type (absent means `Unit`). Parameter
    /// locals are only allocated when the body is lowered (pass 3).
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

    pub(crate) fn error(&mut self, span: Span, message: String) {
        self.diagnostics.push(Diagnostic::at(span, message));
    }
}
