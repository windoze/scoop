use super::*;
use crate::parser::ListKind;
use scoop_syntax::SyntaxKind;

impl Parser {
    pub(super) fn parse_anonymous_parameters(&mut self) -> Result<Vec<Param>, Diagnostic> {
        self.with_node(SyntaxKind::ParameterList, |parser| {
            parser.expect("`(`", |kind| matches!(kind, TokenKind::LParen))?;
            let parameters = parser.parse_delimited_items(ListKind::FunctionParameters, |parser| {
                parser.with_node(SyntaxKind::Parameter, Self::parse_anonymous_parameter)
            });
            Ok(parameters.items)
        })
    }

    fn parse_anonymous_parameter(&mut self) -> Result<Param, Diagnostic> {
        let annotations = self.parse_annotations()?;
        self.skip_parameter_compatibility_modifiers();
        let name = self.expect_ident("parameter name")?;
        self.expect("`:`", |kind| matches!(kind, TokenKind::Colon))?;
        let ty = self.parse_type_ref()?;
        Ok(Param {
            annotations,
            span: Span::new(name.span.start, ty.span.end),
            name,
            ty,
            syntax: scoop_ast::ParameterSyntax::Required,
        })
    }

    pub(super) fn parse_lambda_parameter(&mut self) -> Result<LambdaParam, Diagnostic> {
        let annotations = self.parse_annotations()?;
        self.skip_parameter_compatibility_modifiers();
        let target = self.parse_pattern()?;
        let target_span = crate::pattern::pattern_span(&target);
        let ty = if matches!(self.peek().kind, TokenKind::Colon) {
            self.bump();
            Some(self.parse_type_ref()?)
        } else {
            None
        };
        let end = ty.as_ref().map_or(target_span.end, |ty| ty.span.end);
        Ok(LambdaParam {
            annotations,
            target,
            ty,
            span: Span::new(target_span.start, end),
        })
    }
}
