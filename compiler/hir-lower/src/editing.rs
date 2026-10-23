//! Declaration facts for an editor revision, without a successful HIR module.

use scoop_ast as ast;
use scoop_hir as hir;
use scoop_identity::ConeIdentity;

use crate::{CoreProtocolInput, Lowerer, SourceKind, SourceProvider};

mod bodies;
mod callables;
pub(crate) mod candidates;
pub(crate) mod constants;
mod constructors;
mod definitions;
mod identities;
mod implementations;
mod imports;
mod members;
mod parameters;
mod properties;
mod references;
mod source_bodies;
pub(crate) mod types;
pub(crate) use bodies::{BodyObservation, ObservedBody};
mod receivers;
pub(crate) use receivers::ReceiverObservation;
pub(crate) use receivers::RecoveredFacts;
#[cfg(test)]
mod tests;

pub type EditingSignature =
    hir::SemanticFunctionSignature<hir::EditingValue<hir::TypeId>, hir::EditingParameter>;

pub struct DeclarationAnalysis {
    pub(crate) lowerer: Lowerer,
    pub(crate) functions: Vec<hir::EditingFunction>,
    pub(crate) declarations_ready: bool,
    pub(crate) unavailable_files: Vec<hir::SemanticSourceLocation>,
    pub(crate) recovered_bodies:
        std::collections::HashMap<hir::FunctionId, ast::RecoveredFunctionBody>,
    pub(crate) recovered_facts: std::collections::HashMap<hir::FunctionId, RecoveredFacts>,
    pub(crate) body_inputs: std::collections::HashMap<hir::FunctionId, ast::FunctionDecl>,
    pub(crate) body_facts:
        std::collections::HashMap<hir::EditingSourceBodyId, hir::EditingValue<ObservedBody>>,
    pub(crate) reference_index: Option<hir::EditingReferenceIndex>,
    pub(crate) type_references: Vec<hir::EditingTypeReference>,
    pub(crate) type_applications: Vec<hir::EditingTypeApplication>,
    pub(crate) constant_references: Vec<hir::EditingConstantReference>,
    pub(crate) static_variant_references: Vec<hir::EditingStaticVariantReference>,
    pub(crate) static_calls: Vec<hir::EditingStaticCall>,
    pub(crate) type_implementations: Option<
        std::collections::HashMap<hir::SourceNominalId, Vec<hir::EditingTypeImplementation>>,
    >,
    pub(crate) member_implementations: Option<hir::EditingMemberImplementationIndex>,
    pub(crate) documentation: Option<std::sync::Arc<hir::SemanticDocumentation>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EditingCancelled;

#[derive(Clone, Debug)]
pub enum EditingAnalysisError {
    Cancelled,
    Input(Vec<ast::Diagnostic>),
}

impl From<Vec<ast::Diagnostic>> for EditingAnalysisError {
    fn from(diagnostics: Vec<ast::Diagnostic>) -> Self {
        Self::Input(diagnostics)
    }
}

/// The selected inputs and dependency world belong to one current Cone.
pub fn analyze_declarations(
    sources: &[ast::DeclarationSource<'_, '_>],
    core: &CoreProtocolInput,
    world: &hir::ImportedSemanticWorld<'_>,
    test_dependencies: &[ConeIdentity],
    is_cancelled: impl Fn() -> bool,
) -> Result<DeclarationAnalysis, EditingAnalysisError> {
    let check = || {
        if is_cancelled() {
            Err(EditingCancelled)
        } else {
            Ok(())
        }
    };
    check().map_err(|_| EditingAnalysisError::Cancelled)?;
    let Some(first) = sources.first() else {
        return Err(vec![input_error(0, "no source files to analyze")].into());
    };
    let current = first.identity.cone();
    crate::current_input::validate_protocol_context(current, core, world)
        .map_err(|error| vec![input_error(0, error.to_string())])?;
    let mut headers = Vec::with_capacity(sources.len());
    for (file, source) in sources.iter().enumerate() {
        check().map_err(|_| EditingAnalysisError::Cancelled)?;
        if source.identity.cone() != current {
            return Err(vec![input_error(
                file,
                "declaration inputs belong to different Cones",
            )]
            .into());
        }
        headers.push(source.header);
    }
    let providers = sources
        .iter()
        .map(|source| SourceProvider {
            provider: hir::IntrinsicProviderId::from_raw(0),
            kind: if current == ConeIdentity::CORE {
                SourceKind::Core
            } else {
                SourceKind::CurrentUnit
            },
            role: source.role,
            identity: source.identity.clone(),
            name: source.identity.logical_path().as_str().to_owned(),
            source: source.text.to_owned(),
        })
        .collect();
    let mut lowerer = crate::current_lowering::prepare_current_lowerer(providers, world)?;
    if sources.iter().any(|source| source.documentation.is_some()) {
        lowerer.documentation = Some(
            crate::documentation::DocumentationContext::new(sources.iter().enumerate().filter_map(
                |(file, source)| {
                    source
                        .documentation
                        .map(|documentation| (file, documentation))
                },
            ))
            .into(),
        );
    }
    crate::current_input::extend_test_dependencies(
        world,
        &mut lowerer.test_dependencies,
        test_dependencies.iter().copied(),
    )
    .map_err(|error| vec![input_error(0, error.to_string())])?;
    let lowerer = match core {
        CoreProtocolInput::CurrentDeclarations => lowerer,
        CoreProtocolInput::Imported(core) => lowerer.with_imported_core(core),
    };
    let mut analysis = lowerer
        .analyze_declaration_inputs(sources, headers, world, &check)
        .map_err(|_| EditingAnalysisError::Cancelled)?;
    for (file, source) in sources.iter().enumerate() {
        check().map_err(|_| EditingAnalysisError::Cancelled)?;
        if source.header.complete().is_none() {
            analysis.lowerer.diagnostics.push(ast::Diagnostic::at_file(
                file,
                source.header.span,
                "the source package or imports are incomplete",
            ));
        }
    }
    Ok(analysis)
}

impl DeclarationAnalysis {
    pub fn documentation(&self) -> Option<&std::sync::Arc<hir::SemanticDocumentation>> {
        self.documentation.as_ref()
    }

    pub fn functions(&self) -> &[hir::EditingFunction] {
        &self.functions
    }

    pub fn diagnostics(&self) -> Vec<ast::Diagnostic> {
        let identities = self.lowerer.diagnostic_source_identities();
        self.lowerer
            .diagnostics
            .iter()
            .chain(&self.lowerer.warnings)
            .cloned()
            .map(|mut diagnostic| {
                diagnostic.resolve_sources(&identities);
                diagnostic
            })
            .collect()
    }

    pub fn signature(&self, function: hir::FunctionId) -> hir::EditingValue<EditingSignature> {
        use hir::{EditingUnavailable as U, EditingValue as V};
        if !self.declarations_ready || !self.lowerer.signatures.contains_key(&function) {
            return V::Unavailable(U::DeclarationFailure);
        }
        if self
            .lowerer
            .declaration_surface
            .unavailable_signature(function)
            .is_some()
        {
            return V::Unavailable(U::SignatureFailure);
        }
        let signature = &self.lowerer.signatures[&function];
        if signature.parameter_pending {
            return V::Unavailable(U::PendingInference);
        }
        let result = if signature.result_pending {
            V::Unavailable(U::PendingInference)
        } else {
            V::Known(signature.return_ty)
        };
        V::Known(self.lowerer.semantic_function_signature(
            function,
            result,
            self.parameters(function),
        ))
    }

    pub fn describe_type(
        &mut self,
        function: hir::FunctionId,
        ty: hir::TypeId,
    ) -> Result<hir::EditingValue<hir::SemanticTypeDescription>, ast::Diagnostic> {
        if let hir::EditingValue::Unavailable(reason) = self.signature(function) {
            return Ok(hir::EditingValue::Unavailable(reason));
        }
        self.lowerer
            .in_context(hir::SemanticContext::Function(function), |lowerer| {
                lowerer
                    .describe_semantic_type(ty)
                    .map(hir::EditingValue::Known)
            })
    }
}

fn input_error(file: usize, message: impl Into<String>) -> ast::Diagnostic {
    ast::Diagnostic::without_span(ast::DiagnosticSeverity::Error, file, message)
}
