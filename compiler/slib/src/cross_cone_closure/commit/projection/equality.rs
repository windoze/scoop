//! A generated equality use selects the original provider's real MIR body.

use super::*;
use scoop_identity::{Effect, ExactCallableSignature, StrongCallableDefinitionOwner};

impl ValidatedCrossConeSemanticClosure {
    pub(super) fn project_derived_equalities(
        &self,
        hir: &DependencyHirOutput,
        projected: &mut Vec<SelectedExternalMirCallable>,
    ) -> Result<(), CrossConeMirSelectionProjectionError> {
        let local = hir.output().local.module();
        let boolean = local.exact_type_identities[local.boolean].id();
        for use_ in hir
            .executable_derived_equalities()
            .map_err(CrossConeMirSelectionProjectionError::Occurrences)?
        {
            let provider = use_.provider();
            let target = StrongCallableDefinitionOwner::GeneratedCallable(use_.callable());
            let artifact = self
                .provider(provider)
                .ok_or(CrossConeMirSelectionProjectionError::MissingProvider { provider })?;
            let definition = artifact
                .production()
                .layout()
                .mir_type_bridge()
                .exports()
                .callables()
                .get(target)
                .ok_or(CrossConeMirSelectionProjectionError::MissingExport { provider, target })?;
            let signature = ExactCallableSignature::new(
                Effect::Ordinary,
                Some(use_.owner()),
                vec![use_.owner()],
                boolean,
            );
            if definition.semantic_signature().exact() != &signature
                || definition.semantic_signature().gc_effect() != scoop_mir::GcEffect::Managed
                || definition.lowering_role()
                    != &(scoop_mir::MirCallableLoweringRoleV1::DerivedEquality {
                        owner: use_.owner(),
                    })
            {
                return Err(CrossConeMirSelectionProjectionError::SignatureMismatch {
                    provider,
                    target,
                });
            }
            projected.push(
                SelectedExternalMirCallable::from_lowered(provider, definition.clone()).map_err(
                    |_| CrossConeMirSelectionProjectionError::SignatureMismatch {
                        provider,
                        target,
                    },
                )?,
            );
        }
        Ok(())
    }
}
