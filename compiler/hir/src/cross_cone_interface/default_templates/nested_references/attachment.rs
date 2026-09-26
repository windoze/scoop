//! Selects actual source nodes before checking candidate identities.
use super::*;
use DefaultSourceNestedCallableQueryError as Error;

impl<'a> DefaultSourceNestedCallablesV1<'a> {
    /// Membership supplies no occurrence-dependent ABI or capture facts.
    pub fn require_local_declaration(
        &self,
        declaration: scoop_identity::CallableTemplateOrigin,
    ) -> Result<(), Error> {
        let identity = DefaultNestedCallableIdentityV1::LocalFunction(declaration);
        if self
            .occurrences
            .iter()
            .any(|o| o.descriptor.identity() == identity)
        {
            Ok(())
        } else {
            Err(Error::MissingIdentity(identity))
        }
    }
}
