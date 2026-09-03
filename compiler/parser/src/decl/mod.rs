//! Top-level declaration parsing: annotations, functions, structs, enums,
//! classes and interfaces.

use scoop_ast::{
    Annotation, AnnotationArg, AnnotationLiteral, ClassConstructorDecl, ClassDecl, ClassModifier,
    ConstructorProp, Decl, Diagnostic, EnumDecl, Expr, FieldDecl, FunctionBody, FunctionDecl,
    GlobalDecl, InterfaceDecl, MethodModifier, OperatorModifier, Param, Span, StructDecl,
    StructRepresentationDecl, TypeBound, TypeConstraint, TypeParamDecl, TypeParamKindBound,
    Variance, VariantDecl, VariantDeclKind, VariantFieldDecl, WhereClause,
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
    /// Interface body: method signatures only — bodies (default
    /// implementations) are outside the M6 subset.
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
    pub operator: Option<OperatorModifier>,
    pub method_modifier: Option<MethodModifier>,
    pub method_modifier_span: Option<Span>,
    pub start: Option<u32>,
}

/// The parsed supertype list of a class: an optional base class with its
/// constructor arguments (`: Base(args)`) plus the implemented interfaces.
#[derive(Debug, Default)]
pub(crate) struct Supertypes {
    pub base_class: Option<(scoop_ast::TypeRef, Vec<Expr>)>,
    pub interfaces: Vec<scoop_ast::TypeRef>,
}

/// Value types implement interfaces only (spec 4.4.3): a supertype with
/// constructor arguments (a base class) is rejected, the rest are
/// interface names.
fn interfaces_only(supertypes: Supertypes) -> Result<Vec<scoop_ast::TypeRef>, Diagnostic> {
    if let Some((base, _)) = supertypes.base_class {
        return Err(Diagnostic::at(
            base.span,
            "value types cannot have a base class (spec 4.4)",
        ));
    }
    Ok(supertypes.interfaces)
}

mod annotations;
mod enums;
mod functions;
mod generics;
mod nominals;

impl Parser {
    pub(crate) fn parse_decl(&mut self) -> Result<Decl, Diagnostic> {
        match &self.peek().kind {
            TokenKind::Suspend | TokenKind::Fun => Ok(Decl::Function(
                self.parse_non_member_function(Vec::new(), FunctionContext::TopLevel)?,
            )),
            TokenKind::Val | TokenKind::Var => Ok(Decl::Global(self.parse_global(Vec::new())?)),
            TokenKind::Struct => Ok(Decl::Struct(self.parse_struct(Vec::new())?)),
            TokenKind::Enum => Ok(Decl::Enum(self.parse_enum(Vec::new())?)),
            TokenKind::Class => Ok(Decl::Class(self.parse_class(
                Vec::new(),
                ClassModifier::Final,
                None,
            )?)),
            TokenKind::Ident(text) if text == "operator" => Ok(Decl::Function(
                self.parse_non_member_function(Vec::new(), FunctionContext::TopLevel)?,
            )),
            TokenKind::Ident(text) if text == "open" || text == "abstract" => {
                let modifier = if text == "open" {
                    ClassModifier::Open
                } else {
                    ClassModifier::Abstract
                };
                let keyword = self.bump();
                Ok(Decl::Class(self.parse_class(
                    Vec::new(),
                    modifier,
                    Some(keyword.span),
                )?))
            }
            TokenKind::Interface => Ok(Decl::Interface(self.parse_interface(Vec::new())?)),
            TokenKind::At => {
                let annotations = self.parse_annotations()?;
                match &self.peek().kind {
                    TokenKind::Fun | TokenKind::Suspend => Ok(Decl::Function(
                        self.parse_non_member_function(annotations, FunctionContext::TopLevel)?,
                    )),
                    TokenKind::Ident(text) if text == "operator" => Ok(Decl::Function(
                        self.parse_non_member_function(annotations, FunctionContext::TopLevel)?,
                    )),
                    TokenKind::Val | TokenKind::Var => {
                        Ok(Decl::Global(self.parse_global(annotations)?))
                    }
                    TokenKind::Struct => Ok(Decl::Struct(self.parse_struct(annotations)?)),
                    TokenKind::Enum => Ok(Decl::Enum(self.parse_enum(annotations)?)),
                    TokenKind::Class => Ok(Decl::Class(self.parse_class(
                        annotations,
                        ClassModifier::Final,
                        None,
                    )?)),
                    TokenKind::Interface => Ok(Decl::Interface(self.parse_interface(annotations)?)),
                    _ => Err(Diagnostic::at(
                        self.peek().span,
                        "annotations are only allowed on declarations or safety blocks",
                    )),
                }
            }
            TokenKind::Ident(text) if text == "sealed" => Err(Diagnostic::at(
                self.peek().span,
                "`sealed` classes are not supported yet (milestone M6)",
            )),
            TokenKind::Ident(text) if text == "object" => Err(Diagnostic::at(
                self.peek().span,
                "`object` declarations are not supported yet (milestone M6)",
            )),
            _ => self.unexpected("`fun`, `val`, `var`, `struct`, `enum`, `class` or `interface`"),
        }
    }
}
