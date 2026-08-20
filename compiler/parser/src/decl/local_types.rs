use super::*;
use scoop_ast::{AnonymousObjectDecl, Expr, LocalTypeDecl};

impl Parser {
    pub(crate) fn starts_local_type(&self) -> bool {
        let mut tokens = self.tokens[self.pos..].iter().peekable();
        while let Some(token) = tokens.next() {
            match &token.kind {
                TokenKind::At => {
                    if !matches!(
                        tokens.next().map(|token| &token.kind),
                        Some(TokenKind::Ident(_) | TokenKind::EscapedIdent(_))
                    ) {
                        return false;
                    }
                    while matches!(
                        tokens.peek().map(|token| &token.kind),
                        Some(TokenKind::Dot | TokenKind::Colon)
                    ) {
                        tokens.next();
                        if !matches!(
                            tokens.next().map(|token| &token.kind),
                            Some(TokenKind::Ident(_) | TokenKind::EscapedIdent(_))
                        ) {
                            return false;
                        }
                    }
                    if matches!(
                        tokens.peek().map(|token| &token.kind),
                        Some(TokenKind::LParen)
                    ) {
                        tokens.next();
                        let mut depth = 1;
                        for argument in tokens.by_ref() {
                            match argument.kind {
                                TokenKind::LParen => depth += 1,
                                TokenKind::RParen => {
                                    depth -= 1;
                                    if depth == 0 {
                                        break;
                                    }
                                }
                                _ => {}
                            }
                        }
                        if depth != 0 {
                            return false;
                        }
                    }
                }
                TokenKind::Class | TokenKind::Struct | TokenKind::Enum | TokenKind::Interface => {
                    return true;
                }
                TokenKind::Ident(name)
                    if matches!(
                        name.as_str(),
                        "open"
                            | "abstract"
                            | "final"
                            | "sealed"
                            | "inner"
                            | "public"
                            | "internal"
                            | "protected"
                            | "private"
                    ) => {}
                _ => return false,
            }
        }
        false
    }

    pub(crate) fn parse_local_type(&mut self) -> Result<LocalTypeDecl, Diagnostic> {
        let prefix = self.parse_member_prefix()?;
        if let VisibilitySyntax::Explicit { span, .. } = prefix.visibility {
            return Err(Diagnostic::at(
                span,
                "local type declarations cannot have visibility modifiers",
            ));
        }
        if let Some(span) = prefix.modifiers.inner_span {
            return Err(Diagnostic::at(
                span,
                "`inner class` must be declared directly in a class body",
            ));
        }
        match self.parse_nested_nominal(prefix)? {
            NestedTypeDecl::Class(value) => Ok(LocalTypeDecl::Class(value)),
            NestedTypeDecl::Struct(value) => Ok(LocalTypeDecl::Struct(value)),
            NestedTypeDecl::Enum(value) => Ok(LocalTypeDecl::Enum(value)),
            NestedTypeDecl::Interface(value) => Ok(LocalTypeDecl::Interface(value)),
            _ => unreachable!("the nominal parser only accepts the four nominal type keywords"),
        }
    }

    pub(crate) fn parse_anonymous_object(&mut self) -> Result<Expr, Diagnostic> {
        let keyword = self.bump();
        if matches!(
            self.peek().kind,
            TokenKind::Ident(_) | TokenKind::EscapedIdent(_)
        ) {
            return Err(Diagnostic::at(
                self.peek().span,
                "local objects use an anonymous `object` expression",
            ));
        }
        let mut end = keyword.span.end;
        let supertypes = self.parse_supertypes(&mut end)?;
        if !matches!(self.peek().kind, TokenKind::LBrace) {
            return self.unexpected("anonymous object body");
        }
        let (members, end) = self.parse_class_body()?;
        if let Some(constructor) = members.iter().find_map(|member| match member {
            ClassMember::SecondaryConstructor(value) => Some(value),
            _ => None,
        }) {
            return Err(Diagnostic::at(
                constructor.span,
                "anonymous objects cannot declare constructors",
            ));
        }
        Ok(Expr::AnonymousObject(Box::new(AnonymousObjectDecl {
            supertypes,
            members,
            span: Span::new(keyword.span.start, end),
        })))
    }
}
