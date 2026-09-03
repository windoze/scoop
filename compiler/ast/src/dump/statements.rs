use super::super::*;
use super::{dump_expr, dump_pattern, dump_type_ref};

pub(super) fn dump_block(block: &Block, indent: usize, out: &mut String) {
    for statement in &block.statements {
        dump_statement(statement, indent, out);
    }
}

fn dump_statement(statement: &Statement, indent: usize, out: &mut String) {
    let pad = "  ".repeat(indent);
    match &statement.kind {
        StatementKind::Expr(expr) => dump_expr(expr, indent, out),
        StatementKind::LocalFunction(function) => {
            let suspend = if function.is_suspend { "suspend " } else { "" };
            let type_params = if function.type_params.is_empty() {
                String::new()
            } else {
                let names: Vec<_> = function.type_params.iter().map(dump_type_param).collect();
                format!("<{}>", names.join(", "))
            };
            let params: Vec<_> = function
                .params
                .iter()
                .map(|param| format!("{}: {}", param.name.text, dump_type_ref(&param.ty)))
                .collect();
            let return_ty = function
                .return_ty
                .as_ref()
                .map(|ty| format!(": {}", dump_type_ref(ty)))
                .unwrap_or_default();
            out.push_str(&format!(
                "{pad}{suspend}fun {}{}({}){return_ty}\n",
                function.name.text,
                type_params,
                params.join(", ")
            ));
            match &function.body {
                FunctionBody::Block(block) => dump_block(block, indent + 1, out),
                FunctionBody::Expr(expr) => dump_expr(expr, indent + 1, out),
                FunctionBody::None => {}
            }
        }
        StatementKind::Return { value } => {
            out.push_str(&format!("{pad}return\n"));
            if let Some(value) = value {
                dump_expr(value, indent + 1, out);
            }
        }
        StatementKind::ValDecl(decl) => {
            let keyword = if decl.mutable { "var" } else { "val" };
            let ty = decl.ty.as_ref().map(dump_type_ref);
            let ty = ty.map(|t| format!(": {t}")).unwrap_or_default();
            out.push_str(&format!(
                "{pad}{keyword} {}{ty}\n",
                dump_pattern(&decl.target)
            ));
            dump_expr(&decl.init, indent + 1, out);
        }
        StatementKind::When(when) => {
            out.push_str(&format!("{pad}when\n"));
            dump_expr(&when.subject, indent + 1, out);
            for arm in &when.arms {
                out.push_str(&format!(
                    "{}  arm {}{}\n",
                    pad,
                    dump_pattern(&arm.pattern),
                    if arm.guard.is_some() {
                        " if <guard>"
                    } else {
                        ""
                    }
                ));
                dump_block(&arm.body, indent + 2, out);
            }
            if let Some(else_body) = &when.else_body {
                out.push_str(&format!("{pad}  else\n"));
                dump_block(else_body, indent + 2, out);
            }
        }
        StatementKind::Assign(assign) => {
            match &assign.target {
                AssignTarget::Local(name) => out.push_str(&format!("{pad}assign {}\n", name.text)),
                AssignTarget::Index { .. } => out.push_str(&format!("{pad}assign []\n")),
                AssignTarget::Field { name, .. } => {
                    out.push_str(&format!("{pad}assign .{}\n", name.text))
                }
            }
            match &assign.target {
                AssignTarget::Index {
                    receiver, index, ..
                } => {
                    dump_expr(receiver, indent + 1, out);
                    dump_expr(index, indent + 1, out);
                }
                AssignTarget::Field { receiver, .. } => {
                    dump_expr(receiver, indent + 1, out);
                }
                AssignTarget::Local(_) => {}
            }
            dump_expr(&assign.value, indent + 1, out);
        }
        StatementKind::If(if_) => {
            out.push_str(&format!("{pad}if\n"));
            dump_expr(&if_.cond, indent + 1, out);
            dump_block(&if_.then_block, indent + 1, out);
            if let Some(else_block) = &if_.else_block {
                out.push_str(&format!("{pad}else\n"));
                dump_block(else_block, indent + 1, out);
            }
        }
        StatementKind::Try(try_) => {
            out.push_str(&format!("{pad}try\n"));
            dump_block(&try_.body, indent + 1, out);
            for catch in &try_.catches {
                out.push_str(&format!(
                    "{pad}catch {}: {}\n",
                    catch.name.text,
                    dump_type_ref(&catch.ty)
                ));
                dump_block(&catch.body, indent + 1, out);
            }
            if let Some(finally_body) = &try_.finally_body {
                out.push_str(&format!("{pad}finally\n"));
                dump_block(finally_body, indent + 1, out);
            }
        }
        StatementKind::Throw(expr) => {
            out.push_str(&format!("{pad}throw\n"));
            dump_expr(expr, indent + 1, out);
        }
        StatementKind::While(while_) => {
            out.push_str(&format!("{pad}while\n"));
            dump_expr(&while_.cond, indent + 1, out);
            dump_block(&while_.body, indent + 1, out);
        }
        StatementKind::Block(block) => {
            out.push_str(&format!("{pad}block\n"));
            dump_block(block, indent + 1, out);
        }
        StatementKind::SafetyBlock { mode, block } => {
            let name = match mode {
                SafetyMode::Safe => "Safe",
                SafetyMode::Unsafe => "Unsafe",
            };
            out.push_str(&format!("{pad}@{name} block\n"));
            dump_block(block, indent + 1, out);
        }
    }
}

pub(super) fn dump_type_param(param: &TypeParamDecl) -> String {
    let variance = match param.variance {
        Variance::Invariant => "",
        Variance::In => "in ",
        Variance::Out => "out ",
    };
    let bound = match &param.inline_bound {
        None => String::new(),
        Some(bound) => format!(" : {}", dump_type_bound(bound)),
    };
    format!("{variance}{}{bound}", param.name.text)
}

pub(super) fn dump_type_params(params: &[TypeParamDecl]) -> String {
    if params.is_empty() {
        String::new()
    } else {
        format!(
            "<{}>",
            params
                .iter()
                .map(dump_type_param)
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

fn dump_type_bound(bound: &TypeBound) -> String {
    match bound {
        TypeBound::Kind(TypeParamKindBound::Value) => "value".to_string(),
        TypeBound::Kind(TypeParamKindBound::Ref) => "ref".to_string(),
        TypeBound::Upper(ty) => dump_type_ref(ty),
    }
}

pub(super) fn dump_where_clause(clause: Option<&WhereClause>) -> String {
    let Some(clause) = clause else {
        return String::new();
    };
    format!(
        " where {}",
        clause
            .constraints
            .iter()
            .map(|constraint| format!(
                "{} : {}",
                constraint.parameter.text,
                dump_type_bound(&constraint.bound)
            ))
            .collect::<Vec<_>>()
            .join(", ")
    )
}
