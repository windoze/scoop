use super::*;
use crate::{
    CanonicalCallableInterfacesV1, CanonicalNominalInterfacesV1, ExportHir, ExportHirOutput,
    SourceNominalId,
};
use scoop_wire::WirePath;

mod roots;

impl NominalMaterializationClosure {
    pub fn from_export_hir(
        export: &ExportHirOutput,
    ) -> Result<Self, PublicNominalShapeProjectionError> {
        let roots = &export.shared_source().roots;
        let nominals =
            CanonicalNominalInterfacesV1::declarations_for_required(export, &roots.nominals)
                .map_err(shared_declarations)?;
        Self::from_nominals(export, &nominals)
    }

    pub fn from_current_declarations(
        export: &ExportHir,
    ) -> Result<Self, PublicNominalShapeProjectionError> {
        let roots = roots::all_current(export)?;
        Self::from_roots(export, &roots)
    }

    fn from_roots(
        export: &ExportHir,
        roots: &[SourceNominalId],
    ) -> Result<Self, PublicNominalShapeProjectionError> {
        let nominals = CanonicalNominalInterfacesV1::declarations_for_roots(export, roots)
            .map_err(shared_declarations)?;
        Self::from_nominals(export, &nominals)
    }

    fn from_nominals(
        export: &ExportHir,
        nominals: &CanonicalNominalInterfacesV1,
    ) -> Result<Self, PublicNominalShapeProjectionError> {
        let properties =
            crate::CanonicalPropertyInterfacesV1::from_nominal_declarations(export, nominals)
                .map_err(|error| match error {
                    crate::PropertyInterfaceBuildError::Resource(error) => resource(error),
                    other => {
                        PublicNominalShapeProjectionError::SharedDeclarations(other.to_string())
                    }
                })?;
        let callables =
            CanonicalCallableInterfacesV1::from_nominal_declarations(export, &properties, nominals)
                .map_err(|error| match error {
                    crate::CallableInterfaceBuildError::Resource(error) => resource(error),
                    other => {
                        PublicNominalShapeProjectionError::SharedDeclarations(other.to_string())
                    }
                })?;
        Self::from_declarations(nominals, &callables)
            .map_err(PublicNominalShapeProjectionError::Materialization)
    }
}

fn shared_declarations(
    error: crate::NominalInterfaceBuildError,
) -> PublicNominalShapeProjectionError {
    match error {
        crate::NominalInterfaceBuildError::Resource(error) => resource(error),
        other => PublicNominalShapeProjectionError::SharedDeclarations(other.to_string()),
    }
}

fn resource(error: scoop_wire::WireError) -> PublicNominalShapeProjectionError {
    PublicNominalShapeProjectionError::Materialization(
        NominalMaterializationClosureError::Resource(error),
    )
}

impl PublicNominalShapeRequirementsV1 {
    pub fn from_export_hir(
        export: &ExportHirOutput,
    ) -> Result<Self, PublicNominalShapeProjectionError> {
        let closure = NominalMaterializationClosure::from_export_hir(export)?;
        Self::from_sources(export.cone, closure.sources().iter().copied().collect())
    }

    pub fn from_shared_surface(
        producer: ConeIdentity,
        surface: &CanonicalDirectPublicSurfaceV1,
        foundation: &CanonicalHirFoundation,
        nominals: &CanonicalNominalInterfacesV1,
        callables: &CanonicalCallableInterfacesV1,
    ) -> Result<Self, PublicNominalShapeProjectionError> {
        let requirements = Self::from_direct_surface(producer, surface, foundation)?;
        let builtins = LANGUAGE_BUILTINS.map(|builtin| builtin.identity_record().id());
        for root in requirements.roots() {
            if !builtins.contains(&root.source())
                && nominals
                    .declaration(SourceNominalId::Concrete(root.source()))
                    .is_none()
            {
                return Err(PublicNominalShapeProjectionError::Materialization(
                    NominalMaterializationClosureError::MissingNominal(root.source()),
                ));
            }
        }
        let closure = NominalMaterializationClosure::from_declarations(nominals, callables)
            .map_err(PublicNominalShapeProjectionError::Materialization)?;
        Self::from_sources(producer, closure.sources().iter().copied().collect())
    }
}
