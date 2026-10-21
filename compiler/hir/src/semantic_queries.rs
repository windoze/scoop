//! Read-only frontend facts shared by compilation hooks and language services.
//!
//! Local ids belong to the borrowed frontend. Protocol adapters translate them
//! to their typed declaration identities before sending them to another process.

use crate::*;
use scoop_ast as ast;
use scoop_identity::{ConeIdentity, SignatureTypeKey, SourceIdentity};

mod generation;
pub use generation::*;

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
    pub origin: SemanticSourceLocation,
}

/// Complete type structure and the ordinary direct parent relation.
#[derive(Clone, Debug)]
pub struct SemanticTypeDescription {
    pub ty: TypeId,
    pub kind: Type,
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
    fn pending_methods(&self) -> Vec<PendingMethod>;
    fn annotations(&self) -> &SourceAnnotations;
    fn signature(&self, function: FunctionId) -> SemanticFunctionSignature;
    fn method_identity(
        &mut self,
        function: FunctionId,
    ) -> Result<HirSourceFunctionIdentity, Diagnostic>;
    fn type_signature(
        &self,
        context: SemanticContext,
        ty: TypeId,
    ) -> Result<SignatureTypeKey, Diagnostic>;
    fn describe_type(&mut self, context: SemanticContext, ty: TypeId) -> SemanticTypeDescription;
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
}
