use super::super::*;
use super::{dump_block, dump_pattern, dump_type_ref};

pub(super) fn dump_expr(expr: &Expr, indent: usize, out: &mut String) {
    let pad = "  ".repeat(indent);
    match expr {
        Expr::StringLiteral { value, .. } => {
            out.push_str(&format!("{pad}StringLiteral {value:?}\n"));
        }
        Expr::IntLiteral { value, .. } => out.push_str(&format!("{pad}IntLiteral {value}\n")),
        Expr::BoolLiteral { value, .. } => out.push_str(&format!("{pad}BoolLiteral {value}\n")),
        Expr::UnitLiteral { .. } => out.push_str(&format!("{pad}UnitLiteral\n")),
        Expr::TupleLiteral { elements, .. } => {
            out.push_str(&format!("{pad}TupleLiteral\n"));
            for element in elements {
                dump_expr(element, indent + 1, out);
            }
        }
        Expr::StructInit { name, args, .. } => {
            out.push_str(&format!("{pad}StructInit {}\n", name.text));
            for arg in args {
                dump_call_argument(arg, indent + 1, out);
            }
        }
        Expr::Var(ident) => out.push_str(&format!("{pad}Var {}\n", ident.text)),
        Expr::Lambda {
            id,
            is_suspend,
            parameters,
            body,
            ..
        } => {
            out.push_str(&format!("{pad}Lambda {} suspend={is_suspend}\n", id.0));
            match parameters {
                None => out.push_str(&format!("{pad}  parameters omitted\n")),
                Some(parameters) => {
                    for parameter in parameters {
                        let ty = parameter
                            .ty
                            .as_ref()
                            .map_or_else(|| "_".to_string(), dump_type_ref);
                        out.push_str(&format!(
                            "{pad}  param {}: {ty}\n",
                            dump_pattern(&parameter.target)
                        ));
                    }
                }
            }
            dump_block(body, indent + 1, out);
        }
        Expr::AnonymousFunction {
            id,
            is_suspend,
            params,
            return_ty,
            body,
            ..
        } => {
            let return_ty = return_ty
                .as_ref()
                .map_or_else(|| "_".to_string(), dump_type_ref);
            out.push_str(&format!(
                "{pad}AnonymousFunction {} suspend={is_suspend} return={return_ty}\n",
                id.0
            ));
            for param in params {
                out.push_str(&format!(
                    "{pad}  param {}: {}\n",
                    param.name.text,
                    dump_type_ref(&param.ty)
                ));
            }
            dump_block(body, indent + 1, out);
        }
        Expr::CallableReference {
            id, receiver, name, ..
        } => {
            out.push_str(&format!("{pad}CallableReference {} {}\n", id.0, name.text));
            if let Some(receiver) = receiver {
                dump_expr(receiver, indent + 1, out);
            }
        }
        Expr::FieldAccess(access) => {
            let selector = match &access.selector {
                FieldSelector::Name(name) => name.text.clone(),
                FieldSelector::Index(index, _) => format!("_{index}"),
            };
            let marker = if access.safe { "?" } else { "" };
            out.push_str(&format!("{pad}FieldAccess {marker}{selector}\n"));
            dump_expr(&access.receiver, indent + 1, out);
        }
        Expr::Call(call) => {
            let type_args = dump_call_type_args(&call.type_args);
            out.push_str(&format!("{pad}Call {}{type_args}\n", call.callee.text));
            for arg in &call.args {
                dump_call_argument(arg, indent + 1, out);
            }
        }
        Expr::Invoke { callee, args, .. } => {
            out.push_str(&format!("{pad}Invoke\n"));
            dump_expr(callee, indent + 1, out);
            for arg in args {
                dump_call_argument(arg, indent + 1, out);
            }
        }
        Expr::Binary { op, lhs, rhs, .. } => {
            out.push_str(&format!("{pad}Binary {op:?}\n"));
            dump_expr(lhs, indent + 1, out);
            dump_expr(rhs, indent + 1, out);
        }
        Expr::Unary { op, operand, .. } => {
            out.push_str(&format!("{pad}Unary {op:?}\n"));
            dump_expr(operand, indent + 1, out);
        }
        Expr::NullAssert { operand, .. } => {
            out.push_str(&format!("{pad}NullAssert\n"));
            dump_expr(operand, indent + 1, out);
        }
        Expr::Elvis { lhs, rhs, .. } => {
            out.push_str(&format!("{pad}Elvis\n"));
            dump_expr(lhs, indent + 1, out);
            dump_expr(rhs, indent + 1, out);
        }
        Expr::This { .. } => out.push_str(&format!("{pad}This\n")),
        Expr::MethodCall {
            receiver,
            name,
            type_args,
            args,
            ..
        } => {
            let type_args = dump_call_type_args(type_args);
            out.push_str(&format!("{pad}MethodCall {}{type_args}\n", name.text));
            dump_expr(receiver, indent + 1, out);
            for arg in args {
                dump_call_argument(arg, indent + 1, out);
            }
        }
        Expr::Is {
            operand,
            ty,
            negated,
            ..
        } => {
            out.push_str(&format!("{pad}Is {} {negated}\n", dump_type_ref(ty)));
            dump_expr(operand, indent + 1, out);
        }
        Expr::Cast {
            operand,
            ty,
            optional,
            ..
        } => {
            out.push_str(&format!(
                "{pad}Cast {} optional={optional}\n",
                dump_type_ref(ty)
            ));
            dump_expr(operand, indent + 1, out);
        }
        Expr::ArrayLiteral { elements, .. } => {
            out.push_str(&format!("{pad}ArrayLiteral\n"));
            for element in elements {
                dump_expr(element, indent + 1, out);
            }
        }
        Expr::Index {
            receiver, index, ..
        } => {
            out.push_str(&format!("{pad}Index\n"));
            dump_expr(receiver, indent + 1, out);
            dump_expr(index, indent + 1, out);
        }
        Expr::If(if_) => {
            out.push_str(&format!("{pad}IfExpression\n"));
            dump_expr(&if_.cond, indent + 1, out);
            dump_block(&if_.then_block, indent + 1, out);
            if let Some(else_block) = &if_.else_block {
                out.push_str(&format!("{pad}  else\n"));
                dump_block(else_block, indent + 1, out);
            }
        }
        Expr::When(when) => {
            out.push_str(&format!("{pad}WhenExpression\n"));
            dump_expr(&when.subject, indent + 1, out);
            for arm in &when.arms {
                out.push_str(&format!("{pad}  arm {}\n", dump_pattern(&arm.pattern)));
                dump_block(&arm.body, indent + 2, out);
            }
            if let Some(else_body) = &when.else_body {
                out.push_str(&format!("{pad}  else\n"));
                dump_block(else_body, indent + 2, out);
            }
        }
        Expr::Try(try_) => {
            out.push_str(&format!("{pad}TryExpression\n"));
            dump_block(&try_.body, indent + 1, out);
            for catch in &try_.catches {
                out.push_str(&format!(
                    "{pad}  catch {}: {}\n",
                    catch.name.text,
                    dump_type_ref(&catch.ty)
                ));
                dump_block(&catch.body, indent + 2, out);
            }
            if let Some(finally_body) = &try_.finally_body {
                out.push_str(&format!("{pad}  finally\n"));
                dump_block(finally_body, indent + 2, out);
            }
        }
    }
}

fn dump_call_argument(argument: &CallArgument, indent: usize, out: &mut String) {
    if matches!(argument.name, CallArgumentName::Positional)
        && matches!(argument.spread, SpreadSyntax::Plain)
    {
        dump_expr(&argument.expression, indent, out);
        return;
    }
    let pad = "  ".repeat(indent);
    let name = match &argument.name {
        CallArgumentName::Positional => String::new(),
        CallArgumentName::Named(name) => format!("{}=", name.text),
    };
    let spread = if matches!(argument.spread, SpreadSyntax::Spread(_)) {
        "*"
    } else {
        ""
    };
    out.push_str(&format!("{pad}Argument {name}{spread}\n"));
    dump_expr(&argument.expression, indent + 1, out);
}

fn dump_call_type_args(type_args: &[TypeRef]) -> String {
    if type_args.is_empty() {
        String::new()
    } else {
        format!(
            "<{}>",
            type_args
                .iter()
                .map(dump_type_ref)
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}
