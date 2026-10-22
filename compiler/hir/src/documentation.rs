//! Complete source documentation, separate from concrete machine entities.

use crate::{
    AnnotationTargetV1, DeclaredVisibility, DefinitionOrigin, FunctionId, SourceAnnotationTarget,
    TypeId, TypeParamId,
};
use scoop_ast::{Diagnostic, DocText, NonEmptyVec, Span};
use scoop_identity::PersistentAnnotationId;

mod source;
pub use source::*;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DocumentationTarget {
    Declaration(SourceAnnotationTarget),
    TypeParameter {
        owner: SourceAnnotationTarget,
        parameter: TypeParamId,
    },
    ContextParameter {
        owner: SourceAnnotationTarget,
        index: u32,
    },
    AnnotationParameter {
        annotation: PersistentAnnotationId,
        index: u32,
    },
    DerivedFunction(FunctionId),
}

impl From<SourceAnnotationTarget> for DocumentationTarget {
    fn from(target: SourceAnnotationTarget) -> Self {
        Self::Declaration(target)
    }
}

#[derive(Clone, Debug)]
pub struct ConeDocumentation {
    pub declarations: Vec<DocumentationDeclaration>,
    pub warnings: Vec<Diagnostic>,
    pub hidden_annotation: Option<PersistentAnnotationId>,
    pub alias_annotation: Option<PersistentAnnotationId>,
    pub hidden_dependency_bindings: Vec<scoop_identity::BindableEntity>,
    pub source_locations:
        std::collections::HashMap<DocumentationSymbol, crate::SemanticSourceLocation>,
}

#[derive(Clone, Debug)]
pub struct DocumentationDeclaration {
    pub target: DocumentationTarget,
    pub parent: Option<DocumentationTarget>,
    pub kind: DocumentationKind,
    pub name: String,
    pub package: Vec<String>,
    pub origin: DefinitionOrigin,
    pub visibility: DeclaredVisibility,
    pub synthesized: bool,
    pub signature: Vec<DocumentationSignaturePart>,
    /// Names in the original binder scope, including enclosing declarations.
    pub type_parameters: Vec<crate::TypeParamDecl>,
    pub comment: Option<DocumentationComment>,
    pub relations: Vec<DocumentationRelation>,
}

#[derive(Clone, Debug)]
pub struct DocumentationRelation {
    pub kind: DocumentationRelationKind,
    pub target: DocumentationSymbol,
    pub signature: String,
    pub origin: String,
    pub visibility: DeclaredVisibility,
    /// Imported annotations are available even when no dependency page is selected.
    pub hidden: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DocumentationRelationKind {
    Supertype,
    Inherited,
    Overrides,
}

/// Written syntax is preserved verbatim; inferred and generated types always
/// reference the complete Export HIR type, never an inference placeholder.
#[derive(Clone, Debug)]
pub enum DocumentationSignaturePart {
    Text(String),
    Source(Span),
    LinkedSource {
        span: Span,
        symbol: DocumentationSymbol,
    },
    SourceType {
        span: Span,
        ty: TypeId,
    },
    Type(TypeId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DocumentationKind {
    Class,
    Struct,
    Enum,
    Interface,
    Object,
    Annotation,
    TypeAlias,
    Function,
    Constructor,
    Property,
    Field,
    Variant,
    Parameter,
    TypeParameter,
    ContextParameter,
    Getter,
    Setter,
}

#[derive(Clone, Debug)]
pub struct DocumentationComment {
    pub span: Span,
    pub body: DocumentationMarkdown,
    pub tags: Vec<DocumentationTag>,
}

#[derive(Clone, Debug)]
pub struct DocumentationMarkdown {
    pub text: DocText,
    pub definitions: Vec<DocText>,
    pub references: Vec<DocumentationReference>,
}

#[derive(Clone, Debug)]
pub struct DocumentationReference {
    pub range: Span,
    pub resolution: DocumentationLink,
}

#[derive(Clone, Debug)]
pub enum DocumentationLink {
    Symbols(NonEmptyVec<DocumentationSymbol>),
    Url(String),
    /// A warning explains this reference; rendering retains its label.
    Unresolved,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DocumentationSymbol {
    Current(DocumentationTarget),
    Imported(AnnotationTargetV1),
    /// A conditional source declaration, independent of any exact machine use.
    ImportedDerivedEquality(crate::SourceNominalId),
}

#[derive(Clone, Debug)]
pub struct DocumentationTag {
    pub kind: DocumentationTagKind,
    pub label: String,
    pub span: Span,
    pub body: DocumentationMarkdown,
}

#[derive(Clone, Debug)]
pub enum DocumentationTagKind {
    Param(DocumentationTarget),
    Property(DocumentationTarget),
    Constructor(DocumentationTarget),
    Receiver(DocumentationTarget),
    Return(DocumentationTarget),
    Throws(DocumentationSymbol),
    See(DocumentationLink),
    Since,
}
