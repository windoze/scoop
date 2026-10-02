use super::*;
use scoop_identity::SignatureTypeKey;

impl Lowerer {
    pub(crate) fn prepare_imported_foreign_callback_protocol(&mut self) -> Result<(), String> {
        let crate::CoreLoweringAuthority::Imported(core) = &self.core else {
            return Ok(());
        };
        let mode = core.foreign_callbacks().mode().persistent();
        let state = core.foreign_callbacks().state().persistent();
        let throwable = core.exceptions().throwable().persistent();
        let option = core.option().option().persistent();
        for signature in [
            SignatureTypeKey::Nominal(mode),
            SignatureTypeKey::Nominal(state),
            SignatureTypeKey::NominalApplication {
                origin: option,
                arguments: scoop_identity::NonEmptyVec::from_first(
                    SignatureTypeKey::Nominal(throwable),
                    [],
                ),
            },
        ] {
            self.imported_signature_type(&signature)
                .map_err(|error| error.diagnostic("foreign callback protocol type"))?;
        }
        Ok(())
    }
}
