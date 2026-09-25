use super::*;

pub(super) fn validate(
    bound: &BoundDefaultSourceAccessDeclarationsV1<'_, '_, '_>,
) -> Result<(), Error> {
    for record in bound.source.records() {
        let subject = record.subject();
        let key = bound.source_key(subject)?;
        let access = record.declaration_access();

        let mut replay = Replay { bound, subject };
        access
            .validate_for_declaration(key, &mut replay)
            .map_err(|e| Error::access(subject, e))?;
        if matches!(subject, Subject::Constructor(_)) {
            let owner = access
                .lexical_owners()
                .last()
                .ok_or(Error::ConstructorOwner(subject))?;
            let owner_key = bound.source_key(nominal_subject(*owner))?;
            if !matches!(
                owner_key.declaration_kind(),
                scoop_identity::SourceDeclarationKind::Class
                    | scoop_identity::SourceDeclarationKind::Struct
            ) {
                return Err(Error::ConstructorOwner(subject));
            }
        }
        let nominal = match subject {
            Subject::Type(id) => Some(SourceNominalId::Concrete(id)),
            Subject::GenericType(id) => Some(SourceNominalId::GenericTemplate(id)),
            _ => None,
        };
        if let Some(owner) = nominal {
            let sources = &bound.foundation.source().entries().sources;

            if let Some(existing) = sources.get(owner) {
                if existing.access() != access {
                    return Err(Error::NominalOverlap(owner));
                }
            }
        }
    }
    Ok(())
}

struct Replay<'b, 's, 'a, 'f> {
    bound: &'b BoundDefaultSourceAccessDeclarationsV1<'s, 'a, 'f>,
    subject: Subject,
}
impl ExportDefinitionSourceSemanticAuthority<Error> for Replay<'_, '_, '_, '_> {
    fn current_cone(&self) -> ConeIdentity {
        self.bound.provider()
    }
    fn validate_export_definition_source(
        &mut self,
        source: &ExportDefinitionSourceV1,
    ) -> Result<(), Error> {
        let foundation = self.bound.foundation;
        foundation.validate_origin(source)?;

        let expected = foundation
            .foundation
            .definition_origin(self.subject)
            .ok_or(Error::Origin(self.subject))?;

        if expected.origin() != source.origin() {
            return Err(Error::Origin(self.subject));
        }
        Ok(())
    }
}
impl DeclarationAccessSourceSemanticAuthority<Error> for Replay<'_, '_, '_, '_> {
    fn nominal_declaration_key(
        &self,
        owner: SourceNominalId,
    ) -> Result<&SourceDeclarationKey, Error> {
        let key = self.bound.source_key(nominal_subject(owner))?;

        // DeclarationAccessSourceV1 re-derives each nominal owner identity.

        Ok(key)
    }
    fn nominal_definition_source(
        &self,
        owner: SourceNominalId,
    ) -> Result<&ExportDefinitionSourceV1, Error> {
        let record = self.bound.declaration(nominal_subject(owner))?;
        let origin = record.declaration_access().definition_origin();

        Ok(origin)
    }
}
