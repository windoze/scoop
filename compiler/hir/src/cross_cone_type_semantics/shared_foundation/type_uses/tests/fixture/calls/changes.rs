use super::*;

impl Loaded {
    pub fn change_last_argument(&mut self, exact: PersistentExactTypeId) {
        self.change_last_call(|site| {
            let mut arguments = site.arguments().to_vec();
            arguments[0] = exact;
            HirDependencyCallSiteV1::try_new(
                site.position(),
                site.origin().clone(),
                arguments,
                site.result(),
                site.witness_indices().to_vec(),
                site.receiver(),
            )
            .unwrap()
        });
    }

    pub fn change_last_call(
        &mut self,
        change: impl FnOnce(&HirDependencyCallSiteV1) -> HirDependencyCallSiteV1,
    ) {
        let mut references = self.public.external_references().records().to_vec();
        let reference = references
            .iter_mut()
            .find(|reference| reference.call_sites().records().len() > 1)
            .unwrap();
        let mut calls = reference.call_sites().records().to_vec();
        let site = calls.last_mut().unwrap();
        *site = change(site);
        *reference = ExternalHirReferenceV1::try_new(
            reference.origin(),
            reference.target(),
            reference.roles().clone(),
            reference.witnesses().clone(),
            CanonicalHirDependencyCallSitesV1::try_new(calls).unwrap(),
            reference.type_sites().clone(),
        )
        .unwrap();
        self.public = with_tables(
            &self.public,
            self.public.callable_interfaces().clone(),
            CanonicalExternalHirReferencesV1::try_new(references).unwrap(),
        );
    }
}
