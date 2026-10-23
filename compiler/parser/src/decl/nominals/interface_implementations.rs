//! Conditional interface implementation syntax, without a new nominal scope.

use super::*;

impl Parser {
    pub(in crate::decl) fn parse_interface_implementation(
        &mut self,
        mut prefix: MemberPrefix,
        property_context: PropertyContext,
    ) -> Result<InterfaceImplementationDecl, Diagnostic> {
        let start = prefix.modifiers.override_span.take().ok_or_else(|| {
            Diagnostic::at(
                self.peek().span,
                "an interface implementation requires `override`",
            )
        })?;
        self.require_empty_member_prefix(&prefix, "an interface implementation")?;
        if let Some(span) = prefix.modifiers.sealed_span.or(prefix.modifiers.inner_span) {
            return Err(Diagnostic::at(
                span,
                "invalid modifier on an interface implementation",
            ));
        }
        if !prefix.modifiers.context_parameters.is_empty() {
            return Err(Diagnostic::at(
                start,
                "an interface implementation cannot declare context parameters",
            ));
        }
        self.expect("`interface`", |kind| matches!(kind, TokenKind::Interface))?;
        let interface = self.parse_type_ref()?;
        let where_clause = self.parse_where_clause()?.ok_or_else(|| {
            Diagnostic::at(interface.span, "`override interface` requires a non-empty `where` clause; declare an unconditional interface in the type or companion's interface list")
        })?;
        if let Some(requirement) = where_clause.requirements.first() {
            return Err(Diagnostic::at(
                requirement.span,
                "`requires` is not allowed in an interface implementation",
            ));
        }
        if let Some(condition) = where_clause
            .constraints
            .iter()
            .find(|condition| matches!(condition.bound, TypeBound::ExcludedInterface(_)))
        {
            return Err(Diagnostic::at(
                condition.span,
                "interface implementation conditions must be positive",
            ));
        }
        self.expect("`{`", |kind| matches!(kind, TokenKind::LBrace))?;
        let body_depth = self.brace_depth();
        let mut members = Vec::new();
        let close = loop {
            if matches!(self.peek().kind, TokenKind::RBrace) {
                break self.bump();
            }
            if self.at_eof() {
                return self.unexpected("`}`");
            }
            let member_start = self.pos;
            match self.parse_interface_implementation_member(property_context) {
                Ok(member) => {
                    self.attach_documentation(member_start, member.span());
                    members.push(member);
                    if let Err(diagnostic) = self.expect_statement_end() {
                        self.diagnostics.push(diagnostic);
                        self.synchronize_body_item(body_depth, member_start);
                    }
                }
                Err(diagnostic) => {
                    if self.at_eof() {
                        return Err(diagnostic);
                    }
                    self.diagnostics.push(diagnostic);
                    self.synchronize_body_item(body_depth, member_start);
                }
            }
        };
        Ok(InterfaceImplementationDecl {
            interface,
            where_clause,
            members,
            span: Span::new(start.start, close.span.end),
        })
    }

    fn parse_interface_implementation_member(
        &mut self,
        property_context: PropertyContext,
    ) -> Result<InterfaceImplementationMember, Diagnostic> {
        let prefix = self.parse_member_prefix()?;
        if prefix.modifiers.override_span.is_none() {
            return Err(Diagnostic::at(
                self.peek().span,
                "interface implementation members must be `override` methods or computed properties",
            ));
        }
        match self.peek().kind {
            TokenKind::Fun => {
                let function =
                    self.parse_prefixed_member_function(prefix, FunctionContext::TypeBody)?;
                if let Some(parameter) = function.type_params.first() {
                    return Err(Diagnostic::at(
                        parameter.span,
                        "interface implementation methods cannot declare type parameters",
                    ));
                }
                Ok(InterfaceImplementationMember::Function(Box::new(function)))
            }
            TokenKind::Val | TokenKind::Var => {
                let property = self.parse_property(prefix, property_context)?;
                if !matches!(
                    property.body,
                    PropertyBodySyntax::Computed(_) | PropertyBodySyntax::Abstract
                ) {
                    return Err(Diagnostic::at(
                        property.span,
                        "interface implementations cannot declare property storage or delegation",
                    ));
                }
                Ok(InterfaceImplementationMember::Property(Box::new(property)))
            }
            _ => Err(Diagnostic::at(
                self.peek().span,
                "interface implementations contain only override methods and computed properties",
            )),
        }
    }
}
