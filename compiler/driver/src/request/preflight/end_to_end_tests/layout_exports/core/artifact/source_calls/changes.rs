use super::*;

pub(super) fn call(
    public: &hir::CrossConeHirInterfaceSectionV1,
    reference_index: usize,
    call_index: usize,
    replacement: hir::HirDependencyCallSiteV1,
) -> hir::CrossConeHirInterfaceSectionV1 {
    let reference = &public.external_references().records()[reference_index];
    let mut calls = reference.call_sites().records().to_vec();
    calls[call_index] = replacement;
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
    hir::CrossConeHirInterfaceSectionV1::new(
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
        public.generic_callable_bodies().clone(),
    )
}
