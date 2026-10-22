use super::*;
use crate::parser::NominalMember;

type ParsedInterfaceBody = (
    Vec<FunctionDecl>,
    Vec<PropertyDecl>,
    Vec<NestedTypeDecl>,
    Option<CompanionObjectDecl>,
    u32,
);

impl Parser {
    pub(super) fn parse_interface_body(&mut self) -> Result<ParsedInterfaceBody, Diagnostic> {
        self.with_node(scoop_syntax::SyntaxKind::InterfaceBody, |parser| {
            parser.parse_interface_body_inner()
        })
    }

    fn parse_interface_body_inner(&mut self) -> Result<ParsedInterfaceBody, Diagnostic> {
        self.expect("`{`", |kind| matches!(kind, TokenKind::LBrace))?;
        let body_depth = self.brace_depth();
        let mut methods = Vec::new();
        let mut properties = Vec::new();
        let mut nested = Vec::new();
        let mut companion = None;
        let close = loop {
            if matches!(self.peek().kind, TokenKind::RBrace) {
                break self.bump();
            }
            if matches!(self.peek().kind, TokenKind::Eof) {
                return self.missing_token(TokenKind::RBrace, "`}`");
            }
            let start = self.pos;
            let checkpoint = self.checkpoint();
            self.begin_nominal_member();
            let parsed = self.parse_interface_member(companion.is_some());
            self.end_type_member(&parsed, checkpoint);
            match parsed {
                Ok(member) => {
                    self.attach_documentation(start, member.span());
                    match member {
                        NominalMember::Function(method) => methods.push(*method),
                        NominalMember::Property(property) => properties.push(*property),
                        NominalMember::Nested(declaration) => nested.push(*declaration),
                        NominalMember::Companion(declaration) => companion = Some(*declaration),
                        NominalMember::Class(_)
                        | NominalMember::Struct(_)
                        | NominalMember::Variant(_) => {
                            unreachable!("interface member production")
                        }
                    }
                    if let Err(diagnostic) = self.expect_statement_end() {
                        self.invalidate_nominal_projection();
                        self.diagnostics.push(diagnostic);
                        self.synchronize_body_item(body_depth, start);
                    }
                }
                Err(diagnostic) => {
                    if self.at_eof() {
                        return Err(diagnostic);
                    }
                    self.diagnostics.push(diagnostic);
                    self.synchronize_body_item(body_depth, start);
                }
            }
        };
        Ok((methods, properties, nested, companion, close.span.end))
    }

    fn parse_interface_member(&mut self, has_companion: bool) -> Result<NominalMember, Diagnostic> {
        let prefix = self.parse_member_prefix()?;
        self.nominal_member_kind();
        match &self.peek().kind {
            TokenKind::Val | TokenKind::Var => self
                .parse_property(prefix, PropertyContext::Interface)
                .map(Box::new)
                .map(NominalMember::Property),
            TokenKind::Fun => self
                .parse_prefixed_member_function(prefix, FunctionContext::Interface)
                .map(Box::new)
                .map(NominalMember::Function),
            TokenKind::Struct | TokenKind::Enum | TokenKind::Class | TokenKind::Interface => self
                .parse_nested_nominal(prefix)
                .map(Box::new)
                .map(NominalMember::Nested),
            TokenKind::Ident(text) if text == "annotation" => self
                .parse_annotation_class(prefix)
                .map(Box::new)
                .map(NestedTypeDecl::AnnotationClass)
                .map(Box::new)
                .map(NominalMember::Nested),
            TokenKind::Ident(text) if text == "object" => {
                self.require_unmodified_nominal_prefix(&prefix, "object")?;
                self.parse_object(prefix.annotations, prefix.visibility)
                    .map(Box::new)
                    .map(NestedTypeDecl::Object)
                    .map(Box::new)
                    .map(NominalMember::Nested)
            }
            TokenKind::Ident(text) if text == "companion" => {
                self.require_unmodified_nominal_prefix(&prefix, "companion object")?;
                if has_companion {
                    return Err(Diagnostic::at(
                        self.peek().span,
                        "a nominal declaration may contain at most one companion object",
                    ));
                }
                self.parse_companion_object(prefix.annotations, prefix.visibility)
                    .map(Box::new)
                    .map(NominalMember::Companion)
            }
            TokenKind::Ident(text) if text == "init" => Err(Diagnostic::at(
                self.peek().span,
                "`init` blocks are not allowed in interfaces",
            )),
            TokenKind::Ident(text) if text == "release" => self.invalid_release_owner("interfaces"),
            TokenKind::Ident(text) if text == "constructor" => Err(Diagnostic::at(
                self.peek().span,
                "constructors are not allowed in interfaces",
            )),
            TokenKind::Ident(text) if text == "typealias" => self
                .parse_type_alias(prefix)
                .map(Box::new)
                .map(NestedTypeDecl::TypeAlias)
                .map(Box::new)
                .map(NominalMember::Nested),
            _ => Err(Diagnostic::at(
                self.peek().span,
                "expected an interface function, property, nested declaration, or companion object",
            )),
        }
    }
}
