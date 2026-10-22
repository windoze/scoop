//! Declaration facts for an editor revision, without a successful HIR module.

use scoop_ast as ast;
use scoop_hir as hir;
use scoop_identity::ConeIdentity;

use crate::{CoreProtocolInput, Lowerer, SourceKind, SourceProvider};

mod bodies;
mod callables;
mod parameters;
pub(crate) use bodies::{BodyObservation, CheckedBody};
mod receivers;
pub(crate) use receivers::ReceiverObservation;
#[cfg(test)]
mod tests;

pub type EditingSignature =
    hir::SemanticFunctionSignature<hir::EditingValue<hir::TypeId>, hir::EditingParameter>;

pub struct DeclarationAnalysis {
    pub(crate) lowerer: Lowerer,
    pub(crate) functions: Vec<hir::EditingFunction>,
    pub(crate) declarations_ready: bool,
    pub(crate) recovered_bodies:
        std::collections::HashMap<hir::FunctionId, ast::RecoveredFunctionBody>,
    pub(crate) receiver_facts:
        std::collections::HashMap<hir::FunctionId, Vec<hir::EditingReceiver>>,
    pub(crate) body_inputs: std::collections::HashMap<hir::FunctionId, ast::FunctionDecl>,
    pub(crate) body_facts:
        std::collections::HashMap<hir::FunctionId, hir::EditingValue<CheckedBody>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EditingCancelled;

/// The selected inputs and dependency world belong to one current Cone.
pub fn analyze_declarations(
    sources: &[ast::DeclarationSource<'_, '_>],
    core: &CoreProtocolInput,
    world: &hir::ImportedSemanticWorld<'_>,
    test_dependencies: &[ConeIdentity],
) -> Result<DeclarationAnalysis, Vec<ast::Diagnostic>> {
    let Some(first) = sources.first() else {
        return Err(vec![input_error(0, "no source files to analyze")]);
    };
    let current = first.identity.cone();
    crate::current_input::validate_protocol_context(current, core, world)
        .map_err(|error| vec![input_error(0, error.to_string())])?;
    let mut headers = Vec::with_capacity(sources.len());
    for (file, source) in sources.iter().enumerate() {
        if source.identity.cone() != current {
            return Err(vec![input_error(
                file,
                "declaration inputs belong to different Cones",
            )]);
        }
        let Some(header) = source.header else {
            let mut diagnostic = input_error(file, "the source package or imports are incomplete");
            diagnostic.source = Some(Box::new(source.identity.clone()));
            return Err(vec![diagnostic]);
        };
        headers.push(header);
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
    Ok(lowerer.analyze_declaration_inputs(sources, headers, world))
}

impl DeclarationAnalysis {
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
