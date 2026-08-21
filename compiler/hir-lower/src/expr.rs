//! Expression lowering and type checking (milestone2 DESIGN.md 2.2).
//!
//! Every expression that survives this stage carries its type
//! (`hir::Expr::ty`); calls resolve to a `FunctionId`, struct
//! constructions to a `StructId`, field accesses to a `FieldRef`.

use scoop_ast as ast;
use scoop_hir as hir;

use ast::Span;
use hir::{ExprKind, Type, TypeId};

use crate::Lowerer;

impl Lowerer {
    /// Lower an expression, recording a diagnostic and returning `None`
    /// on error.
    pub(crate) fn lower_expr(&mut self, expr: &ast::Expr) -> Option<hir::Expr> {
        match expr {
            ast::Expr::StringLiteral { value, span } => Some(hir::Expr {
                kind: ExprKind::StringLiteral(value.clone()),
                ty: self.string,
                span: *span,
            }),
            ast::Expr::IntLiteral { value, span } => Some(hir::Expr {
                kind: ExprKind::IntLiteral(*value),
                ty: self.int,
                span: *span,
            }),
            ast::Expr::BoolLiteral { value, span } => Some(hir::Expr {
                kind: ExprKind::BoolLiteral(*value),
                ty: self.boolean,
                span: *span,
            }),
            ast::Expr::UnitLiteral { span } => Some(hir::Expr {
                kind: ExprKind::UnitLiteral,
                ty: self.unit,
                span: *span,
            }),
            ast::Expr::TupleLiteral { elements, span } => self.lower_tuple_literal(elements, *span),
            ast::Expr::StructInit { name, args, span } => {
                let Some(&(struct_id, ty)) = self.structs_by_name.get(&name.text) else {
                    self.error(name.span, format!("unknown struct `{}`", name.text));
                    return None;
                };
                self.lower_struct_init(struct_id, ty, args, *span)
            }
            ast::Expr::Var(name) => self.lower_var(name),
            ast::Expr::FieldAccess(access) => self.lower_field_access(access),
            ast::Expr::Call(call) => self.lower_call(call),
            ast::Expr::Binary { op, lhs, rhs, span } => self.lower_binary(*op, lhs, rhs, *span),
            ast::Expr::Unary { op, operand, span } => self.lower_unary(*op, operand, *span),
        }
    }

    fn lower_tuple_literal(&mut self, elements: &[ast::Expr], span: Span) -> Option<hir::Expr> {
        // The parser never produces an empty tuple literal (`()` is a
        // `UnitLiteral`); reject it here so every AST shape is handled.
        if elements.is_empty() {
            self.error(
                span,
                "tuple literal must contain at least one element".to_string(),
            );
            return None;
        }
        let mut lowered = Vec::with_capacity(elements.len());
        for element in elements {
            lowered.push(self.lower_expr(element)?);
        }
        let ty = self.intern_tuple(lowered.iter().map(|element| element.ty).collect());
        Some(hir::Expr {
            kind: ExprKind::TupleLiteral(lowered),
            ty,
            span,
        })
    }

    fn lower_var(&mut self, name: &ast::Ident) -> Option<hir::Expr> {
        let Some(local) = self.scopes.lookup(&name.text) else {
            self.error(name.span, format!("unknown variable `{}`", name.text));
            return None;
        };
        let ty = self.locals[local].ty;
        Some(hir::Expr {
            kind: ExprKind::Local(local),
            ty,
            span: name.span,
        })
    }

    /// `Name(args...)` in call position: the name resolves in the
    /// struct namespace first (construction), then in the function
    /// namespace (M1 direct call).
    fn lower_call(&mut self, call: &ast::CallExpr) -> Option<hir::Expr> {
        if let Some(&(struct_id, ty)) = self.structs_by_name.get(&call.callee.text) {
            return self.lower_struct_init(struct_id, ty, &call.args, call.span);
        }
        self.lower_function_call(call)
    }

    fn lower_function_call(&mut self, call: &ast::CallExpr) -> Option<hir::Expr> {
        let function = match self.functions_by_name.get(&call.callee.text) {
            Some(&id) => id,
            None => {
                self.error(
                    call.callee.span,
                    format!("unknown function `{}`", call.callee.text),
                );
                return None;
            }
        };
        let name = self.functions[function].name.clone();
        let is_builtin = matches!(self.functions[function].kind, hir::FunctionKind::Builtin(_));

        // M2: builtins take exactly one `String` / `Int` / `Boolean`
        // argument, user functions take none; every function returns
        // `Unit`.
        let args = if is_builtin {
            if call.args.len() != 1 {
                let supplied = call.args.len();
                self.error(
                    call.span,
                    format!("`{name}` takes exactly 1 argument, but {supplied} were supplied"),
                );
                return None;
            }
            let arg = self.lower_expr(&call.args[0])?;
            if arg.ty != self.string && arg.ty != self.int && arg.ty != self.boolean {
                let found = self.type_name(arg.ty);
                self.error(
                    arg.span,
                    format!("argument of `{name}` must be String, Int or Boolean, found {found}"),
                );
                return None;
            }
            vec![arg]
        } else {
            if !call.args.is_empty() {
                let supplied = call.args.len();
                self.error(
                    call.span,
                    format!("function `{name}` takes no arguments, but {supplied} were supplied"),
                );
                return None;
            }
            Vec::new()
        };

        Some(hir::Expr {
            kind: ExprKind::Call { function, args },
            ty: self.unit,
            span: call.span,
        })
    }

    /// Struct construction with positional arguments: argument count
    /// and types must match the declared fields one by one.
    fn lower_struct_init(
        &mut self,
        struct_id: hir::StructId,
        ty: TypeId,
        args: &[ast::Expr],
        span: Span,
    ) -> Option<hir::Expr> {
        let name = self.structs[struct_id].name.clone();
        let fields: Vec<(String, TypeId)> = self.structs[struct_id]
            .fields
            .iter()
            .map(|field| (field.name.clone(), field.ty))
            .collect();
        if args.len() != fields.len() {
            let expected = fields.len();
            let supplied = args.len();
            let noun = if expected == 1 {
                "argument"
            } else {
                "arguments"
            };
            self.error(
                span,
                format!(
                    "struct `{name}` takes exactly {expected} {noun}, but {supplied} were supplied"
                ),
            );
            return None;
        }
        let mut lowered = Vec::with_capacity(args.len());
        for (arg, (field_name, field_ty)) in args.iter().zip(fields) {
            let arg = self.lower_expr(arg)?;
            if !self.types_equal(field_ty, arg.ty) {
                let expected = self.type_name(field_ty);
                let found = self.type_name(arg.ty);
                self.error(
                    arg.span,
                    format!(
                        "argument for field `{field_name}` of `{name}` must be of type {expected}, found {found}"
                    ),
                );
                return None;
            }
            lowered.push(arg);
        }
        Some(hir::Expr {
            kind: ExprKind::StructInit {
                struct_id,
                args: lowered,
            },
            ty,
            span,
        })
    }

    fn lower_field_access(&mut self, access: &ast::FieldAccess) -> Option<hir::Expr> {
        let receiver = self.lower_expr(&access.receiver)?;
        match self.types[receiver.ty].clone() {
            Type::Struct(struct_id) => {
                let struct_name = self.structs[struct_id].name.clone();
                match &access.selector {
                    ast::FieldSelector::Name(field) => {
                        let fields = &self.structs[struct_id].fields;
                        let Some(index) = fields.iter().position(|f| f.name == field.text) else {
                            self.error(
                                field.span,
                                format!("struct `{struct_name}` has no field `{}`", field.text),
                            );
                            return None;
                        };
                        let ty = fields[index].ty;
                        Some(hir::Expr {
                            kind: ExprKind::FieldAccess {
                                receiver: Box::new(receiver),
                                field: hir::FieldRef::StructField {
                                    struct_id,
                                    index: index as u32,
                                },
                            },
                            ty,
                            span: access.span,
                        })
                    }
                    ast::FieldSelector::Index(index, span) => {
                        self.error(
                            *span,
                            format!("struct `{struct_name}` has no field `_{index}`"),
                        );
                        None
                    }
                }
            }
            Type::Tuple(elements) => match &access.selector {
                ast::FieldSelector::Index(index, span) => {
                    // Tuple indices are 1-based (`._1` is the first
                    // element); anything outside `1..=len` is an error.
                    let ty = (1..=elements.len() as u32)
                        .contains(index)
                        .then(|| elements[*index as usize - 1]);
                    match ty {
                        Some(ty) => Some(hir::Expr {
                            kind: ExprKind::FieldAccess {
                                receiver: Box::new(receiver),
                                field: hir::FieldRef::TupleIndex(index - 1),
                            },
                            ty,
                            span: access.span,
                        }),
                        None => {
                            let found = self.type_name(receiver.ty);
                            self.error(
                                *span,
                                format!("tuple type `{found}` has no element `_{index}`"),
                            );
                            None
                        }
                    }
                }
                ast::FieldSelector::Name(field) => {
                    let found = self.type_name(receiver.ty);
                    self.error(
                        field.span,
                        format!("tuple type `{found}` has no field `{}`", field.text),
                    );
                    None
                }
            },
            _ => {
                let found = self.type_name(receiver.ty);
                let span = match &access.selector {
                    ast::FieldSelector::Name(field) => field.span,
                    ast::FieldSelector::Index(_, span) => *span,
                };
                self.error(span, format!("type `{found}` has no fields"));
                None
            }
        }
    }

    fn lower_binary(
        &mut self,
        op: ast::BinOp,
        lhs: &ast::Expr,
        rhs: &ast::Expr,
        span: Span,
    ) -> Option<hir::Expr> {
        let lhs = self.lower_expr(lhs)?;
        let rhs = self.lower_expr(rhs)?;
        let (op, symbol) = convert_bin_op(op);
        let ty = match op {
            hir::BinOp::Add => {
                // `String + String` concatenates (docs/milestone2/
                // DESIGN.md 2.3); all other arithmetic is Int-only.
                if lhs.ty == self.string && rhs.ty == self.string {
                    self.string
                } else {
                    self.expect_int_operands(symbol, &lhs, &rhs, span)?;
                    self.int
                }
            }
            hir::BinOp::Sub | hir::BinOp::Mul | hir::BinOp::Div => {
                self.expect_int_operands(symbol, &lhs, &rhs, span)?;
                self.int
            }
            hir::BinOp::Lt | hir::BinOp::Le | hir::BinOp::Gt | hir::BinOp::Ge => {
                self.expect_int_operands(symbol, &lhs, &rhs, span)?;
                self.boolean
            }
            hir::BinOp::Eq | hir::BinOp::Ne => {
                // M2: every type supports structural equality; the two
                // sides just have to agree.
                if !self.types_equal(lhs.ty, rhs.ty) {
                    let lhs_ty = self.type_name(lhs.ty);
                    let rhs_ty = self.type_name(rhs.ty);
                    self.error(
                        span,
                        format!(
                            "operator `{symbol}` requires operands of the same type, found {lhs_ty} and {rhs_ty}"
                        ),
                    );
                    return None;
                }
                self.boolean
            }
            hir::BinOp::And | hir::BinOp::Or => {
                if lhs.ty != self.boolean || rhs.ty != self.boolean {
                    let lhs_ty = self.type_name(lhs.ty);
                    let rhs_ty = self.type_name(rhs.ty);
                    self.error(
                        span,
                        format!(
                            "operator `{symbol}` requires Boolean operands, found {lhs_ty} and {rhs_ty}"
                        ),
                    );
                    return None;
                }
                self.boolean
            }
        };
        Some(hir::Expr {
            kind: ExprKind::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            },
            ty,
            span,
        })
    }

    /// Arithmetic and comparison operators only accept `Int` operands.
    fn expect_int_operands(
        &mut self,
        symbol: &str,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
        span: Span,
    ) -> Option<()> {
        if lhs.ty == self.int && rhs.ty == self.int {
            return Some(());
        }
        let lhs_ty = self.type_name(lhs.ty);
        let rhs_ty = self.type_name(rhs.ty);
        self.error(
            span,
            format!("operator `{symbol}` requires Int operands, found {lhs_ty} and {rhs_ty}"),
        );
        None
    }

    fn lower_unary(&mut self, op: ast::UnOp, operand: &ast::Expr, span: Span) -> Option<hir::Expr> {
        let operand = self.lower_expr(operand)?;
        let (op, symbol, expected, ty) = match op {
            ast::UnOp::Neg => (hir::UnOp::Neg, "-", self.int, self.int),
            ast::UnOp::Not => (hir::UnOp::Not, "!", self.boolean, self.boolean),
        };
        if operand.ty != expected {
            let article = if expected == self.int { "an" } else { "a" };
            let expected_name = self.type_name(expected);
            let found = self.type_name(operand.ty);
            self.error(
                span,
                format!(
                    "operator `{symbol}` requires {article} {expected_name} operand, found {found}"
                ),
            );
            return None;
        }
        Some(hir::Expr {
            kind: ExprKind::Unary {
                op,
                operand: Box::new(operand),
            },
            ty,
            span,
        })
    }
}

fn convert_bin_op(op: ast::BinOp) -> (hir::BinOp, &'static str) {
    match op {
        ast::BinOp::Add => (hir::BinOp::Add, "+"),
        ast::BinOp::Sub => (hir::BinOp::Sub, "-"),
        ast::BinOp::Mul => (hir::BinOp::Mul, "*"),
        ast::BinOp::Div => (hir::BinOp::Div, "/"),
        ast::BinOp::Lt => (hir::BinOp::Lt, "<"),
        ast::BinOp::Le => (hir::BinOp::Le, "<="),
        ast::BinOp::Gt => (hir::BinOp::Gt, ">"),
        ast::BinOp::Ge => (hir::BinOp::Ge, ">="),
        ast::BinOp::Eq => (hir::BinOp::Eq, "=="),
        ast::BinOp::Ne => (hir::BinOp::Ne, "!="),
        ast::BinOp::And => (hir::BinOp::And, "&&"),
        ast::BinOp::Or => (hir::BinOp::Or, "||"),
    }
}
