use crate::{Annotation, Block, Expr, Ident, Span, TypeRef};

use super::{MethodModifier, TypeParamDecl, VisibilitySyntax, WhereClause};

/// A complete function keeps its unresolved signature separate from its body.
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionDecl {
    pub signature: FunctionSignature,
    pub body: FunctionBody,
    pub span: Span,
}

impl std::ops::Deref for FunctionDecl {
    type Target = FunctionSignature;

    fn deref(&self) -> &Self::Target {
        &self.signature
    }
}

impl std::ops::DerefMut for FunctionDecl {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.signature
    }
}

/// The body form is known before parsing any expression or statement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionBodyForm {
    Block,
    Expression,
    Omitted,
}

impl FunctionBody {
    pub fn form(&self) -> FunctionBodyForm {
        match self {
            Self::Block(_) => FunctionBodyForm::Block,
            Self::Expr(_) => FunctionBodyForm::Expression,
            Self::None => FunctionBodyForm::Omitted,
        }
    }
}

/// A complete header retained by grammar even when its body has errors.
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionHeader {
    pub signature: FunctionSignature,
    pub body_form: FunctionBodyForm,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionSignature {
    pub context_parameters: Vec<crate::ContextParameter>,
    /// Compiler-recognized annotations in source order (spec 9.4 / M12).
    pub annotations: Vec<Annotation>,
    pub visibility: VisibilitySyntax,
    /// Whether this callable uses the coroutine calling convention.
    pub is_suspend: bool,
    /// `override` (required when overriding, forbidden otherwise).
    pub is_override: bool,
    /// Present exactly when the declaration has an `operator` modifier; the
    /// modifier span is retained for HIR signature/target diagnostics.
    pub operator: Option<OperatorModifier>,
    /// Present exactly when the declaration has an `infix` modifier.
    pub infix: Option<InfixModifier>,
    /// Effective member modality. It is `Final` for top-level
    /// functions, where member modality is not applicable.
    pub modifier: MethodModifier,
    /// Receiver type of a top-level extension function (`fun T.name(...)`).
    /// `None` for ordinary top-level functions, members, and local functions.
    pub receiver_ty: Option<TypeRef>,
    pub name: Ident,
    /// Generic type parameters (`fun <T> f(...)`); empty for
    /// non-generic functions.
    pub type_params: Vec<TypeParamDecl>,
    pub params: Vec<Param>,
    /// An omitted result is inferred for an expression body and is `Unit`
    /// for a block or omitted body.
    pub return_ty: Option<TypeRef>,
    pub where_clause: Option<WhereClause>,
    /// The declaration prefix through its parameter/result/where syntax.
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperatorModifier {
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InfixModifier {
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub annotations: Vec<Annotation>,
    pub name: Ident,
    pub ty: TypeRef,
    pub syntax: ParameterSyntax,
    pub span: Span,
}

/// Source calling shape of one declared value parameter. The variants keep
/// impossible combinations (for example, a required parameter carrying a
/// default expression) out of the AST.
#[derive(Debug, Clone, PartialEq)]
pub enum ParameterSyntax {
    Required,
    Default {
        expression: Expr,
        equals_span: Span,
    },
    Vararg {
        modifier_span: Span,
        default: VarargDefaultSyntax,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum VarargDefaultSyntax {
    EmptyWhenOmitted,
    Expression { expression: Expr, equals_span: Span },
}

#[derive(Debug, Clone, PartialEq)]
pub enum FunctionBody {
    Block(Block),
    /// `fun f(...) [: T] = expr`
    Expr(Box<Expr>),
    /// Bodyless declaration. HIR decides whether the declaration kind permits
    /// the missing body (`abstract`, interface, intrinsic or extern).
    None,
}
