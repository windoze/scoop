use super::CrossConeTypeSemanticsProductionError as Error;
use super::inheritance::source_errors::{invalid, resource};
use crate::production::{callable_interfaces, signatures::HirInterfaceSignatureProjector};
use crate::*;
use scoop_identity::{CallableTemplateOrigin, DefinitionOriginSubject, SourceDeclarationKey};
use scoop_wire::WirePath;
use std::collections::BTreeSet;

mod accessors;
mod functions;
mod parameters;
mod relations;
mod variants;

pub(super) fn project(
    export: &ExportHir,
    required: BTreeSet<CallableTemplateOrigin>,
) -> Result<Vec<NominalSupportCallableInterfaceV1>, Error> {
    for declaration in &required {
        if !matches!(
            declaration,
            CallableTemplateOrigin::Function(_)
                | CallableTemplateOrigin::GenericFunction(_)
                | CallableTemplateOrigin::Accessor(_)
                | CallableTemplateOrigin::VariantConstructor(_)
        ) {
            return Err(invalid(
                "nominal callable source has another declaration role",
            ));
        }
    }
    let mut projection = Projection {
        export,
        signatures: HirInterfaceSignatureProjector::new(export),
        required,
        records: Vec::new(),
    };
    projection.functions()?;
    projection.accessors()?;
    projection.variants()?;
    if !projection.required.is_empty() {
        return Err(invalid(
            "required nominal source callable has no sealed declaration",
        ));
    }
    Ok(projection.records)
}

struct Projection<'a> {
    export: &'a ExportHir,
    signatures: HirInterfaceSignatureProjector<'a>,
    required: BTreeSet<CallableTemplateOrigin>,
    records: Vec<NominalSupportCallableInterfaceV1>,
}
impl Projection<'_> {
    fn take(&mut self, declaration: CallableTemplateOrigin) -> bool {
        self.required.remove(&declaration)
    }
    fn origin(
        &mut self,
        subject: DefinitionOriginSubject,
    ) -> Result<ExportDefinitionSourceV1, Error> {
        let origin = self
            .export
            .export_definition_origins
            .get(subject)
            .ok_or(Error::MissingDefinitionOrigin(subject))?;

        Ok(ExportDefinitionSourceV1::new(origin.origin().clone()))
    }
    fn access(
        &mut self,
        key: &SourceDeclarationKey,
        subject: DefinitionOriginSubject,
        visibility: DeclaredVisibility,
    ) -> Result<DeclarationAccessSourceV1, Error> {
        if key.origin() != self.export.cone {
            return Err(invalid("nominal callable source belongs to another Cone"));
        }

        let owners = super::nominals::lexical_owners(key)?;
        let origin = self.origin(subject)?;
        DeclarationAccessSourceV1::try_new(visibility.into(), owners, origin).map_err(invalid)
    }
    fn push(
        &mut self,
        declaration: CallableTemplateOrigin,
        access: DeclarationAccessSourceV1,
        payload: NominalSourceCallablePayloadV1,
    ) -> Result<(), Error> {
        let path = WirePath::root();

        scoop_wire::allocation::try_reserve(&mut self.records, 1, &path).map_err(resource)?;
        self.records.push(
            NominalSupportCallableInterfaceV1::try_new(declaration, access, payload)
                .map_err(invalid)?,
        );
        Ok(())
    }
}

fn owner(access: &DeclarationAccessSourceV1) -> Result<SourceNominalId, Error> {
    access
        .lexical_owners()
        .last()
        .copied()
        .ok_or_else(|| invalid("nominal callable has no lexical nominal owner"))
}
