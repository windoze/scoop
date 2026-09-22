use super::*;
use scoop_hir::{NativeBoundaryNominalShape, NominalSourceShapeV1, SourceNominalId};
use scoop_identity::{PersistentGenericTypeId, PersistentTypeId, SourceDeclarationKey};

pub(super) fn collect<'a>(
    source: AbiReplayDependency<'a>,
    definitions: &mut HashMap<
        NativeBoundaryNominalOwner,
        Cow<'a, NativeBoundaryTypeDefinitionRecord>,
    >,
    meter: &mut BudgetMeter,
) -> Result<(), NativeBoundaryCompileError> {
    let path = WirePath::root().field(33);
    for nominal in source.nominals.records() {
        meter
            .charge_work(
                scoop_wire::encoded_length(nominal)
                    .map_err(NativeBoundaryCompileError::Encoding)?,
                &path,
            )
            .map_err(NativeBoundaryCompileError::Resource)?;
        let owner = match nominal.declaration() {
            SourceNominalId::Concrete(id) => NativeBoundaryNominalOwner::Concrete(id),
            SourceNominalId::GenericTemplate(id) => NativeBoundaryNominalOwner::GenericTemplate(id),
        };
        let NominalSourceShapeV1::Intrinsic(representation) = nominal.source_shape() else {
            if definitions.get(&owner).is_some_and(|record| {
                matches!(record.shape(), NativeBoundaryNominalShape::Intrinsic(_))
            }) {
                return Err(NativeBoundaryCompileError::ConflictingTypeWitness { owner });
            }
            continue;
        };
        let key = match nominal.declaration() {
            SourceNominalId::Concrete(id) => source
                .identities
                .canonical_key::<PersistentTypeId, SourceDeclarationKey>(id),
            SourceNominalId::GenericTemplate(id) => source
                .identities
                .canonical_key::<PersistentGenericTypeId, SourceDeclarationKey>(id),
        }
        .map_err(NativeBoundaryCompileError::Reference)?;
        meter
            .charge_work(
                scoop_wire::encoded_length(key.as_ref())
                    .map_err(NativeBoundaryCompileError::Encoding)?,
                &path,
            )
            .map_err(NativeBoundaryCompileError::Resource)?;
        if key.origin() != source.identity {
            return Err(NativeBoundaryCompileError::IntrinsicProvider {
                owner,
                declared: key.origin(),
                provider: source.identity,
            });
        }
        let record = NativeBoundaryTypeDefinitionRecord::new(
            &key,
            &[nominal.type_parameters().len_u32()],
            NativeBoundaryNominalShape::Intrinsic(*representation),
        )
        .map_err(NativeBoundaryCompileError::TypeDefinition)?;
        insert_definition(definitions, Cow::Owned(record), meter)?;
    }
    Ok(())
}
