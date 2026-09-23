use super::*;
use crate::SourceNominalId;
use scoop_identity::{
    DeclarationName, DeclarationScope, DefinitionOwnerAtom, DuplicateSignatureKey,
};
use scoop_wire::{WireError, WireErrorKind, encoded_length};

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
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), Failure> {
    use NominalRepresentationSourceMismatchV1 as Mismatch;
    let key = expected.key;
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
    charge_key(key, meter, &path.clone().field(1))?;
    if PersistentTypeId::from_source_declaration(key).ok() != Some(record.owner()) {
        return Err(Mismatch::OwnerIdentity.into());
    }
    if key.declaration_kind() != expected.shape.source_kind() {
        return Err(Mismatch::SourceKind.into());
    }
    let at = path.clone().field(2);
    charge_access(expected.access, meter, &at)?;
    charge_access(record.declaration_access(), meter, &at)?;
    let source = expected.access.definition_origin().origin().source();
    if source.cone() != provider || key.scope().source().is_some_and(|scope| scope != source) {
        return Err(Mismatch::AccessSource.into());
    }
    if !owners_match(key, expected.access.lexical_owners()) {
        return Err(Mismatch::AccessOwners.into());
    }
    if record.declaration_access() != expected.access {
        return Err(Mismatch::Access.into());
    }
    if !compare::shape(
        record.shape(),
        expected.shape,
        meter,
        &path.clone().field(3),
    )? {
        return Err(Mismatch::Shape.into());
    }
    if let NominalRepresentationPublicSourceShapeV1::PublicSourceShape(public) =
        expected.public_source_shape
        && !record.public_source_shape_matches(public, meter, &path.clone().field(3))?
    {
        return Err(Mismatch::PublicSourceShape.into());
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

pub(super) fn sequence(
    count: usize,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), WireError> {
    meter.check_table_entries(count as u64, path)?;
    meter.charge_work(count as u64, path)?;
    meter.charge_nodes(count as u64, path)?;
    meter.charge_edges(count as u64, path)
}
fn leaf(text: &str, meter: &mut BudgetMeter, path: &WirePath) -> Result<(), WireError> {
    meter.check_semantic_leaf(text.len() as u64, path)?;
    meter.charge_work(text.len() as u64, path)
}
pub(in crate::cross_cone_type_semantics::representation) fn charge_key(
    key: &SourceDeclarationKey,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), WireError> {
    meter.check_semantic_depth(2, path)?;
    meter.charge_nodes(1, path)?;
    sequence(key.package().segments().len(), meter, path)?;
    for segment in key.package().segments() {
        leaf(segment.as_str(), meter, path)?;
    }
    sequence(key.owners().owners().len(), meter, path)?;
    if let DeclarationName::Named(name) = key.name() {
        leaf(name.as_str(), meter, path)?;
    }
    if let Some(source) = key.scope().source() {
        leaf(source.logical_path().as_str(), meter, path)?;
    }
    if let DeclarationScope::LexicalScoped {
        path: definition, ..
    } = key.scope()
    {
        sequence(definition.segments().len(), meter, path)?;
    }
    let bytes = encoded_length(key)
        .map_err(|_| WireError::new(WireErrorKind::IntegerOutOfRange, path.clone(), None))?;
    meter.charge_work(bytes, path)
}
fn charge_access(
    access: &DeclarationAccessSourceV1,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), WireError> {
    meter.check_semantic_depth(2, path)?;
    meter.charge_nodes(1, path)?;
    sequence(access.lexical_owners().len(), meter, path)?;
    leaf(
        access
            .definition_origin()
            .origin()
            .source()
            .logical_path()
            .as_str(),
        meter,
        path,
    )?;
    let bytes = encoded_length(access)
        .map_err(|_| WireError::new(WireErrorKind::IntegerOutOfRange, path.clone(), None))?;
    meter.charge_work(bytes, path)
}
