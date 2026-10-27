//! Snapshot conversions before emitting a contiguous, single-use concat chain.

use super::*;

// Keep in step with the MIR string fusion limits, without a stage dependency.
pub(super) const MAX_PARTS: usize = 8;
const MAX_LITERAL_BYTES: usize = 64;

pub(super) fn normalize(parts: &[ast::StringPart]) -> Vec<ast::StringPart> {
    let mut result = Vec::new();
    for part in parts {
        if let ast::StringPart::Text { value, span } = part {
            if value.is_empty() {
                continue;
            }
            if let Some(ast::StringPart::Text {
                value: previous,
                span: previous_span,
            }) = result.last_mut()
                && previous.len() + value.len() <= MAX_LITERAL_BYTES
            {
                previous.push_str(value);
                previous_span.end = span.end;
                continue;
            }
        }
        result.push(part.clone());
    }
    result
}

impl Lowerer {
    pub(super) fn lower_short_interpolation(
        &mut self,
        parts: &[ast::StringPart],
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let mut strings = Vec::with_capacity(parts.len());
        for part in parts {
            strings.push(match part {
                ast::StringPart::Text { value, span } => self.interpolation_literal(value, *span),
                ast::StringPart::Expression { value, span } => {
                    let value = self.lower_expr(value, sink, None)?;
                    let text = self.interpolation_to_string(value, *span)?;
                    let local = self.alloc_hidden("interpolation", self.string);
                    sink.push(hir::Statement {
                        kind: hir::StatementKind::ValDecl {
                            pattern: hir::Pattern::Binding { local },
                            init: text,
                        },
                        span: *span,
                    });
                    hir::Expr {
                        kind: ExprKind::Local(local),
                        ty: self.string,
                        span: *span,
                        origin: self.expression_origin(*span),
                    }
                }
            });
        }
        if strings.len() > 1 {
            // Coroutine restoration may write earlier conversion locals on
            // several paths. Snapshot them after every conversion so the
            // concat chain reads single-definition locals in its own block.
            for text in &mut strings {
                if !matches!(text.kind, ExprKind::Local(_)) {
                    continue;
                }
                let local = self.alloc_hidden("concatPart", self.string);
                sink.push(hir::Statement {
                    kind: hir::StatementKind::ValDecl {
                        pattern: hir::Pattern::Binding { local },
                        init: text.clone(),
                    },
                    span: text.span,
                });
                text.kind = ExprKind::Local(local);
            }
        }
        let mut strings = strings.into_iter();
        let mut result = strings
            .next()
            .unwrap_or_else(|| self.interpolation_literal("", span));
        for next in strings {
            result = hir::Expr {
                kind: ExprKind::PrimitiveBinary {
                    kind: hir::PrimitiveBinaryKind::StringConcat,
                    lhs: Box::new(result),
                    rhs: Box::new(next),
                },
                ty: self.string,
                span,
                origin: self.expression_origin(span),
            };
        }
        Some(result)
    }

    fn interpolation_literal(&self, value: &str, span: Span) -> hir::Expr {
        hir::Expr {
            kind: ExprKind::StringLiteral {
                value: value.into(),
                owner: hir::StringConstantOwner::CurrentDefinition,
            },
            ty: self.string,
            span,
            origin: self.expression_origin(span),
        }
    }
}
