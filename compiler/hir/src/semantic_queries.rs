//! Read-only frontend facts shared by compilation hooks and language services.
//!
//! Local ids belong to the borrowed frontend. Protocol adapters translate them
//! to their typed declaration identities before sending them to another process.

use crate::*;
use scoop_ast as ast;
use scoop_identity::{ConeIdentity, SignatureTypeKey, SourceIdentity};

mod generation;
pub use generation::*;
mod annotations;
pub use annotations::*;
mod bodies;
pub use bodies::*;
mod defaults;
pub use defaults::*;
mod types;
pub use types::*;
mod fields;
pub use fields::*;
mod properties;
pub use properties::*;
mod parameters;
pub use parameters::*;
mod constructors;
pub use constructors::*;
mod methods;
pub use methods::*;
mod type_context;
pub use type_context::*;

/// A context already established by ordinary declaration binding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SemanticContext {
    Function(FunctionId),
    Nominal(NominalOwner),
}

#[derive(Clone, Debug)]
pub struct SemanticSourceLocation {
    pub source: SourceIdentity,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct SemanticFunctionSignature {
    pub function: FunctionId,
    pub name: String,
    pub receiver: Option<TypeId>,
    pub parameters: Vec<ExportValueParameter>,
    pub type_parameters: Vec<TypeParamDecl>,
    pub owner_parameter_count: usize,
    pub context_parameters: Vec<ContextParameter>,
    pub result: TypeId,
    pub is_suspend: bool,
    pub modifiers: CallableModifiers,
    pub attributes: FunctionAttributes,
    pub release_callability: ReleaseCallability,
    pub origin: SemanticSourceLocation,
}

/// Complete type structure and the ordinary direct parent relation.
#[derive(Clone, Debug)]
pub struct SemanticTypeDescription {
    pub ty: TypeId,
    pub structure: SemanticTypeStructure,
    pub name: String,
    pub parents: Vec<TypeId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SemanticTypeRelation {
    Equal,
    Subtype,
}

/// Operations compute existing semantic facts; they never generate a body.
pub trait SemanticQueries {
    fn current_cone(&self) -> ConeIdentity;
    fn lookup_export(
        &mut self,
        context: SemanticContext,
        provider: ConeIdentity,
        path: &[scoop_identity::CanonicalIdentifier],
        namespace: scoop_identity::BindingNamespace,
    ) -> Result<Vec<scoop_identity::BindableEntity>, Diagnostic>;
    fn pending_methods(&self) -> Vec<PendingMethod>;
    fn annotations(&self) -> &SourceAnnotations;
    fn annotation_declaration(
        &self,
        annotation: scoop_identity::PersistentAnnotationId,
    ) -> Option<SemanticAnnotationDeclaration<'_>>;
    fn describe_annotations(&self, target: SemanticAnnotationTarget) -> SemanticAnnotations<'_>;
    fn resolve_declaration(
        &mut self,
        reference: DecodedAnnotationTargetV1,
    ) -> Result<Option<SemanticDeclaration>, Diagnostic>;
    fn body(
        &self,
        callable: SemanticCallable,
    ) -> Option<SemanticBodyAvailability<SemanticBody<'_>>>;
    fn projected_body(
        &mut self,
        callable: SemanticCallable,
    ) -> Result<
        Option<SemanticBodyAvailability<std::sync::Arc<ExportGenericCallableBodyV1>>>,
        Diagnostic,
    >;
    fn default_source(&self, source: ExportDefaultSourceId) -> SemanticDefaultSource<'_>;
    fn default_definition(
        &self,
        source: SemanticDefaultReference,
    ) -> Option<SemanticDefaultDefinition<'_>>;
    fn projected_default(
        &mut self,
        source: SemanticDefaultReference,
    ) -> Result<Option<std::sync::Arc<ExportDefaultTemplateV1>>, Diagnostic>;
    fn default_application(
        &mut self,
        context: SemanticTypeContext<'_>,
        application: &SemanticDefaultUse,
    ) -> Result<Option<SemanticDefaultApplication>, Diagnostic>;
    fn source_location(&self, origin: DefinitionOrigin) -> SemanticSourceLocation;
    fn signature(&self, function: FunctionId) -> SemanticFunctionSignature;
    fn function_parameters(
        &mut self,
        function: FunctionId,
    ) -> Result<Vec<SemanticParameterDescription>, Diagnostic>;
    fn method_identity(
        &mut self,
        function: FunctionId,
    ) -> Result<HirSourceFunctionIdentity, Diagnostic>;
    fn type_signature(
        &self,
        context: SemanticTypeContext<'_>,
        ty: TypeId,
    ) -> Result<SignatureTypeKey, Diagnostic>;
    fn describe_type(
        &mut self,
        context: SemanticTypeContext<'_>,
        ty: TypeId,
    ) -> Result<SemanticTypeDescription, Diagnostic>;
    /// Includes core nominals whose runtime representation is structural.
    fn nominal_declaration(
        &mut self,
        ty: TypeId,
    ) -> Result<Option<SemanticNominalDescription>, Diagnostic>;
    fn nominal_fields(
        &mut self,
        context: SemanticContext,
        ty: TypeId,
    ) -> Result<Option<SemanticNominalFields>, Diagnostic>;
    fn nominal_properties(
        &mut self,
        context: SemanticContext,
        ty: TypeId,
    ) -> Result<Option<Vec<SemanticPropertyDescription>>, Diagnostic>;
    fn nominal_constructors(
        &mut self,
        context: SemanticContext,
        ty: TypeId,
    ) -> Result<Option<Vec<SemanticConstructorDescription>>, Diagnostic>;
    fn nominal_methods(
        &mut self,
        context: SemanticContext,
        ty: TypeId,
    ) -> Result<Option<Vec<SemanticMethodDescription>>, Diagnostic>;
    fn type_relation(
        &mut self,
        context: SemanticContext,
        relation: SemanticTypeRelation,
        left: TypeId,
        right: TypeId,
    ) -> bool;
    fn interfaces(&mut self, owner: NominalOwner) -> Vec<TypeId>;
    fn requirements(&mut self, interface: TypeId) -> Vec<AppliedRequirement>;
    fn resolve_type(
        &mut self,
        context: SemanticContext,
        syntax: &ast::TypeRef,
    ) -> Result<TypeId, Vec<Diagnostic>>;
    fn resolve_type_reference(
        &mut self,
        context: SemanticTypeContext<'_>,
        reference: scoop_identity::DecodedSignatureTypeKey,
        span: Span,
    ) -> Result<TypeId, Vec<Diagnostic>>;
}
