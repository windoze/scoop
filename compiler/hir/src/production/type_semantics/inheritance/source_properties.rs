use super::source_errors::{invalid, resource};
use super::*;
use crate::production::{
    property_interfaces::source_property_representation, signatures::HirInterfaceSignatureProjector,
};
use scoop_identity::{DefinitionOriginSubject, PersistentPropertyId, SourceDeclarationKey};
use scoop_wire::WirePath;

mod contract;
mod nominals;
pub(in crate::production::type_semantics) use nominals::project as project_nominal;
mod slots;

fn declaration_access(
    export: &ExportHir,
    key: &SourceDeclarationKey,
    subject: DefinitionOriginSubject,
    visibility: DeclaredVisibility,
) -> Result<DeclarationAccessSourceV1, Error> {
    export
        .export_definition_origins
        .get(subject)
        .ok_or(Error::MissingDefinitionOrigin(subject))?;

    super::super::nominals::declaration_access_for_subject(export, key, subject, visibility.into())
}
