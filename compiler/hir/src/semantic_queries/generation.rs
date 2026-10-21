//! Method-body inputs for the compiler driver's synchronous generation step.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AppliedRequirement {
    pub interface: TypeId,
    pub declaration: InterfaceMethodReference,
}

#[derive(Clone, Debug)]
pub struct PendingMethod {
    pub function: FunctionId,
    pub owner: NominalOwner,
    pub requirements: Vec<AppliedRequirement>,
}

/// The parser has accepted one complete ordinary block in this source text.
#[derive(Clone, Debug)]
pub struct GeneratedMethodBody {
    pub function: FunctionId,
    pub body: ast::Block,
    pub source: GeneratedMethodSource,
    pub origin: SemanticSourceLocation,
}

#[derive(Clone, Debug)]
pub struct GeneratedMethodSource {
    pub identity: SourceIdentity,
    pub name: String,
    pub text: scoop_source::SourceText,
}

/// The driver owns process IO and parsing; the frontend owns all binding.
pub trait MethodBodyGenerator {
    fn generate(
        &mut self,
        queries: &mut dyn SemanticQueries,
    ) -> Result<Vec<GeneratedMethodBody>, Vec<Diagnostic>>;
}
