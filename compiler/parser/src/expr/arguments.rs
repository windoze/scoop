use super::*;
use scoop_ast::{RecoveredArgumentValue, RecoveredCallArgument, RecoveredCallArguments};

pub(super) struct ParsedArguments {
    pub(super) args: Vec<CallArgument>,
    pub(super) end: u32,
    pub(super) recovery: Option<RecoveredCallArguments>,
    pub(super) complete: bool,
}

enum ArgumentSlot {
    Complete(usize),
    Unavailable(RecoveredCallArgument),
}

impl Parser {
    /// The same argument grammar also serves constructor delegation.
    pub(crate) fn parse_args(&mut self) -> Result<(Vec<CallArgument>, u32), Diagnostic> {
        let parsed = self.parse_arguments()?;
        Ok((parsed.args, parsed.end))
    }

    pub(super) fn parse_arguments(&mut self) -> Result<ParsedArguments, Diagnostic> {
        self.with_node(scoop_syntax::SyntaxKind::ArgumentList, |parser| {
            parser.parse_arguments_inner()
        })
    }

    fn parse_arguments_inner(&mut self) -> Result<ParsedArguments, Diagnostic> {
        let checkpoint = self.checkpoint();
        let opening = self.expect("`(`", |k| matches!(k, TokenKind::LParen))?.span;
        let mut slots = Vec::new();
        let mut parsed_count = 0;
        let parsed = self.parse_delimited_items(crate::parser::ListKind::CallArguments, |parser| {
            let argument = parser.parse_call_argument(&mut slots, parsed_count);
            parsed_count += usize::from(argument.is_ok());
            argument
        });
        let complete = self.fragment_is_complete(checkpoint);
        let recovery =
            (!complete || self.starts_trailing_lambda()).then(|| RecoveredCallArguments {
                opening,
                commas: parsed.commas,
                end: parsed.end,
                closed: parsed.closed,
                items: slots
                    .into_iter()
                    .map(|slot| match slot {
                        ArgumentSlot::Complete(index) => {
                            let argument = &parsed.items[index];
                            RecoveredCallArgument {
                                name: argument.name.clone(),
                                spread: argument.spread,
                                span: argument.span,
                                value: RecoveredArgumentValue::Complete(Box::new(
                                    argument.expression.clone(),
                                )),
                            }
                        }
                        ArgumentSlot::Unavailable(argument) => argument,
                    })
                    .collect(),
            });
        Ok(ParsedArguments {
            args: parsed.items,
            end: parsed.end,
            recovery,
            complete,
        })
    }

    fn parse_call_argument(
        &mut self,
        slots: &mut Vec<ArgumentSlot>,
        index: usize,
    ) -> Result<CallArgument, Diagnostic> {
        self.with_node(scoop_syntax::SyntaxKind::Argument, |parser| {
            parser.parse_call_argument_inner(slots, index)
        })
    }

    fn parse_call_argument_inner(
        &mut self,
        slots: &mut Vec<ArgumentSlot>,
        index: usize,
    ) -> Result<CallArgument, Diagnostic> {
        let checkpoint = self.checkpoint();
        let start = self.peek().span.start;
        let name = if let TokenKind::Ident(_) | TokenKind::EscapedIdent(_) = &self.peek().kind
            && matches!(
                self.tokens.get(self.pos + 1).map(|token| &token.kind),
                Some(TokenKind::Equal)
            ) {
            let name = self.expect_ident("argument name")?;
            self.bump();
            CallArgumentName::Named(name)
        } else {
            CallArgumentName::Positional
        };
        let spread = if matches!(self.peek().kind, TokenKind::Star) {
            SpreadSyntax::Spread(self.bump().span)
        } else {
            SpreadSyntax::Plain
        };
        let expression_start = self.peek().span.start;
        let expression = self.parse_expr();
        let end = expression
            .as_ref()
            .map_or(self.peek().span.start, |expression| expression.span().end);
        let span = Span::new(start, end);
        if expression.is_ok() && self.fragment_is_complete(checkpoint) {
            slots.push(ArgumentSlot::Complete(index));
        } else {
            slots.push(ArgumentSlot::Unavailable(RecoveredCallArgument {
                name: name.clone(),
                spread,
                value: RecoveredArgumentValue::Unavailable(Span::new(expression_start, end)),
                span,
            }));
        }
        Ok(CallArgument {
            span,
            name,
            spread,
            expression: expression?,
        })
    }
}
