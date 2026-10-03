//! Top-level declaration parsing: annotations, functions, properties,
//! transparent aliases, structs, enums, classes, interfaces and objects.

use scoop_ast::{
    AccessorBodySyntax, AccessorSyntax, Annotation, AnnotationArg, AnnotationLiteral,
    ClassConstructorDecl, ClassDecl, ClassMember, ClassModifier, CompanionNameSyntax,
    CompanionObjectDecl, ConstructorDelegation, Decl, DeclaredVisibility, Diagnostic, EnumDecl,
    FieldDecl, FunctionBody, FunctionDecl, GetterDecl, InfixModifier, InitBlockDecl, InterfaceDecl,
    MethodModifier, NestedNominalDecl, ObjectDecl, OperatorModifier, Param, ParameterSyntax,
    PrimaryClassParameter, PrimaryConstructorDecl, PrimaryParameterProperty, PropertyBodySyntax,
    PropertyDecl, ReleaseBlock, SecondaryConstructorDecl, SetterDecl, SetterParameterSyntax,
    SetterVisibilitySyntax, Span, StructDecl, StructMember, StructRepresentationDecl,
    SupertypeSpec, TypeAliasDecl, TypeBound, TypeConstraint, TypeParamDecl, TypeParamKindBound,
    VarargDefaultSyntax, VariantDecl, VariantDeclKind, VariantFieldDecl, VisibilitySyntax,
    WhereClause,
};

use crate::lexer::TokenKind;
use crate::parser::Parser;

/// Where a `fun` declaration appears; drives the modifier and body rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FunctionContext {
    /// Top-level: no modifiers; a body is required (unless `@Intrinsic`).
    TopLevel,
    /// Class / struct / enum body: method modality and `override`
    /// modifiers are parsed here; owner-specific legality is checked
    /// by hir-lower. `abstract` requires a bodyless declaration.
    TypeBody,
    /// Interface body: signatures and default implementations.
    Interface,
    /// Block-local declaration: no annotations or member modifiers and a
    /// body is mandatory, like a top-level user function.
    Local,
}

/// The modality / `override` modifiers in front of a member `fun`.
/// `start` is the byte offset of the first modifier keyword, for spans.
#[derive(Debug, Default)]
pub(crate) struct Modifiers {
    pub is_suspend: bool,
    pub suspend_span: Option<Span>,
    pub is_override: bool,
    pub override_span: Option<Span>,
    pub operator: Option<OperatorModifier>,
    pub infix: Option<InfixModifier>,
    pub method_modifier: Option<MethodModifier>,
    pub method_modifier_span: Option<Span>,
    pub start: Option<u32>,
}

#[derive(Debug)]
pub(crate) struct MemberPrefix {
    pub annotations: Vec<Annotation>,
    pub visibility: VisibilitySyntax,
    pub modifiers: Modifiers,
    pub is_const: bool,
    pub const_span: Option<Span>,
}

/// Value types implement interfaces only (spec 4.4.3): a supertype with
/// constructor arguments (a base class) is rejected, the rest are
/// interface names.
fn interfaces_only(supertypes: Vec<SupertypeSpec>) -> Result<Vec<scoop_ast::TypeRef>, Diagnostic> {
    let mut interfaces = Vec::with_capacity(supertypes.len());
    for supertype in supertypes {
        if supertype.constructor_arguments.is_some() {
            return Err(Diagnostic::at(
                supertype.span,
                "value types cannot have a base class (spec 4.4)",
            ));
        }
        interfaces.push(supertype.ty);
    }
    Ok(interfaces)
}

mod annotations;
mod enums;
mod functions;
mod generics;
mod nominals;
mod properties;
mod type_aliases;

pub(super) use properties::PropertyContext;

impl Parser {
    pub(super) fn parse_visibility(&mut self) -> Result<VisibilitySyntax, Diagnostic> {
        let Some(visibility) = visibility_token(&self.peek().kind) else {
            return Ok(VisibilitySyntax::Omitted);
        };
        let token = self.bump();
        if visibility_token(&self.peek().kind).is_some() {
            return Err(Diagnostic::at(
                self.peek().span,
                "a declaration may have only one visibility modifier",
            ));
        }
        Ok(VisibilitySyntax::Explicit {
            visibility,
            span: token.span,
        })
    }

    pub(crate) fn parse_decl(&mut self) -> Result<Decl, Diagnostic> {
        let prefix = self.parse_top_level_prefix()?;
        match &self.peek().kind {
            TokenKind::Fun => {
                if prefix.modifiers.method_modifier.is_some() || prefix.modifiers.is_override {
                    return self.unexpected("`class`");
                }
                Ok(Decl::Function(self.parse_prefixed_member_function(
                    prefix,
                    FunctionContext::TopLevel,
                )?))
            }
            TokenKind::Val | TokenKind::Var => {
                if prefix.modifiers.method_modifier.is_some() || prefix.modifiers.is_override {
                    return Err(Diagnostic::at(
                        prefix
                            .modifiers
                            .method_modifier_span
                            .or(prefix.modifiers.start.map(|start| Span::new(start, start)))
                            .expect("top-level member modifier has a span"),
                        "member modality and `override` are not allowed on top-level properties",
                    ));
                }
                Ok(Decl::Global(
                    self.parse_property(prefix, PropertyContext::TopLevel)?,
                ))
            }
            TokenKind::Ident(text) if text == "typealias" => {
                self.parse_type_alias(prefix).map(Decl::TypeAlias)
            }
            TokenKind::Class => {
                self.require_nominal_prefix(&prefix, "class")?;
                let modifier = match prefix.modifiers.method_modifier {
                    Some(MethodModifier::Open) => ClassModifier::Open,
                    Some(MethodModifier::Abstract) => ClassModifier::Abstract,
                    Some(MethodModifier::Final) | None => ClassModifier::Final,
                };
                Ok(Decl::Class(self.parse_class(
                    prefix.annotations,
                    prefix.visibility,
                    modifier,
                    prefix.modifiers.method_modifier_span,
                )?))
            }
            TokenKind::Struct => {
                self.require_unmodified_nominal_prefix(&prefix, "struct")?;
                Ok(Decl::Struct(
                    self.parse_struct(prefix.annotations, prefix.visibility)?,
                ))
            }
            TokenKind::Enum => {
                self.require_unmodified_nominal_prefix(&prefix, "enum")?;
                Ok(Decl::Enum(
                    self.parse_enum(prefix.annotations, prefix.visibility)?,
                ))
            }
            TokenKind::Interface => {
                self.require_unmodified_nominal_prefix(&prefix, "interface")?;
                Ok(Decl::Interface(
                    self.parse_interface(prefix.annotations, prefix.visibility)?,
                ))
            }
            TokenKind::Ident(text) if text == "object" => {
                self.require_unmodified_nominal_prefix(&prefix, "object")?;
                Ok(Decl::Object(
                    self.parse_object(prefix.annotations, prefix.visibility)?,
                ))
            }
            TokenKind::Ident(text) if text == "sealed" => Err(Diagnostic::at(
                self.peek().span,
                "`sealed` classes are not supported yet (milestone M6)",
            )),
            _ => self.unexpected(
                "`fun`, `val`, `var`, `typealias`, `struct`, `enum`, `class` or `interface`",
            ),
        }
    }
}

fn visibility_token(kind: &TokenKind) -> Option<DeclaredVisibility> {
    let TokenKind::Ident(text) = kind else {
        return None;
    };
    Some(match text.as_str() {
        "public" => DeclaredVisibility::Public,
        "internal" => DeclaredVisibility::Internal,
        "private" => DeclaredVisibility::Private,
        "protected" => DeclaredVisibility::Protected,
        _ => return None,
    })
}
