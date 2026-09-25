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
            let mut calls = reference.call_sites().records().to_vec();
            calls[call_index] = hir::HirDependencyCallSiteV1::try_new(
                call.position(),
                call.origin().clone(),
                call.arguments().to_vec(),
                call.result(),
                call.witness_indices().to_vec(),
                wrong,
            )
            .unwrap();
            let mut references = public.external_references().records().to_vec();
            references[reference_index] = hir::ExternalHirReferenceV1::try_new(
                reference.origin(),
                reference.target(),
                reference.roles().clone(),
                reference.witnesses().clone(),
                hir::CanonicalHirDependencyCallSitesV1::try_new(calls).unwrap(),
                reference.type_sites().clone(),
            )
            .unwrap();
            result.push(Candidate {
                public: hir::CrossConeHirInterfaceSectionV1::new(
                    public.public_bindings().clone(),
                    public.nominal_interfaces().clone(),
                    public.callable_interfaces().clone(),
                    public.property_interfaces().clone(),
                    public.type_aliases().clone(),
                    public.source_interfaces().clone(),
                    public.default_templates().clone(),
                    public.constants().clone(),
                    public.definition_sources().clone(),
                    hir::CanonicalExternalHirReferencesV1::try_new(references).unwrap(),
                ),
                position: call.position(),
                receiver: wrong,
                expected: call.arguments()[0],
            });
        }
    }
    result
}
