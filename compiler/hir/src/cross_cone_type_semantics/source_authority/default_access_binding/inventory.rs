use super::*;
use scoop_identity::DefinitionOwnerAtom;

pub(super) fn bind<'f>(
    foundation: &BoundTypeFoundationSourcesV1<'f>,
    source: &CanonicalDefaultSourceAccessDeclarationsV1,
    required: &BTreeSet<Subject>,
) -> Result<BTreeMap<Subject, &'f SourceDeclarationKey>, Error> {
    for subject in required {
        DefaultSourceAccessDeclarationV1::validate_subject(*subject)
            .map_err(|_| Error::InvalidSubject(*subject))?;
    }
    let available = keys::Available::new(foundation)?;
    let mut pending = required.clone();
    let mut keys = BTreeMap::new();
    while let Some(subject) = pending.pop_first() {
        if keys.contains_key(&subject) {
            continue;
        }
        let key = available.get(subject)?;
        if key.origin() != foundation.source().entries().provider {
            return Err(Error::ForeignKey(subject));
        }

        if source.get(subject).is_none() {
            return Err(Error::MissingRecord(subject));
        }

        for atom in key.owners().owners() {
            let owner = match atom {
                DefinitionOwnerAtom::Type(id) => Subject::Type(*id),
                DefinitionOwnerAtom::GenericType(id) => Subject::GenericType(*id),
                _ => return Err(Error::NonNominalOwner(subject)),
            };

            if !keys.contains_key(&owner) && !pending.contains(&owner) {
                pending.insert(owner);
            }
        }

        keys.insert(subject, key);
    }
    for record in source.records() {
        if !keys.contains_key(&record.subject()) {
            return Err(Error::UnexpectedRecord(record.subject()));
        }
    }
    Ok(keys)
}
