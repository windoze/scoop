use super::CrossConeTypeSemanticsProductionError as Error;
use super::inheritance::source_errors::{invalid, resource};
use super::nominals;
use crate::*;
use scoop_identity::{DefinitionOriginSubject as Subject, SourceDeclarationKey};
use scoop_wire::WirePath;
use std::collections::{BTreeMap, BTreeSet};
mod index;

impl CanonicalDefaultSourceAccessDeclarationsV1 {
    /// Projects independently demanded declarations and their full lexical owner
    /// closure. Foreign providers must project their own artifact sources.
    pub fn from_export_hir(
        output: &ExportHirOutput,
        required: &BTreeSet<Subject>,
    ) -> Result<Self, Error> {
        let path = WirePath::root();

        for subject in required {
            DefaultSourceAccessDeclarationV1::validate_subject(*subject)
                .map_err(Error::SourceInventory)?;
        }
        let export = output.module();
        let index = index::collect(export)?;
        let mut pending = required.clone();
        let mut records = BTreeMap::new();
        while let Some(subject) = pending.pop_first() {
            if records.contains_key(&subject) {
                continue;
            }

            let declaration = index.get(&subject).ok_or_else(|| {
                invalid(format!(
                    "required default access declaration has no local sealed source: {subject:?}"
                ))
            })?;
            let access = declaration.access(export, subject)?;
            for owner in access.lexical_owners() {
                let owner = match owner {
                    SourceNominalId::Concrete(id) => Subject::Type(*id),
                    SourceNominalId::GenericTemplate(id) => Subject::GenericType(*id),
                };

                if !records.contains_key(&owner) && !pending.contains(&owner) {
                    pending.insert(owner);
                }
            }

            let record = DefaultSourceAccessDeclarationV1::try_new(subject, access)
                .map_err(Error::SourceInventory)?;

            records.insert(subject, record);
        }
        let mut output = Vec::new();
        scoop_wire::allocation::try_reserve(&mut output, records.len(), &path).map_err(resource)?;

        output.extend(records.into_values());
        Self::try_new(output).map_err(Error::SourceInventory)
    }
}

struct Declaration<'a> {
    key: &'a SourceDeclarationKey,
    visibility: DeclaredVisibility,
}
impl Declaration<'_> {
    fn access(
        &self,
        export: &ExportHir,
        subject: Subject,
    ) -> Result<DeclarationAccessSourceV1, Error> {
        // The result owns its owner chain; construction also checks uniqueness.

        let owners = nominals::lexical_owners(self.key)?;

        let origin = export
            .export_definition_origins
            .get(subject)
            .ok_or(Error::MissingDefinitionOrigin(subject))?;

        DeclarationAccessSourceV1::try_new(
            self.visibility.into(),
            owners,
            ExportDefinitionSourceV1::new(origin.origin().clone()),
        )
        .map_err(invalid)
    }
}
