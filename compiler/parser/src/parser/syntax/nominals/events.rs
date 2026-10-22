//! Member syntax recorded by the original nominal productions.

use scoop_ast as ast;
use scoop_syntax::SyntaxKind;

#[derive(Debug)]
pub(in crate::parser::syntax) enum NominalHeader {
    Class(Box<ast::ClassHeader>),
    Struct(Box<ast::StructHeader>),
    Interface(Box<ast::InterfaceHeader>),
    Enum(Box<ast::EnumHeader>),
}

impl NominalHeader {
    pub(super) fn recovered(self, end: u32) -> ast::RecoveredNominal {
        match self {
            Self::Class(header) => {
                let span = ast::Span::new(header.span.start, end);
                ast::RecoveredNominal::Class(Box::new(ast::RecoveredClass {
                    header: *header,
                    span,
                    members: Vec::new(),
                    recovery: Default::default(),
                }))
            }
            Self::Struct(header) => {
                let span = ast::Span::new(header.span.start, end);
                ast::RecoveredNominal::Struct(Box::new(ast::RecoveredStruct {
                    header: *header,
                    span,
                    members: Vec::new(),
                    recovery: Default::default(),
                }))
            }
            Self::Interface(header) => {
                let span = ast::Span::new(header.span.start, end);
                ast::RecoveredNominal::Interface(Box::new(ast::RecoveredInterface {
                    header: *header,
                    span,
                    methods: Vec::new(),
                    properties: Vec::new(),
                    nested: Vec::new(),
                    companion: None,
                    recovery: Default::default(),
                }))
            }
            Self::Enum(header) => {
                let span = ast::Span::new(header.span.start, end);
                ast::RecoveredNominal::Enum(Box::new(ast::RecoveredEnum {
                    header: *header,
                    span,
                    variants: Vec::new(),
                    methods: Vec::new(),
                    properties: Vec::new(),
                    nested: Vec::new(),
                    companion: None,
                    recovery: Default::default(),
                }))
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum NominalMember {
    Class(Box<ast::ClassMember>),
    Struct(Box<ast::StructMember>),
    Function(Box<ast::FunctionDecl>),
    Property(Box<ast::PropertyDecl>),
    Nested(Box<ast::NestedTypeDecl>),
    Companion(Box<ast::CompanionObjectDecl>),
    Variant(Box<ast::VariantDecl>),
}

impl NominalMember {
    pub(crate) fn span(&self) -> ast::Span {
        match self {
            Self::Class(value) => value.span(),
            Self::Struct(value) => value.span(),
            Self::Function(value) => value.span,
            Self::Property(value) => value.span,
            Self::Nested(value) => value.span(),
            Self::Companion(value) => value.span,
            Self::Variant(value) => value.span,
        }
    }
}

#[derive(Debug)]
pub(in crate::parser::syntax) enum NominalEvent {
    Header(NominalHeader),
    BeginMember,
    MemberKind(SyntaxKind),
    EndMember(Option<NominalMember>),
    Unavailable,
}
