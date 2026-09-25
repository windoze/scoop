use super::*;

pub(super) struct Candidate {
    pub public: hir::CrossConeHirInterfaceSectionV1,
    pub position: hir::concrete::ExecutableExpressionPosition,
    argument_count: bool,
}

impl Candidate {
    pub fn check_error(&self, error: &hir::HirDependencyCallSignatureError) {
        assert!(
            match error {
                hir::HirDependencyCallSignatureError::ArgumentCount { .. } => self.argument_count,
                hir::HirDependencyCallSignatureError::Result { .. } => !self.argument_count,
                _ => false,
            },
            "unexpected source signature rejection: {error:?}"
        );
    }
}

pub(super) fn mutations(public: &hir::CrossConeHirInterfaceSectionV1) -> Vec<Candidate> {
    let unit =
        scoop_identity::PersistentExactTypeId::from_key(&scoop_identity::ExactTypeKey::Nominal(
            scoop_identity::CoreBuiltinNominal::Unit
                .identity_record()
                .id(),
        ))
        .unwrap();
    let (reference_index, reference) = public
        .external_references()
        .records()
        .iter()
        .enumerate()
        .find(|(_, reference)| {
            matches!(
                reference.target(),
                hir::ExternalHirTargetV1::Callable(
                    scoop_identity::CallableTemplateOrigin::Accessor(_)
                )
            ) && reference.call_sites().records().len() > 1
                && reference
                    .call_sites()
                    .records()
                    .iter()
                    .all(|call| call.result() != unit)
        })
        .expect("property fixtures contain repeated getter calls with non-Unit results");
    let mut mutations = Vec::new();
    for call_index in [0, reference.call_sites().records().len() - 1] {
        let call = &reference.call_sites().records()[call_index];
        for argument_count in [false, true] {
            let mut arguments = call.arguments().to_vec();
            let result = if argument_count {
                arguments.push(unit);
                call.result()
            } else {
                unit
            };
            let replacement = hir::HirDependencyCallSiteV1::try_new(
                call.position(),
                call.origin().clone(),
                arguments,
                result,
                call.witness_indices().to_vec(),
                call.receiver(),
            )
            .unwrap();
            mutations.push(Candidate {
                public: changes::call(public, reference_index, call_index, replacement),
                position: call.position(),
                argument_count,
            });
        }
    }
    mutations
}
