use super::*;
use scoop_identity::PersistentConstructorId;

pub(super) fn project(
    export: &ExportHir,
    local: LocalNominalId,
    owner: SourceNominalId,
) -> Result<CanonicalPersistentIdsV1<PersistentConstructorId>, Error> {
    let mut constructors = Vec::new();
    match local {
        LocalNominalId::Class(id) => {
            let d = &export.classes[id];
            for &constructor in &d.constructors {
                if export.class_constructors[constructor].owner != id {
                    return Err(invalid("source constructor has a different class owner"));
                }
                match &export.constructor_identities[constructor] {
                    HirClassConstructorIdentity::Source(record) => {
                        validate_owner(export, owner, record.key())?;
                        push(&mut constructors, record.id())?;
                    }
                    HirClassConstructorIdentity::ZeroArgumentAdapter { source, .. } => {
                        // A generated adapter references an actual source constructor;
                        // it never becomes a second declaration-side contract.

                        if !d.constructors.contains(source)
                            || export.class_constructors[*source].owner != id
                            || export.constructor_identities[*source]
                                .source_record()
                                .is_none()
                        {
                            return Err(invalid(
                                "nominal source contains an orphan constructor adapter",
                            ));
                        }
                    }
                }
            }
        }
        LocalNominalId::Struct(id) => {
            for &constructor in &export.structs[id].constructors {
                if export.struct_constructors[constructor].owner != id {
                    return Err(invalid("source constructor has a different struct owner"));
                }
                let record = &export.constructor_identities[constructor];
                validate_owner(export, owner, record.key())?;
                push(&mut constructors, record.id())?;
            }
        }
        LocalNominalId::Interface(_) | LocalNominalId::Enum(_) | LocalNominalId::Object(_) => {
            return Ok(CanonicalPersistentIdsV1::empty());
        }
    }

    CanonicalPersistentIdsV1::try_new(constructors).map_err(invalid)
}
