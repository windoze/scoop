use std::collections::BTreeSet;

use scoop_identity::ExactCallableSignature;

use super::*;
use crate::{CallableDeclarationRecordV1, InheritanceCallableSignatureV1};

impl<'a> SharedTypeMetadataV1<'a> {
    pub(crate) fn applied_member_receiver(
        self,
        root: PersistentExactTypeId,
        owner: SourceNominalId,
        dependencies: &[Self],
    ) -> Result<PersistentExactTypeId, Error> {
        let mut pending = vec![root];
        let mut visited = BTreeSet::new();
        while let Some(exact) = pending.pop() {
            if !visited.insert(exact) {
                continue;
            }
            let application = self.applied_nominal(exact, dependencies.iter().copied())?;
            if application.declaration.declaration() == owner {
                return Ok(exact);
            }
            let bindings = application.bindings();
            for parent in application.declaration.exact_supertypes().values() {
                pending.push(self.signature_exact_type_with_bindings(
                    parent,
                    &bindings,
                    self.identities,
                )?);
            }
        }
        Err(Error::InheritanceSource(owner))
    }

    pub(crate) fn applied_member_signature(
        self,
        receiver: PersistentExactTypeId,
        source: &CallableDeclarationRecordV1,
    ) -> Result<InheritanceCallableSignatureV1, Error> {
        if !source.type_parameters().is_empty() {
            return Err(Error::NonConcreteSignature);
        }
        let key = self.identities.canonical_key::<_, ExactTypeKey>(receiver)?;
        let (owner, bindings) = match key.as_ref() {
            ExactTypeKey::Nominal(owner) => (SourceNominalId::Concrete(*owner), Vec::new()),
            ExactTypeKey::NominalApplication { origin, arguments } => (
                SourceNominalId::GenericTemplate(*origin),
                vec![arguments.as_slice().to_vec()],
            ),
            _ => return Err(Error::NonConcreteSignature),
        };
        if source.owner().nominal_owner() != Some(owner) {
            return Err(Error::CallableContract(source.declaration()));
        }
        let exact = |ty| self.signature_exact_type_with_bindings(ty, &bindings, self.identities);
        let parameters = source
            .parameters()
            .parameters()
            .iter()
            .map(|parameter| exact(parameter.value_type()))
            .collect::<Result<Vec<_>, _>>()?;
        InheritanceCallableSignatureV1::try_new(
            ExactCallableSignature::new(
                source.effects().execution(),
                Some(receiver),
                parameters,
                exact(source.result())?,
            ),
            source.effects(),
        )
        .map_err(|_| Error::CallableContract(source.declaration()))
    }
}
