//! Complete and seal prepared LIR after driver-supplied target classification.
use super::*;
use scoop_identity::{CanonicalExactTypeDiagnosticName, ExactTypeDiagnosticGraph};

impl PreparedLir<'_, '_> {
    pub fn complete(self, plans: lir::AbiPlanSet) -> Result<lir::ConeLirOutput, LirLoweringError> {
        let input = self.input;
        seal(input, lowering::lower_graph(self, plans)?)
    }

    pub fn complete_with_diagnostics(
        self,
        plans: lir::AbiPlanSet,
        diagnostics: &impl ExactTypeDiagnosticGraph,
    ) -> Result<lir::ConeLirOutput, LirLoweringError> {
        let input = self.input;
        let mut lowered = lowering::lower_graph(self, plans)?;
        canonicalize(&mut lowered.module, diagnostics)?;
        seal(input, lowered)
    }
}

fn canonicalize(
    module: &mut lir::Module,
    diagnostics: &impl ExactTypeDiagnosticGraph,
) -> Result<(), LirLoweringError> {
    for (_, descriptor) in module.meta.type_descriptors.iter_mut() {
        descriptor.diagnostic_name = CanonicalExactTypeDiagnosticName::from_validated_graph(
            descriptor.identity.exact_type(),
            diagnostics,
        )
        .map_err(LirLoweringError::Diagnostic)?
        .into_string();
    }
    Ok(())
}

fn seal(
    input: &mir::ConeMirInput,
    lowered: lowering::LoweredModule,
) -> Result<lir::ConeLirOutput, LirLoweringError> {
    lir::ConeLirOutput::try_new(
        lowered.module,
        input
            .materialization()
            .shape_support()
            .iter()
            .map(|root| root.declaration().clone())
            .collect(),
    )
    .map_err(LirLoweringError::Output)
}
