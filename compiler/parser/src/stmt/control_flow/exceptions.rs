use super::*;

enum ParsedCatch {
    Projected {
        clause: CatchClause,
        incomplete: Option<ast::BodyBlockId>,
        complete_header: bool,
    },
    Failed(ast::RecoveredCatch),
}

#[derive(Default)]
struct TryTail {
    catches: Vec<ParsedCatch>,
    finally_body: Option<Block>,
    finally_incomplete: Option<ast::BodyBlockId>,
}

fn recovered_block(body: &Block, incomplete: Option<ast::BodyBlockId>) -> ast::RecoveredTryBlock {
    incomplete.map_or_else(
        || ast::RecoveredTryBlock::Complete(Box::new(body.clone())),
        ast::RecoveredTryBlock::Incomplete,
    )
}

impl Parser {
    /// Handlers attach across newlines and always require a typed parameter.
    pub(crate) fn parse_try(&mut self) -> Result<Statement, Diagnostic> {
        let try_ = self.parse_try_node()?;
        Ok(Statement {
            span: try_.span,
            kind: StatementKind::Try(try_),
        })
    }

    pub(crate) fn parse_try_expression(&mut self) -> Result<Expr, Diagnostic> {
        Ok(Expr::Try(Box::new(self.parse_try_node()?)))
    }

    fn parse_try_node(&mut self) -> Result<Try, Diagnostic> {
        self.with_node(scoop_syntax::SyntaxKind::TryExpr, |parser| {
            parser.parse_try_node_inner()
        })
    }

    fn parse_try_node_inner(&mut self) -> Result<Try, Diagnostic> {
        let checkpoint = self.checkpoint();
        let keyword = self.bump();
        let (result, incomplete) = self.parse_try_block();
        let body = match result {
            Ok(body) => body,
            Err(error) => {
                if let Some(id) = incomplete {
                    self.record_recovered_try(ast::RecoveredTry {
                        block: None,
                        start: keyword.span.start,
                        body: ast::RecoveredTryBlock::Incomplete(id),
                        catches: Vec::new(),
                        finally_body: None,
                    });
                }
                return Err(error);
            }
        };
        let mut tail = TryTail::default();
        let result = self.parse_try_tail(body.span.end, &mut tail);
        if result.is_err() || !self.fragment_is_complete(checkpoint) {
            let catches = tail
                .catches
                .iter()
                .map(|catch| match catch {
                    ParsedCatch::Projected {
                        clause,
                        incomplete,
                        complete_header,
                    } => {
                        if *complete_header {
                            ast::RecoveredCatch::Clause {
                                name: clause.name.clone(),
                                ty: clause.ty.clone(),
                                body: recovered_block(&clause.body, *incomplete),
                                span: clause.span,
                            }
                        } else {
                            ast::RecoveredCatch::Unavailable(clause.span)
                        }
                    }
                    ParsedCatch::Failed(catch) => catch.clone(),
                })
                .collect();
            self.record_recovered_try(ast::RecoveredTry {
                block: None,
                start: keyword.span.start,
                body: recovered_block(&body, incomplete),
                catches,
                finally_body: tail.finally_incomplete,
            });
        }
        let end = result?;
        let catches = tail
            .catches
            .into_iter()
            .map(|catch| match catch {
                ParsedCatch::Projected { clause, .. } => clause,
                ParsedCatch::Failed(_) => {
                    unreachable!("a failed catch prevents a complete try projection")
                }
            })
            .collect();
        Ok(Try {
            body,
            catches,
            finally_body: tail.finally_body,
            span: Span::new(keyword.span.start, end),
        })
    }

    fn parse_try_tail(&mut self, mut end: u32, tail: &mut TryTail) -> Result<u32, Diagnostic> {
        while matches!(&self.peek().kind, TokenKind::Ident(text) if text == "catch") {
            let checkpoint = self.checkpoint();
            let keyword = self.bump();
            self.expect("`(`", |kind| matches!(kind, TokenKind::LParen))?;
            let name = self.expect_ident("catch parameter name")?;
            if !matches!(self.peek().kind, TokenKind::Colon) {
                return Err(Diagnostic::at(
                    self.peek().span,
                    "catch parameter requires a type annotation",
                ));
            }
            self.bump();
            let ty = self.parse_type_ref()?;
            self.expect("`)`", |kind| matches!(kind, TokenKind::RParen))?;
            let complete_header = self.fragment_is_complete(checkpoint);
            let (result, incomplete) = self.parse_try_block();
            match result {
                Ok(body) => {
                    end = body.span.end;
                    tail.catches.push(ParsedCatch::Projected {
                        clause: CatchClause {
                            name,
                            ty,
                            body,
                            span: Span::new(keyword.span.start, end),
                        },
                        incomplete,
                        complete_header,
                    });
                }
                Err(error) => {
                    if complete_header && let Some(id) = incomplete {
                        tail.catches
                            .push(ParsedCatch::Failed(ast::RecoveredCatch::Clause {
                                name,
                                ty,
                                body: ast::RecoveredTryBlock::Incomplete(id),
                                span: Span::new(keyword.span.start, self.peek().span.start),
                            }));
                    }
                    return Err(error);
                }
            }
        }
        if matches!(&self.peek().kind, TokenKind::Ident(text) if text == "finally") {
            self.bump();
            let (result, incomplete) = self.parse_try_block();
            tail.finally_incomplete = incomplete;
            let body = result?;
            end = body.span.end;
            tail.finally_body = Some(body);
        }
        if tail.catches.is_empty() && tail.finally_body.is_none() {
            return self.unexpected("`catch` or `finally`");
        }
        Ok(end)
    }

    fn parse_try_block(&mut self) -> (Result<Block, Diagnostic>, Option<ast::BodyBlockId>) {
        let checkpoint = self.checkpoint();
        let entry = matches!(self.peek().kind, TokenKind::LBrace)
            .then_some(ast::BodyBlockId(self.peek().span.start));
        let result = self.parse_block();
        let incomplete = (result.is_err() || !self.fragment_is_complete(checkpoint))
            .then_some(entry)
            .flatten();
        (result, incomplete)
    }
}
