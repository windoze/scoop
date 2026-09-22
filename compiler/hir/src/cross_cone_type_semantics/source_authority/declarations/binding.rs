use super::*;

mod errors;
pub use errors::*;

impl TypeDeclarationSourceAuthorityV1 {
    /// Build one borrowing source transaction and expose it only after every
    /// table and the independently transported protected inventory agree.
    pub fn with_bound_sources<R>(
        &self,
        foundation: &BoundTypeFoundationSourcesV1<'_>,
        parameters: &CanonicalNominalSourceParameterProtocolsV1,
        core: &ImportedCoreFundamentalTypeProtocol,
        meter: &mut BudgetMeter,
        run: impl FnOnce(&mut BoundNominalDispatchSourcesV1<'_, '_, '_, '_, '_>, &mut BudgetMeter) -> R,
    ) -> Result<R, TypeDeclarationSourceBindingError> {
        let path = WirePath::root();
        meter.check_semantic_depth(1, &path)?;
        meter.charge_nodes(1, &path)?;
        meter.charge_work(9, &path)?;
        let e = self.entries();
        let nominals = foundation.bind_nominal_sources(&e.nominals, meter)?;
        let members = nominals.bind_member_sources(&e.properties, &e.callables, core, meter)?;
        let constructors = nominals.bind_constructor_sources(&e.constructors, meter)?;
        let parameters = members.bind_parameter_protocols(&constructors, parameters, meter)?;
        let dispatch = foundation.bind_inheritance_dispatch_sources(
            &e.inheritance,
            &e.interfaces,
            &e.selections,
            &e.dispatch_callables,
            meter,
        )?;
        let slots = dispatch.bind_slot_sources(meter)?;
        let mut sources = parameters.bind_dispatch_sources(&slots, meter)?;
        let required = sources.required_protected_declarations()?;
        meter.charge_work(
            (required.values().len() as u64 + e.required_protected.values().len() as u64)
                .saturating_mul(128),
            &path,
        )?;
        if required != &e.required_protected {
            return Err(TypeDeclarationSourceBindingError::ProtectedInventory);
        }
        Ok(run(&mut sources, meter))
    }
}
