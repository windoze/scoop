use super::*;
use crate::SourceCallReceiver;

mod parents;
mod relations;

impl Graph<'_> {
    pub(super) fn source_receiver(
        &mut self,
        receiver: SourceCallReceiver<PersistentExactTypeId>,
    ) -> Result<(), Error> {
        let SourceCallReceiver::Receiver { static_type } = receiver else {
            return Ok(());
        };
        self.exact_signature_type(static_type)
    }

    pub(super) fn exact_signature_type(
        &mut self,
        static_type: PersistentExactTypeId,
    ) -> Result<(), Error> {
        let nominals = crate::collect_type_site_nominals(static_type, |exact| {
            self.current
                .identities
                .canonical_key::<_, ExactTypeKey>(exact)
        })
        .map_err(|error| match error {
            crate::HirTypeSiteExactError::Resource(error) => Error::Resource(error),
            crate::HirTypeSiteExactError::Identity(error) => Error::Identity(error),
        })?;
        for owner in nominals {
            let SourceNominalId::Concrete(owner) = owner else {
                return Err(Error::NonConcreteSignature);
            };
            let declaration = self
                .current
                .identities
                .canonical_key::<_, SourceDeclarationKey>(owner)?;
            if declaration.origin() != self.current.provider {
                self.select(owner, Kind::Signature)?;
            }
        }
        Ok(())
    }

    pub(super) fn source_extension(
        &self,
        source: &crate::CallableDeclarationRecordV1,
        call: &crate::HirDependencyCallSiteV1,

        path: &WirePath,
    ) -> Result<(), Error> {
        if source.owner() != crate::PublicDeclarationOwnerV1::Extension {
            return Ok(());
        }
        let expected = *call
            .arguments()
            .first()
            .ok_or(Error::CallableContract(source.declaration()))?;
        let invalid = || Error::CallReceiver {
            position: Box::new(call.position()),
            receiver: call.receiver(),
            expected,
        };
        let SourceCallReceiver::Receiver { static_type } = call.receiver() else {
            return Err(invalid());
        };
        if self.receiver_is_subtype(static_type, expected, path)? {
            Ok(())
        } else {
            Err(invalid())
        }
    }
}
