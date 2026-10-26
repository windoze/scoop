use crate::syntax::{Input, Token};
use scoop_syntax::SyntaxKind as K;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Gap {
    None,
    Space,
    Line,
    Blank,
}

pub(crate) fn gaps(input: &Input) -> Vec<Gap> {
    let mut gaps = vec![Gap::None; input.tokens.len()];
    for (index, gap) in gaps.iter_mut().enumerate().skip(1) {
        *gap = between(&input.tokens[index - 1], &input.tokens[index]);
    }
    for &(kind, start, end, parent) in &input.nodes {
        if kind == K::Statement
            && parent == Some(K::LambdaExpr)
            && start > 0
            && input.tokens[start - 1].kind() == K::Arrow
        {
            let single_statement = input.tokens[start]
                .syntax
                .parent_ancestors()
                .find(|node| node.kind() == K::LambdaExpr)
                .is_some_and(|node| {
                    node.children()
                        .filter(|child| child.kind() == K::Statement)
                        .count()
                        == 1
                });
            gaps[start] = if single_statement {
                Gap::Space
            } else {
                Gap::Line
            };
            continue;
        }
        let separation = match kind {
            K::Declaration if parent == Some(K::SourceFile) => Some(Gap::Blank),
            K::PackageHeader => Some(Gap::Blank),
            K::ImportHeader => Some(Gap::Line),
            K::WhenArm | K::Member => Some(Gap::Line),
            K::Statement if matches!(parent, Some(K::Block | K::LambdaExpr)) => Some(Gap::Line),
            _ => None,
        };
        if let Some(separation) = separation {
            let mut leading = start;
            while leading > 0
                && input.tokens[leading - 1].is_comment()
                && (leading == 1 || input.tokens[leading - 1].newlines() > 0)
            {
                leading -= 1;
            }
            if leading > 0 {
                gaps[leading] = if gaps[leading] == Gap::Blank {
                    Gap::Blank
                } else {
                    separation
                };
            }
        }
        if kind == K::PackageHeader && end < gaps.len() {
            gaps[end] = Gap::Blank;
        }
        if kind == K::SingleControlBody
            && end < gaps.len()
            && matches!(input.tokens[end].kind(), K::KwElse | K::KwWhile)
        {
            gaps[end] = Gap::Line;
        }
        if kind == K::AnnotationList
            && matches!(parent, Some(K::Declaration | K::Member | K::Statement))
        {
            for &(child, child_start, child_end, _) in &input.nodes {
                if child == K::Annotation
                    && child_start >= start
                    && child_end <= end
                    && child_end < gaps.len()
                {
                    gaps[child_end] = Gap::Line;
                }
            }
        }
    }
    if !gaps.is_empty() {
        gaps[0] = Gap::None;
    }
    gaps
}

fn between(left: &Token, right: &Token) -> Gap {
    use Gap::*;
    let l = left.kind();
    let r = right.kind();
    if left.is_comment() || right.is_comment() {
        return if matches!(l, K::LineComment | K::DocLineComment | K::Shebang)
            || right.newlines() > 0
        {
            if right.newlines() > 1 { Blank } else { Line }
        } else {
            Space
        };
    }
    if matches!(l, K::FStringStart | K::FStringText | K::InterpolationStart)
        || matches!(
            r,
            K::FStringText | K::FStringEnd | K::InterpolationStart | K::InterpolationEnd
        )
    {
        return None;
    }
    let lp = left.parent_kind();
    let rp = right.parent_kind();
    if lp == K::BinaryOperator
        && rp == K::BinaryOperator
        && left.syntax.parent() == right.syntax.parent()
    {
        return None;
    }
    if lp == K::BinaryOperator || rp == K::BinaryOperator {
        return Space;
    }
    // Other newlines can terminate return operands, infix calls, and when-arm bodies.
    // Delimited-list whitespace is relaxed by the layout layer at its own separators.
    if right.newlines() > 0 {
        return if right.newlines() > 1 { Blank } else { Line };
    }
    if rp == K::PostfixOperator || lp == K::PrefixOperator {
        return if would_merge(left, right) {
            Space
        } else {
            None
        };
    }
    if lp == K::PostfixOperator && rp == K::PostfixOperator {
        return None;
    }
    if matches!(
        r,
        K::Comma | K::Semicolon | K::Colon | K::RParen | K::RBracket | K::Question
    ) || matches!(
        l,
        K::Dot | K::QuestionDot | K::DoubleColon | K::At | K::LParen | K::LBracket
    ) || matches!(r, K::Dot | K::QuestionDot | K::DoubleColon)
    {
        return None;
    }
    if r == K::Less && matches!(rp, K::TypeArgumentList | K::TypeParameterList) {
        return if l == K::KwFun { Space } else { None };
    }
    if l == K::Less && matches!(lp, K::TypeArgumentList | K::TypeParameterList)
        || r == K::Greater && matches!(rp, K::TypeArgumentList | K::TypeParameterList)
    {
        return None;
    }
    if r == K::LBracket && rp == K::IndexArguments {
        return None;
    }
    if r == K::LParen {
        return if matches!(
            l,
            K::KwIf | K::KwWhile | K::KwFor | K::KwWhen | K::KwSuspend
        ) {
            Space
        } else if matches!(
            rp,
            K::ArgumentList | K::ParameterList | K::PatternList | K::ContextParameters
        ) || right.before.is_empty()
        {
            None
        } else {
            Space
        };
    }
    if r == K::At
        && matches!(
            l,
            K::KwReturn | K::KwBreak | K::KwContinue | K::Ident | K::EscapedIdent
        )
    {
        return None;
    }
    if l == K::Star && lp != K::BinaryOperator {
        return None;
    }
    if l == K::Semicolon {
        return Line;
    }
    Space
}

fn would_merge(left: &Token, right: &Token) -> bool {
    matches!(
        (left.kind(), right.kind()),
        (K::Plus, K::Plus | K::PlusPlus | K::Equal)
            | (K::Minus, K::Minus | K::MinusMinus | K::Equal | K::Greater)
            | (K::Bang, K::Equal | K::KwIn)
    )
}
