use super::*;
use scoop_hir::SourceNominalId;
use scoop_identity::{PersistentGenericTypeId, PersistentTypeId, SourceDeclarationKey};

mod shapes;

pub(super) fn collect<'a>(
    source: AbiReplayDependency<'a>,
    definitions: &mut HashMap<NativeBoundaryNominalOwner, AbiNominalDefinition<'a>>,
) -> Result<(), NativeBoundaryCompileError> {
    for nominal in source.nominals.all_records() {
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
        let shape = shapes::project(nominal.source_shape(), owner, source.identities)?;
        let record = AbiNominalDefinition::shared(
            shape,
            nominal.type_parameters().len_u32(),
            key.declaration_kind() == scoop_identity::SourceDeclarationKind::Interface,
        );
        if let Some(previous) = definitions.get(&owner) {
            if !previous.agrees_with_source(&record) {
                return Err(NativeBoundaryCompileError::ConflictingTypeWitness { owner });
            }
        } else {
            insert_definition(definitions, owner, record)?;
        }
    }
    Ok(())
}
