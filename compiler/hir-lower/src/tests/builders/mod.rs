use super::*;

pub(crate) fn sp() -> Span {
    Span::new(0, 0)
}

/// A span for a trailing `..` rest marker: the split between leading
/// and trailing elements is recovered by span comparison, so a
/// trailing rest must sort after the (zero-span) builder patterns.
pub(crate) fn trailing_rest() -> Span {
    Span::new(u32::MAX - 1, u32::MAX)
}

pub(crate) fn ident(text: &str) -> Ident {
    Ident {
        text: text.to_string(),
        span: sp(),
    }
}

pub(crate) fn type_param(name: &str) -> ast::TypeParamDecl {
    ast::TypeParamDecl {
        name: ident(name),
        inline_bound: None,
        span: sp(),
    }
}

pub(crate) fn ident_at(text: &str, span: Span) -> Ident {
    Ident {
        text: text.to_string(),
        span,
    }
}

pub(crate) fn bare_supertype(ty: TypeRef) -> ast::SupertypeSpec {
    ast::SupertypeSpec {
        ty,
        constructor_arguments: None,
        span: sp(),
    }
}

pub(crate) fn constructor_supertype(
    ty: TypeRef,
    arguments: Vec<ast::CallArgument>,
) -> ast::SupertypeSpec {
    ast::SupertypeSpec {
        ty,
        constructor_arguments: Some(arguments),
        span: sp(),
    }
}

mod enums;
mod expressions;
mod functions;
mod member_expressions;
mod nominals;
mod patterns;
mod source;
mod statements;
mod types;

pub(crate) use enums::*;
pub(crate) use expressions::*;
pub(crate) use functions::*;
pub(crate) use member_expressions::*;
pub(crate) use nominals::*;
pub(crate) use patterns::*;
pub(crate) use source::*;
pub(crate) use statements::*;
pub(crate) use types::*;
