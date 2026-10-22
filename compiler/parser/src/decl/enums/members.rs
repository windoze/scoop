use super::*;

impl Parser {
    pub(super) fn parse_enum_member(
        &mut self,
        annotations: Vec<Annotation>,
        has_companion: bool,
    ) -> Result<NominalMember, Diagnostic> {
        let mut prefix = self.parse_member_prefix()?;
        prefix.annotations.splice(0..0, annotations);
        self.nominal_member_kind();
        match &self.peek().kind {
            TokenKind::Fun => self
                .parse_prefixed_member_function(prefix, FunctionContext::TypeBody)
                .map(Box::new)
                .map(NominalMember::Function),
            TokenKind::Val | TokenKind::Var => self
                .parse_property(prefix, PropertyContext::ValueType)
                .map(Box::new)
                .map(NominalMember::Property),
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
                "`init` blocks are not allowed in enums",
            )),
            TokenKind::Ident(text) if text == "release" => self.invalid_release_owner("enums"),
            TokenKind::Ident(text) if text == "constructor" => Err(Diagnostic::at(
                self.peek().span,
                "secondary constructors are not allowed in enums",
            )),
            TokenKind::Ident(text) if text == "typealias" => self
                .parse_type_alias(prefix)
                .map(Box::new)
                .map(NestedTypeDecl::TypeAlias)
                .map(Box::new)
                .map(NominalMember::Nested),
            _ => Err(Diagnostic::at(
                self.peek().span,
                "expected an enum property, function, nested declaration, or companion object",
            )),
        }
    }
}
