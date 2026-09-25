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

        run: impl FnOnce(&mut BoundNominalDispatchSourcesV1<'_, '_, '_, '_, '_>) -> R,
    ) -> Result<R, TypeDeclarationSourceBindingError> {
        let e = self.entries();
        let nominals = foundation.bind_nominal_sources(&e.nominals)?;
        let members = nominals.bind_member_sources(&e.properties, &e.callables, core)?;
        let constructors = nominals.bind_constructor_sources(&e.constructors)?;
        let parameters = members.bind_parameter_protocols(&constructors, parameters)?;
        let dispatch = foundation.bind_inheritance_dispatch_sources(
            &e.inheritance,
            &e.interfaces,
            &e.selections,
            &e.dispatch_callables,
        )?;
        let slots = dispatch.bind_slot_sources()?;
        let mut sources = parameters.bind_dispatch_sources(&slots)?;
        let required = sources.required_protected_declarations()?;

        if required != &e.required_protected {
            return Err(TypeDeclarationSourceBindingError::ProtectedInventory);
        }
        Ok(run(&mut sources))
    }
}
