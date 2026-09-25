use super::*;
use crate::SourceCallReceiver;

mod parents;
mod relations;

impl Graph<'_> {
    pub(super) fn source_receiver(
        &mut self,
        receiver: SourceCallReceiver<PersistentExactTypeId>,
        meter: &mut BudgetMeter,
    ) -> Result<(), Error> {
        let SourceCallReceiver::Receiver { static_type } = receiver else {
            return Ok(());
        };
        let nominals = crate::collect_type_site_nominals(
            static_type,
            |exact| {
                self.current
                    .identities
                    .canonical_key::<_, ExactTypeKey>(exact)
            },
            meter,
        )
        .map_err(|error| match error {
            crate::HirTypeSiteExactError::Resource(error) => Error::Resource(error),
            crate::HirTypeSiteExactError::Identity(error) => Error::Identity(error),
        })?;
        for owner in nominals {
            let SourceNominalId::Concrete(owner) = owner else {
                return Err(Error::NonConcreteSignature);
            };
            self.select(owner, Kind::Signature, meter)?;
        }
        Ok(())
    }

    pub(super) fn source_extension(
        &self,
        source: &crate::CallableDeclarationRecordV1,
        metadata: SharedTypeMetadataV1<'_>,
        call: &crate::HirDependencyCallSiteV1,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), Error> {
        if source.owner() != crate::PublicDeclarationOwnerV1::Extension {
            return Ok(());
        }
        let signature = source
            .receiver()
            .ok_or(Error::CallableContract(source.declaration()))?;
        let expected = metadata.signature_exact_type(signature, meter)?;
        let invalid = || Error::CallReceiver {
            position: Box::new(call.position()),
            receiver: call.receiver(),
            expected,
        };
        let SourceCallReceiver::Receiver { static_type } = call.receiver() else {
            return Err(invalid());
        };
        if self.receiver_is_subtype(static_type, expected, meter, path)? {
            Ok(())
        } else {
            Err(invalid())
        }
    }
}
