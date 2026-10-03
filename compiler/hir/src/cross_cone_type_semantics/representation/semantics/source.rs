use super::*;
use crate::SourceNominalId;
use scoop_identity::{DefinitionOwnerAtom, DuplicateSignatureKey};
use scoop_wire::WireError;

pub(super) enum Failure {
    Resource(WireError),
    Mismatch(NominalRepresentationSourceMismatchV1),
}
impl From<WireError> for Failure {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl From<NominalRepresentationSourceMismatchV1> for Failure {
    fn from(error: NominalRepresentationSourceMismatchV1) -> Self {
        Self::Mismatch(error)
    }
}

pub(super) fn validate(
    record: &NominalRepresentationSupportV1,
    expected: NominalRepresentationSourceV1<'_>,
    provider: ConeIdentity,

    path: &WirePath,
) -> Result<(), Failure> {
    use NominalRepresentationSourceMismatchV1 as Mismatch;
    validate_header(
        record,
        expected.key,
        expected.access,
        provider,
        expected.shape.source_kind(),
    )?;
    if !compare::shape(record.shape(), expected.shape, &path.clone().field(3))? {
        return Err(Mismatch::Shape.into());
    }
    if let NominalRepresentationPublicSourceShapeV1::PublicSourceShape(public) =
        expected.public_source_shape
        && !record.public_source_shape_matches(public, &path.clone().field(3))?
    {
        return Err(Mismatch::PublicSourceShape.into());
    }
    Ok(())
}

pub(super) fn validate_header(
    record: &NominalRepresentationSupportV1,
    key: &SourceDeclarationKey,
    access: &DeclarationAccessSourceV1,
    provider: ConeIdentity,
    source_kind: scoop_identity::SourceDeclarationKind,
) -> Result<(), Failure> {
    use NominalRepresentationSourceMismatchV1 as Mismatch;
    if !key.declaration_kind().is_nominal()
        || !matches!(
            key.duplicate_signature(),
            DuplicateSignatureKey::Nominal {
                type_parameter_count: 0
            }
        )
    {
        return Err(Mismatch::SourceKey.into());
    }
    if key.origin() != provider {
        return Err(Mismatch::Provider.into());
    }

    if PersistentTypeId::from_source_declaration(key).ok() != Some(record.owner()) {
        return Err(Mismatch::OwnerIdentity.into());
    }
    if key.declaration_kind() != source_kind {
        return Err(Mismatch::SourceKind.into());
    }

    let source = access.definition_origin().origin().source();
    if source.cone() != provider || key.scope().source().is_some_and(|scope| scope != source) {
        return Err(Mismatch::AccessSource.into());
    }
    if !owners_match(key, access.lexical_owners()) {
        return Err(Mismatch::AccessOwners.into());
    }
    if record.declaration_access() != access {
        return Err(Mismatch::Access.into());
    }
    Ok(())
}

fn owners_match(key: &SourceDeclarationKey, owners: &[SourceNominalId]) -> bool {
    key.owners().owners().len() == owners.len()
        && key
            .owners()
            .owners()
            .iter()
            .zip(owners)
            .all(|(key, owner)| match (key, owner) {
                (DefinitionOwnerAtom::Type(left), SourceNominalId::Concrete(right)) => {
                    left == right
                }
                (
                    DefinitionOwnerAtom::GenericType(left),
                    SourceNominalId::GenericTemplate(right),
                ) => left == right,
                _ => false,
            })
}
