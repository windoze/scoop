use super::*;
use scoop_hir::DeclaredVisibilityV1;
use scoop_identity::{DefinitionOriginSubject, SourceDeclarationKind};
use scoop_wire::WirePath;

impl CanonicalCrossConeHirSurfaceAuthority<'_> {
    pub(crate) fn validate_declaration_origin(
        &mut self,
        key: &SourceDeclarationKey,
        subject: DefinitionOriginSubject,
        owner: PublicDeclarationOwnerV1,
        visibility: DeclaredVisibilityV1,
    ) -> Result<(), CrossConeHirNominalAuthorityError> {
        use CrossConeHirNominalAuthorityError as Error;
        let invalid = |reason| Error::DeclarationOrigin { subject, reason };
        let origin = self
            .current_foundation
            .definition_origin(subject)
            .ok_or(Error::MissingDefinitionOrigin { subject })?;
        let source = origin.origin().source();
        let path = WirePath::root();
        self.meter
            .charge_work(
                key.owners().owners().len() as u64
                    + source.logical_path().as_str().len() as u64
                    + 1,
                &path,
            )
            .map_err(Error::Resource)?;
        if key.origin() != self.current
            || source.cone() != self.current
            || key.scope().source().is_some_and(|actual| actual != source)
        {
            return Err(invalid("source origin differs from its typed declaration"));
        }
        if self.source_key_owner("shared declaration", key)? != owner {
            return Err(invalid("declaration has a different typed lexical owner"));
        }
        let owners = key.owners().owners();
        if visibility == DeclaredVisibilityV1::Protected && owners.is_empty() {
            return Err(invalid("protected declaration requires a class owner"));
        }
        for (index, atom) in owners.iter().enumerate() {
            let (nominal, subject) = match atom {
                DefinitionOwnerAtom::Type(id) => (
                    SourceNominalId::Concrete(*id),
                    DefinitionOriginSubject::Type(*id),
                ),
                DefinitionOwnerAtom::GenericType(id) => (
                    SourceNominalId::GenericTemplate(*id),
                    DefinitionOriginSubject::GenericType(*id),
                ),
                _ => {
                    return Err(invalid(
                        "shared declaration has a non-nominal lexical owner",
                    ));
                }
            };
            let owner_key = self.source_nominal_key(nominal)?;
            self.meter
                .charge_work((index + 1) as u64, &path)
                .map_err(Error::Resource)?;
            if owner_key.origin() != key.origin()
                || owner_key.package() != key.package()
                || owner_key.owners().owners() != &owners[..index]
            {
                return Err(invalid(
                    "declaration has an inconsistent lexical owner chain",
                ));
            }
            let owner_origin = self
                .current_foundation
                .definition_origin(subject)
                .ok_or(Error::MissingDefinitionOrigin { subject })?;
            if owner_origin.origin().source() != source {
                return Err(invalid(
                    "declaration belongs to a different source than its lexical owner",
                ));
            }
            if visibility == DeclaredVisibilityV1::Protected
                && index + 1 == owners.len()
                && owner_key.declaration_kind() != SourceDeclarationKind::Class
            {
                return Err(invalid("protected declaration requires a class owner"));
            }
        }
        Ok(())
    }
}
