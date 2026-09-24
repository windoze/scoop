use super::*;
use scoop_hir::SourceNominalId;
use scoop_identity::{PersistentGenericTypeId, PersistentTypeId, SourceDeclarationKey};

mod shapes;

pub(super) fn collect<'a>(
    source: AbiReplayDependency<'a>,
    definitions: &mut HashMap<NativeBoundaryNominalOwner, AbiNominalDefinition<'a>>,
    meter: &mut BudgetMeter,
) -> Result<(), NativeBoundaryCompileError> {
    let path = WirePath::root().field(34);
    for nominal in source.nominals.all_records() {
        meter
            .charge_work(
                scoop_wire::encoded_length(nominal)
                    .map_err(NativeBoundaryCompileError::Encoding)?,
                &path,
            )
            .map_err(NativeBoundaryCompileError::Resource)?;
        let (owner, key) = match nominal.declaration() {
            SourceNominalId::Concrete(id) => (
                NativeBoundaryNominalOwner::Concrete(id),
                source
                    .identities
                    .canonical_key::<PersistentTypeId, SourceDeclarationKey>(id),
            ),
            SourceNominalId::GenericTemplate(id) => (
                NativeBoundaryNominalOwner::GenericTemplate(id),
                source
                    .identities
                    .canonical_key::<PersistentGenericTypeId, SourceDeclarationKey>(id),
            ),
        };
        let key = key.map_err(NativeBoundaryCompileError::Reference)?;
        if key.origin() != source.identity {
            return Err(NativeBoundaryCompileError::NominalProvider {
                owner,
                declared: key.origin(),
                provider: source.identity,
            });
        }
        if key.duplicate_signature().type_parameter_count() != nominal.type_parameters().len_u32() {
            return Err(NativeBoundaryCompileError::ConflictingTypeWitness { owner });
        }
        let shape = shapes::project(nominal.source_shape(), owner, source.identities, meter)?;
        let record = AbiNominalDefinition::shared(shape, nominal.type_parameters().len_u32());
        if let Some(previous) = definitions.get(&owner) {
            if !previous.agrees_with_source(&record) {
                return Err(NativeBoundaryCompileError::ConflictingTypeWitness { owner });
            }
        } else {
            insert_definition(definitions, owner, record, meter)?;
        }
    }
    Ok(())
}
