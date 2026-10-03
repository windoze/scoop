use super::*;
use crate::{InheritanceCallableDeclarationV1 as Member, SourceCallReceiver};
use scoop_identity::{AccessorRole, CallableTemplateOrigin, PropertyAccessorKey, PropertyOwner};

impl Graph<'_> {
    pub(super) fn source_member(
        &mut self,
        source: &crate::CallableDeclarationRecordV1,
        metadata: SharedTypeMetadataV1<'_>,
        call: &crate::HirDependencyCallSiteV1,

        path: &WirePath,
    ) -> Result<(), Error> {
        let Some(owner) = source.owner().nominal_owner() else {
            return Ok(());
        };
        let declaration = match source.declaration() {
            CallableTemplateOrigin::Function(id) => Member::Function(id),
            CallableTemplateOrigin::Accessor(id) => {
                let key = metadata
                    .identities
                    .canonical_key::<_, PropertyAccessorKey>(id)?;
                if !matches!(key.owner(), PropertyOwner::Property(_)) {
                    return Err(Error::NonConcreteSignature);
                }
                match key.role() {
                    AccessorRole::Getter => Member::Getter(id),
                    AccessorRole::Setter => Member::Setter(id),
                }
            }
            CallableTemplateOrigin::Constructor(_)
            | CallableTemplateOrigin::VariantConstructor(_) => return Ok(()),
            CallableTemplateOrigin::GenericFunction(_) => return Err(Error::NonConcreteSignature),
        };
        let SourceNominalId::Concrete(owner) = owner else {
            return Err(Error::NonConcreteSignature);
        };
        let (provider, owner_exact) = self.resolve_nominal(owner)?;
        if provider != metadata.provider {
            return Err(Error::CallableContract(source.declaration()));
        }
        let invalid = || Error::CallReceiver {
            position: Box::new(call.position()),
            receiver: call.receiver(),
            expected: owner_exact,
        };
        let SourceCallReceiver::Receiver { static_type } = call.receiver() else {
            return Err(invalid());
        };

        if !self.receiver_is_subtype(static_type, owner_exact, path)? {
            return Err(invalid());
        }
        // The declaration's provider owns the use, even for a local receiver.
        // Check each occurrence before selected records are deduplicated.
        self.select(
            owner,
            Kind::MemberCall {
                receiver: static_type,
                declaration,
            },
        )
    }
}
