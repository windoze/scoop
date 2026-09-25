use super::*;
use scoop_identity::{CoreBuiltinNominal, ExactTypeKey, PersistentExactTypeId};

pub(super) struct Candidate {
    pub public: hir::CrossConeHirInterfaceSectionV1,
    pub position: hir::concrete::ExecutableExpressionPosition,
    pub receiver: hir::SourceCallReceiver<PersistentExactTypeId>,
    pub expected: PersistentExactTypeId,
}

pub(super) fn mutations(public: &hir::CrossConeHirInterfaceSectionV1) -> Vec<Candidate> {
    let wrong = hir::SourceCallReceiver::Receiver {
        static_type: PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
            CoreBuiltinNominal::Unit.identity_record().id(),
        ))
        .unwrap(),
    };
    let mut result = Vec::new();
    for (reference_index, reference) in public.external_references().records().iter().enumerate() {
        for (call_index, call) in reference.call_sites().records().iter().enumerate() {
            if !call.receiver().has_receiver() {
                continue;
            }
            assert_ne!(call.receiver(), wrong);
            let replacement = hir::HirDependencyCallSiteV1::try_new(
                call.position(),
                call.origin().clone(),
                call.arguments().to_vec(),
                call.result(),
                call.witness_indices().to_vec(),
                wrong,
            )
            .unwrap();
            result.push(Candidate {
                public: changes::call(public, reference_index, call_index, replacement),
                position: call.position(),
                receiver: wrong,
                expected: call.arguments()[0],
            });
        }
    }
    result
}
