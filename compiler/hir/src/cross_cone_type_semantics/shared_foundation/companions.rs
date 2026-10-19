use scoop_identity::ExactTypeKey;

use super::*;
use crate::SourceNominalId;

impl<'a> SharedTypeMetadataV1<'a> {
    pub(super) fn companion_type(
        self,
        host: PersistentExactTypeId,
        nominal_source: &impl Fn(SourceNominalId) -> Option<&'a crate::NominalInterfaceRecordV1>,
    ) -> Result<PersistentExactTypeId, Error> {
        let host_key = self.key(host)?;
        let (owner, arguments) = match host_key.as_ref() {
            ExactTypeKey::Nominal(owner) => (SourceNominalId::Concrete(*owner), Vec::new()),
            ExactTypeKey::NominalApplication { origin, arguments } => (
                SourceNominalId::GenericTemplate(*origin),
                arguments.as_slice().to_vec(),
            ),
            _ => return Err(Error::NonConcreteSignature),
        };
        let declaration = nominal_source(owner).ok_or(Error::InheritanceSource(owner))?;
        for &companion in declaration.declaration_details().children().values() {
            let Some(declaration) = nominal_source(companion) else {
                continue;
            };
            if !matches!(declaration.source_shape(), crate::NominalSourceShapeV1::Object(shape)
                if shape.object_kind() == crate::ObjectSourceKindV1::Companion)
            {
                continue;
            }
            return match companion {
                SourceNominalId::Concrete(id) => self.exact_key(ExactTypeKey::Nominal(id)),
                SourceNominalId::GenericTemplate(id) => {
                    self.exact_key(self.application_key(id, arguments)?)
                }
            };
        }
        Err(Error::Key(
            "the exact host has no public companion declaration".into(),
        ))
    }
}
