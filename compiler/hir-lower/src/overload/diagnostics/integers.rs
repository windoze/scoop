use super::*;

use crate::call_resolution::constraints::ConstraintOrigin;
use crate::overload::probe::{
    ApplicableCandidate, CandidateProbeFailure, CandidateProbeFailureKind,
};

pub(super) fn render_literal_exact_commits(
    candidate: &Candidate,
    transaction: &ApplicableCandidate,
    arguments: &OverloadArguments<'_>,
    receiver_offset: usize,
) -> Option<String> {
    let OverloadArguments::Source(arguments) = arguments else {
        return None;
    };
    let argument_map = candidate
        .argument_map
        .as_ref()
        .expect("an applicable candidate has a complete argument map");
    let commits = arguments
        .iter()
        .enumerate()
        .flat_map(|(source_index, argument)| {
            let source_literal_count = source_integer_literal_count(&argument.expression);
            if source_literal_count == 0 {
                return Vec::new();
            }
            let Some(lowered) = transaction.args.get(receiver_offset + source_index) else {
                return Vec::new();
            };
            let input = crate::call_resolution::arguments::SourceInputId::from_index(source_index);
            let (parameter, _) = argument_map.source_binding(input);
            let parameter = &candidate.view.signature.value_parameters[parameter.index()];
            let mut kinds = Vec::new();
            if let Some(sink) = transaction.argument_sinks.get(source_index) {
                collect_statement_integer_literal_kinds(&transaction.state, sink, &mut kinds);
            }
            collect_integer_literal_kinds(&transaction.state, lowered, &mut kinds);
            kinds.truncate(source_literal_count);
            let multiple = kinds.len() > 1;
            kinds
                .into_iter()
                .enumerate()
                .map(|(index, kind)| {
                    let literal = if multiple {
                        format!(" literal {}", index + 1)
                    } else {
                        String::new()
                    };
                    format!(
                        "argument for `{}`{literal} = {}",
                        parameter.name,
                        kind.canonical_name()
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    (!commits.is_empty()).then(|| commits.join(", "))
}

fn source_integer_literal_count(expression: &ast::Expr) -> usize {
    if crate::expr::integer_literal_default_kind(expression).is_some() {
        return 1;
    }
    match expression {
        ast::Expr::TupleLiteral { elements, .. } | ast::Expr::ArrayLiteral { elements, .. } => {
            elements.iter().map(source_integer_literal_count).sum()
        }
        ast::Expr::Call(call) => call
            .args
            .iter()
            .map(|argument| source_integer_literal_count(&argument.expression))
            .sum(),
        ast::Expr::StructInit { args, .. } => args
            .iter()
            .map(|argument| source_integer_literal_count(&argument.expression))
            .sum(),
        ast::Expr::MethodCall { receiver, args, .. } => {
            source_integer_literal_count(receiver)
                + args
                    .iter()
                    .map(|argument| source_integer_literal_count(&argument.expression))
                    .sum::<usize>()
        }
        _ => 0,
    }
}

fn collect_statement_integer_literal_kinds(
    lowerer: &Lowerer,
    statements: &[hir::Statement],
    kinds: &mut Vec<hir::IntegerKind>,
) {
    for statement in statements {
        match &statement.kind {
            hir::StatementKind::Expr(expression)
            | hir::StatementKind::ValDecl {
                init: expression, ..
            }
            | hir::StatementKind::Assign {
                value: expression, ..
            } => collect_integer_literal_kinds(lowerer, expression, kinds),
            hir::StatementKind::Return {
                value: Some(expression),
            } => collect_integer_literal_kinds(lowerer, expression, kinds),
            hir::StatementKind::If {
                cond,
                then_body,
                else_body,
            } => {
                collect_integer_literal_kinds(lowerer, cond, kinds);
                collect_statement_integer_literal_kinds(lowerer, then_body, kinds);
                if let Some(else_body) = else_body {
                    collect_statement_integer_literal_kinds(lowerer, else_body, kinds);
                }
            }
            hir::StatementKind::While {
                target: _,
                condition_setup,
                cond,
                body,
            } => {
                collect_statement_integer_literal_kinds(lowerer, condition_setup, kinds);
                collect_integer_literal_kinds(lowerer, cond, kinds);
                collect_statement_integer_literal_kinds(lowerer, body, kinds);
            }
            hir::StatementKind::InitializationEnsure(_)
            | hir::StatementKind::GenericDelegateEnsure(_)
            | hir::StatementKind::LocalFunction(_)
            | hir::StatementKind::Return { value: None }
            | hir::StatementKind::When(_)
            | hir::StatementKind::Try(_)
            | hir::StatementKind::Throw(_)
            | hir::StatementKind::Break { .. }
            | hir::StatementKind::Continue { .. } => {}
        }
    }
}

fn collect_integer_literal_kinds(
    lowerer: &Lowerer,
    expression: &hir::Expr,
    kinds: &mut Vec<hir::IntegerKind>,
) {
    let integer_kind = || match lowerer.types[expression.ty] {
        hir::Type::Integer(kind) => Some(kind),
        _ => None,
    };
    match &expression.kind {
        hir::ExprKind::IntegerLiteral(_) | hir::ExprKind::IntegerOperation { .. } => {
            if let Some(kind) = integer_kind() {
                kinds.push(kind);
            }
        }
        hir::ExprKind::TupleLiteral(elements) | hir::ExprKind::ArrayLiteral(elements) => {
            for element in elements {
                collect_integer_literal_kinds(lowerer, element, kinds);
            }
        }
        hir::ExprKind::StructInit { args, .. }
        | hir::ExprKind::ClassInit { args, .. }
        | hir::ExprKind::VariantConstruct { args, .. }
        | hir::ExprKind::Call { args, .. } => {
            for argument in args {
                collect_integer_literal_kinds(lowerer, argument, kinds);
            }
        }
        hir::ExprKind::StructConstruct { fields, .. } => {
            for field in fields {
                collect_integer_literal_kinds(lowerer, field, kinds);
            }
        }
        hir::ExprKind::VariantTest { operand, .. }
        | hir::ExprKind::VariantPayloadProject { operand, .. } => {
            collect_integer_literal_kinds(lowerer, operand, kinds);
        }
        hir::ExprKind::MethodCall { receiver, args, .. } => {
            collect_integer_literal_kinds(lowerer, receiver, kinds);
            for argument in args {
                collect_integer_literal_kinds(lowerer, argument, kinds);
            }
        }
        hir::ExprKind::Box(inner) | hir::ExprKind::SomeWrap(inner) => {
            collect_integer_literal_kinds(lowerer, inner, kinds);
        }
        _ => {}
    }
}

pub(super) fn primitive_integer_conversion_suggestion(
    candidate: &Candidate,
    failure: &CandidateProbeFailure,
    arguments: &OverloadArguments<'_>,
) -> Option<String> {
    if !matches!(
        failure.state.functions[candidate.function].kind,
        hir::FunctionKind::Intrinsic(hir::IntrinsicFunction {
            kind: hir::IntrinsicFunctionKind::Integer(_),
            ..
        })
    ) {
        return None;
    }
    let source_index = match &failure.kind {
        CandidateProbeFailureKind::Expression { source_index, .. } => *source_index,
        CandidateProbeFailureKind::Constraint(constraint) => {
            let ConstraintOrigin::Argument(input) = constraint.origin else {
                return None;
            };
            input.index()
        }
        CandidateProbeFailureKind::Shape(_) | CandidateProbeFailureKind::Intrinsic { .. } => {
            return None;
        }
    };
    let receiver_offset = usize::from(matches!(
        candidate.view.receiver,
        crate::call_resolution::candidates::ReceiverShape::Extension(_)
    ));
    let expected = match &failure.kind {
        CandidateProbeFailureKind::Expression {
            expected: Some(expected),
            ..
        } => *expected,
        _ => *candidate.params.get(receiver_offset + source_index)?,
    };
    let hir::Type::Integer(expected_kind) = failure.state.types[expected] else {
        return None;
    };
    let found_kind = failure
        .arguments
        .get(receiver_offset + source_index)
        .and_then(Option::as_ref)
        .and_then(|argument| match failure.state.types[argument.ty] {
            hir::Type::Integer(kind) => Some(kind),
            _ => None,
        })
        .or_else(|| argument_integer_kind(failure, arguments, source_index))?;
    Lowerer::primitive_integer_conversion_hint(expected_kind, found_kind)
}

impl Lowerer {
    pub(crate) fn primitive_integer_conversion_hint(
        expected: hir::IntegerKind,
        found: hir::IntegerKind,
    ) -> Option<String> {
        (found != expected).then(|| format!(
            "; primitive integer operands require one exact type; convert this operand explicitly with `{}()`",
            integer_conversion_name(expected)
        ))
    }
}

fn argument_integer_kind(
    failure: &CandidateProbeFailure,
    arguments: &OverloadArguments<'_>,
    source_index: usize,
) -> Option<hir::IntegerKind> {
    match arguments {
        OverloadArguments::Lowered(arguments) => {
            match failure.state.types[arguments.get(source_index)?.ty] {
                hir::Type::Integer(kind) => Some(kind),
                _ => None,
            }
        }
        OverloadArguments::Source(arguments) => {
            let expression = &arguments.get(source_index)?.expression;
            crate::expr::integer_literal_default_kind(expression)
        }
    }
}

const fn integer_conversion_name(kind: hir::IntegerKind) -> &'static str {
    match kind {
        hir::IntegerKind::SIGNED_8 => "toInt8",
        hir::IntegerKind::SIGNED_16 => "toInt16",
        hir::IntegerKind::SIGNED_32 => "toInt32",
        hir::IntegerKind::SIGNED_64 => "toInt64",
        hir::IntegerKind::UNSIGNED_8 => "toUInt8",
        hir::IntegerKind::UNSIGNED_16 => "toUInt16",
        hir::IntegerKind::UNSIGNED_32 => "toUInt32",
        hir::IntegerKind::UNSIGNED_64 => "toUInt64",
    }
}
