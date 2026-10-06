use scoop_identity::{ExactTypeKey, SourceDeclarationKey};

use super::*;
use crate::{NominalInterfaceRecordV1, SourceNominalId};

mod encoding;
mod members;

/// A borrowed declaration with the receiver's complete substitution.
pub(crate) struct AppliedNominal<'a> {
    pub declaration: &'a NominalInterfaceRecordV1,
    pub arguments: Vec<PersistentExactTypeId>,
}

impl AppliedNominal<'_> {
    pub fn bindings(&self) -> Vec<Vec<PersistentExactTypeId>> {
        if self.arguments.is_empty() {
            Vec::new()
        } else {
            vec![self.arguments.clone()]
        }
    }
}

impl<'a> SharedTypeMetadataV1<'a> {
    pub(crate) fn applied_nominal(
        self,
        exact: PersistentExactTypeId,
        dependencies: impl IntoIterator<Item = Self>,
    ) -> Result<AppliedNominal<'a>, Error> {
        let key = self.identities.canonical_key::<_, ExactTypeKey>(exact)?;
        let (owner, arguments) = match key.as_ref() {
            ExactTypeKey::Nominal(owner) => (SourceNominalId::Concrete(*owner), Vec::new()),
            ExactTypeKey::NominalApplication { origin, arguments } => (
                SourceNominalId::GenericTemplate(*origin),
                arguments.as_slice().to_vec(),
            ),
            _ => return Err(Error::NonConcreteSignature),
        };
        let source = match owner {
            SourceNominalId::Concrete(owner) => self
                .identities
                .canonical_key::<_, SourceDeclarationKey>(owner)?,
            SourceNominalId::GenericTemplate(owner) => self
                .identities
                .canonical_key::<_, SourceDeclarationKey>(owner)?,
        };
        let metadata = std::iter::once(self)
            .chain(dependencies)
            .find(|metadata| metadata.provider == source.origin())
            .ok_or(Error::MissingProvider(source.origin()))?;
        let declaration = metadata
            .public
            .nominal_interfaces()
            .declaration(owner)
            .ok_or(Error::InheritanceSource(owner))?;
        Ok(AppliedNominal {
            declaration,
            arguments,
        })
    }
}

impl<'a> MetadataTypes<'a, '_> {
    pub(crate) fn applied_nominal(
        self,
        exact: PersistentExactTypeId,
    ) -> Result<AppliedNominal<'a>, Error> {
        SharedTypeMetadataV1 {
            identities: self.identity_graph(exact),
            ..self.current
        }
        .applied_nominal(
            exact,
            self.dependencies
                .iter()
                .map(|dependency| dependency.metadata),
        )
    }
}
