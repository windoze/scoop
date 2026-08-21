//! HIR stage: desugaring, type check, overload resolution, instantiation
//! requests. All compile-time errors are reported here.
//!
//! See `docs/specs/SCOOP-IMPL-SPEC.md` section 2.2.
//!
//! M2 (milestone2 DESIGN.md 2.2): on top of M1 this stage registers
//! struct declarations, resolves type annotations, checks every
//! expression (every `Expr` leaves with its `ty` filled in, every field
//! access with a resolved `FieldRef`), and tracks block-level lexical
//! scopes for local `val` / `var` declarations.

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
/// All semantic errors of the M2 subset are diagnosed here with spans;
/// downstream stages (MIR, LIR) never fail.
pub fn lower(file: &ast::SourceFile) -> Result<hir::Module, Vec<Diagnostic>> {
    Lowerer::new(file.span).run(file)
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
    /// Locals of the body currently being lowered (taken into the
    /// finished `hir::Body`).
    pub(crate) locals: Arena<hir::Local>,
    pub(crate) scopes: Scopes,
    diagnostics: Vec<Diagnostic>,
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
        let mut functions = Arena::new();
        let print = functions.alloc(Function {
            name: "print".to_string(),
            kind: FunctionKind::Builtin(Builtin::Print),
            span: file_span,
        });
        let println = functions.alloc(Function {
            name: "println".to_string(),
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
            locals: Arena::new(),
            scopes: Scopes::new(),
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

        // Pass 3: lower bodies.
        for (id, decl) in user_functions {
            let body = self.lower_body(decl);
            self.functions[id].kind = FunctionKind::User(body);
        }

        // A module without `main` never reaches HIR (hir docs); it is a
        // diagnostic here, attributed to the whole file.
        let entry = match self.functions_by_name.get("main") {
            Some(&id) => Some(id),
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

    pub(crate) fn error(&mut self, span: Span, message: String) {
        self.diagnostics.push(Diagnostic::at(span, message));
    }
}
