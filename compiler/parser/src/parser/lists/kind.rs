use crate::lexer::TokenKind;
use crate::parser::Parser;

#[derive(Debug, Clone, Copy)]
pub(crate) enum ListKind {
    FunctionParameters,
    CallArguments,
    TypeArguments,
    TypeParameters,
    TypeElements,
    ArrayElements,
    ParenExpressions,
    AnnotationArguments,
    AnnotationElements,
    LambdaParameters,
    PatternElements,
    NamedPatternElements,
}

impl ListKind {
    pub(super) fn close(self) -> TokenKind {
        match self {
            Self::LambdaParameters => TokenKind::Arrow,
            Self::NamedPatternElements => TokenKind::RBrace,
            Self::FunctionParameters
            | Self::CallArguments
            | Self::TypeElements
            | Self::ParenExpressions
            | Self::AnnotationArguments
            | Self::PatternElements => TokenKind::RParen,
            Self::ArrayElements | Self::AnnotationElements => TokenKind::RBracket,
            Self::TypeArguments | Self::TypeParameters => TokenKind::Greater,
        }
    }

    pub(super) fn close_label(self) -> &'static str {
        match self {
            Self::LambdaParameters => "`->`",
            Self::NamedPatternElements => "`}`",
            Self::FunctionParameters
            | Self::CallArguments
            | Self::TypeElements
            | Self::ParenExpressions
            | Self::AnnotationArguments
            | Self::PatternElements => "`)`",
            Self::ArrayElements | Self::AnnotationElements => "`]`",
            Self::TypeArguments | Self::TypeParameters => "`>`",
        }
    }

    pub(super) fn requires_item(self) -> bool {
        matches!(
            self,
            Self::TypeArguments | Self::TypeParameters | Self::LambdaParameters
        )
    }

    pub(super) fn tracks_angles(self) -> bool {
        matches!(
            self,
            Self::TypeArguments | Self::TypeParameters | Self::TypeElements
        )
    }

    pub(super) fn allows_trailing_comma(self) -> bool {
        matches!(
            self,
            Self::TypeElements
                | Self::ParenExpressions
                | Self::AnnotationArguments
                | Self::AnnotationElements
                | Self::PatternElements
                | Self::NamedPatternElements
        )
    }

    pub(super) fn starts_item(self, parser: &Parser) -> bool {
        let token = &parser.peek().kind;
        if matches!(token, TokenKind::Ident(_) | TokenKind::EscapedIdent(_)) {
            return true;
        }
        match self {
            Self::NamedPatternElements => matches!(token, TokenKind::DotDot),
            Self::PatternElements => matches!(
                token,
                TokenKind::DotDot
                    | TokenKind::LParen
                    | TokenKind::Minus
                    | TokenKind::Int(_)
                    | TokenKind::Float(_)
                    | TokenKind::Str(_)
                    | TokenKind::Char(_)
                    | TokenKind::True
                    | TokenKind::False
            ),
            Self::LambdaParameters => matches!(
                token,
                TokenKind::At
                    | TokenKind::LParen
                    | TokenKind::Minus
                    | TokenKind::Int(_)
                    | TokenKind::Float(_)
                    | TokenKind::Str(_)
                    | TokenKind::Char(_)
                    | TokenKind::True
                    | TokenKind::False
            ),
            Self::FunctionParameters => matches!(
                token,
                TokenKind::At | TokenKind::Vararg | TokenKind::Val | TokenKind::Var
            ),
            Self::TypeParameters => matches!(token, TokenKind::In),
            Self::TypeArguments | Self::TypeElements => matches!(
                token,
                TokenKind::At
                    | TokenKind::LParen
                    | TokenKind::Suspend
                    | TokenKind::In
                    | TokenKind::Star
            ),
            Self::CallArguments
            | Self::ArrayElements
            | Self::ParenExpressions
            | Self::AnnotationArguments
            | Self::AnnotationElements => matches!(
                token,
                TokenKind::Str(_)
                    | TokenKind::Char(_)
                    | TokenKind::Int(_)
                    | TokenKind::Float(_)
                    | TokenKind::FStringStart
                    | TokenKind::LParen
                    | TokenKind::LBracket
                    | TokenKind::LBrace
                    | TokenKind::This
                    | TokenKind::True
                    | TokenKind::False
                    | TokenKind::Fun
                    | TokenKind::Suspend
                    | TokenKind::If
                    | TokenKind::When
                    | TokenKind::Return
                    | TokenKind::Break
                    | TokenKind::Continue
                    | TokenKind::Plus
                    | TokenKind::Minus
                    | TokenKind::Bang
                    | TokenKind::Star
                    | TokenKind::DoubleColon
            ),
        }
    }

    pub(super) fn at_follow(self, parser: &Parser) -> bool {
        let token = &parser.peek().kind;
        if matches!(
            token,
            TokenKind::Eof | TokenKind::RBrace | TokenKind::Semicolon
        ) {
            return true;
        }
        match self {
            Self::PatternElements | Self::NamedPatternElements => {
                matches!(token, TokenKind::Arrow | TokenKind::If | TokenKind::Equal)
            }
            Self::LambdaParameters => matches!(
                token,
                TokenKind::Val | TokenKind::Var | TokenKind::Return | TokenKind::Fun
            ),
            Self::FunctionParameters => {
                matches!(
                    token,
                    TokenKind::LBrace
                        | TokenKind::Equal
                        | TokenKind::Colon
                        | TokenKind::Fun
                        | TokenKind::Class
                        | TokenKind::Struct
                        | TokenKind::Interface
                ) || matches!(token, TokenKind::Ident(name) if name == "where")
            }
            Self::AnnotationArguments | Self::AnnotationElements => matches!(
                token,
                TokenKind::RParen
                    | TokenKind::RBracket
                    | TokenKind::At
                    | TokenKind::Fun
                    | TokenKind::Class
                    | TokenKind::Struct
                    | TokenKind::Interface
                    | TokenKind::Val
                    | TokenKind::Var
            ),
            Self::ArrayElements => matches!(
                token,
                TokenKind::RParen
                    | TokenKind::Val
                    | TokenKind::Var
                    | TokenKind::Class
                    | TokenKind::Struct
                    | TokenKind::Interface
            ),
            Self::CallArguments | Self::ParenExpressions => matches!(
                token,
                TokenKind::RBracket
                    | TokenKind::Val
                    | TokenKind::Var
                    | TokenKind::Class
                    | TokenKind::Struct
                    | TokenKind::Interface
            ),
            Self::TypeArguments | Self::TypeElements => matches!(
                token,
                TokenKind::RParen
                    | TokenKind::RBracket
                    | TokenKind::Colon
                    | TokenKind::Equal
                    | TokenKind::Arrow
                    | TokenKind::LBrace
            ),
            Self::TypeParameters => {
                matches!(
                    token,
                    TokenKind::LParen
                        | TokenKind::RParen
                        | TokenKind::Colon
                        | TokenKind::Equal
                        | TokenKind::LBrace
                ) || matches!(token, TokenKind::Ident(_) | TokenKind::EscapedIdent(_))
                    && matches!(
                        parser.tokens.get(parser.pos + 1).map(|token| &token.kind),
                        Some(TokenKind::LParen)
                    )
            }
        }
    }

    pub(super) fn at_recovery_follow(self, parser: &Parser) -> bool {
        self.at_follow(parser)
            && !(matches!(self, Self::FunctionParameters)
                && matches!(parser.peek().kind, TokenKind::Colon | TokenKind::Equal))
    }
}
