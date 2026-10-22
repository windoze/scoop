//! Class declarations, ordered bodies and constructor syntax.

use super::*;
use crate::CallArgument;

/// `class Name(props) : Base(args), I1, I2 { members }` (spec 9.1).
#[derive(Debug, Clone, PartialEq)]
pub struct ClassDecl {
    pub header: ClassHeader,
    /// Class body items preserve property/initializer interleaving.
    pub members: Vec<ClassMember>,
    pub span: Span,
}

/// A complete class header is independent of its member bodies.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassHeader {
    pub annotations: Vec<Annotation>,
    pub visibility: VisibilitySyntax,
    pub modifier: ClassModifier,
    pub inner_span: Option<Span>,
    pub name: Ident,
    /// Generic host parameters (`class Name<T, U> ...`).
    pub type_params: Vec<TypeParamDecl>,
    /// Source primary constructor. Omission is distinct from `()` because a
    /// class that declares secondary constructors may have no primary.
    pub constructor: ClassConstructorDecl,
    /// Supertypes in source order. Nominal kind is resolved by HIR; parser
    /// deliberately does not equate parentheses with "base class".
    pub supertypes: Vec<SupertypeSpec>,
    pub where_clause: Option<WhereClause>,
    pub span: Span,
}

impl ClassDecl {
    pub fn source(&self) -> ClassSource<'_> {
        ClassSource {
            header: &self.header,
            members: &self.members,
            recovery: crate::RecoveredNominalMembers::EMPTY,
            span: self.span,
        }
    }

    pub fn functions(&self) -> impl Iterator<Item = &FunctionDecl> {
        self.source().functions()
    }

    pub fn secondary_constructors(&self) -> impl Iterator<Item = &SecondaryConstructorDecl> {
        self.source().secondary_constructors()
    }
}

impl std::ops::Deref for ClassDecl {
    type Target = ClassHeader;

    fn deref(&self) -> &Self::Target {
        &self.header
    }
}

impl std::ops::DerefMut for ClassDecl {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.header
    }
}

/// Declaration passes borrow only independently completed syntax.
#[derive(Debug, Clone, Copy)]
pub struct ClassSource<'a> {
    pub header: &'a ClassHeader,
    pub members: &'a [ClassMember],
    pub recovery: &'a crate::RecoveredNominalMembers,
    pub span: Span,
}

impl std::ops::Deref for ClassSource<'_> {
    type Target = ClassHeader;

    fn deref(&self) -> &Self::Target {
        self.header
    }
}

impl<'a> ClassSource<'a> {
    pub fn functions(self) -> impl Iterator<Item = &'a FunctionDecl> {
        self.members.iter().filter_map(|member| match member {
            ClassMember::Function(function) => Some(function),
            ClassMember::StoredProperty(_)
            | ClassMember::InitBlock(_)
            | ClassMember::ReleaseBlock(_)
            | ClassMember::SecondaryConstructor(_)
            | ClassMember::Nested(_)
            | ClassMember::Companion(_) => None,
        })
    }

    pub fn secondary_constructors(self) -> impl Iterator<Item = &'a SecondaryConstructorDecl> {
        self.members.iter().filter_map(|member| match member {
            ClassMember::SecondaryConstructor(constructor) => Some(constructor),
            ClassMember::StoredProperty(_)
            | ClassMember::InitBlock(_)
            | ClassMember::ReleaseBlock(_)
            | ClassMember::Function(_)
            | ClassMember::Nested(_)
            | ClassMember::Companion(_) => None,
        })
    }
}

/// The source form of a class primary constructor. Keeping omission distinct
/// from an explicit empty constructor lets HIR validate intrinsic type shapes
/// without reconstructing syntax from an empty property list.
#[derive(Debug, Clone, PartialEq)]
pub enum ClassConstructorDecl {
    Omitted,
    Declared(PrimaryConstructorDecl),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PrimaryConstructorDecl {
    pub annotations: Vec<Annotation>,
    pub visibility: VisibilitySyntax,
    pub parameters: Vec<PrimaryClassParameter>,
    pub span: Span,
}

impl std::ops::Deref for PrimaryConstructorDecl {
    type Target = [PrimaryClassParameter];

    fn deref(&self) -> &Self::Target {
        &self.parameters
    }
}

impl std::ops::DerefMut for PrimaryConstructorDecl {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.parameters
    }
}

impl ClassConstructorDecl {
    pub fn is_omitted(&self) -> bool {
        matches!(self, Self::Omitted)
    }
}

impl std::ops::Deref for ClassConstructorDecl {
    type Target = [PrimaryClassParameter];

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Omitted => &[],
            Self::Declared(constructor) => &constructor.parameters,
        }
    }
}

impl<'a> IntoIterator for &'a ClassConstructorDecl {
    type Item = &'a PrimaryClassParameter;
    type IntoIter = std::slice::Iter<'a, PrimaryClassParameter>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl FromIterator<PrimaryClassParameter> for ClassConstructorDecl {
    fn from_iter<T: IntoIterator<Item = PrimaryClassParameter>>(iter: T) -> Self {
        let parameters = iter.into_iter().collect::<Vec<_>>();
        let span = parameters
            .first()
            .zip(parameters.last())
            .map_or(Span::new(0, 0), |(first, last)| {
                Span::new(first.span.start, last.span.end)
            });
        Self::Declared(PrimaryConstructorDecl {
            annotations: Vec::new(),
            visibility: VisibilitySyntax::Omitted,
            parameters,
            span,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PrimaryClassParameter {
    pub annotations: Vec<Annotation>,
    pub property: PrimaryParameterProperty,
    /// `None` for a plain parameter. Property parameters always retain an
    /// explicit-or-omitted member visibility node.
    pub member_visibility: Option<VisibilitySyntax>,
    pub is_override: bool,
    pub name: Ident,
    pub ty: TypeRef,
    pub syntax: ParameterSyntax,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimaryParameterProperty {
    Plain,
    Val,
    Var,
}

impl PrimaryParameterProperty {
    pub const fn is_property(self) -> bool {
        matches!(self, Self::Val | Self::Var)
    }

    pub const fn is_mutable(self) -> bool {
        matches!(self, Self::Var)
    }
}

/// One syntactic entry after `:` on a nominal declaration. Parentheses are
/// retained even when empty; HIR resolves whether the target is a class or
/// interface and applies the corresponding legality rules.
#[derive(Debug, Clone, PartialEq)]
pub struct SupertypeSpec {
    pub ty: TypeRef,
    pub initialization: SupertypeInitialization,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SupertypeInitialization {
    None,
    Constructor(Vec<CallArgument>),
    Delegate {
        expression: Box<Expr>,
        by_span: Span,
    },
}

impl SupertypeSpec {
    pub fn constructor_arguments(&self) -> Option<&[CallArgument]> {
        match &self.initialization {
            SupertypeInitialization::Constructor(arguments) => Some(arguments),
            SupertypeInitialization::None | SupertypeInitialization::Delegate { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ClassMember {
    StoredProperty(StoredPropertyDecl),
    InitBlock(InitBlockDecl),
    ReleaseBlock(ReleaseBlock),
    SecondaryConstructor(SecondaryConstructorDecl),
    Function(FunctionDecl),
    Nested(Box<NestedTypeDecl>),
    Companion(Box<CompanionObjectDecl>),
}

impl ClassMember {
    pub fn span(&self) -> Span {
        match self {
            Self::StoredProperty(property) => property.span,
            Self::InitBlock(init) => init.span,
            Self::ReleaseBlock(release) => release.span,
            Self::SecondaryConstructor(constructor) => constructor.span,
            Self::Function(function) => function.span,
            Self::Nested(declaration) => declaration.span(),
            Self::Companion(companion) => companion.span,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct InitBlockDecl {
    pub body: Block,
    pub span: Span,
}

/// A receiver-restricted reclamation block, separate from source callables.
#[derive(Debug, Clone, PartialEq)]
pub struct ReleaseBlock {
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SecondaryConstructorDecl {
    pub annotations: Vec<Annotation>,
    pub visibility: VisibilitySyntax,
    pub params: Vec<Param>,
    pub delegation: Option<ConstructorDelegation>,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConstructorDelegation {
    This {
        target_span: Span,
        arguments: Vec<CallArgument>,
        span: Span,
    },
    Super {
        target_span: Span,
        arguments: Vec<CallArgument>,
        span: Span,
    },
}

impl ConstructorDelegation {
    pub fn span(&self) -> Span {
        match self {
            Self::This { span, .. } | Self::Super { span, .. } => *span,
        }
    }

    pub fn arguments(&self) -> &[CallArgument] {
        match self {
            Self::This { arguments, .. } | Self::Super { arguments, .. } => arguments,
        }
    }
}
