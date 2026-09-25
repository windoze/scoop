use super::*;
use crate::{
    CanonicalCallableInterfacesV1, CanonicalNominalInterfacesV1, ExportHir, SourceNominalId,
};
use scoop_wire::WirePath;

mod roots;

impl NominalMaterializationClosure {
    pub fn from_export_hir(export: &ExportHir) -> Result<Self, PublicNominalShapeProjectionError> {
        let roots = roots::current(export)?;
        Self::from_roots(export, &roots)
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
            .map_err(|error| match error {
                crate::NominalInterfaceBuildError::Resource(error) => resource(error),
                other => PublicNominalShapeProjectionError::SharedDeclarations(other.to_string()),
            })?;
        let properties =
            crate::CanonicalPropertyInterfacesV1::from_nominal_declarations(export, &nominals)
                .map_err(|error| match error {
                    crate::PropertyInterfaceBuildError::Resource(error) => resource(error),
                    other => {
                        PublicNominalShapeProjectionError::SharedDeclarations(other.to_string())
                    }
                })?;
        let callables = CanonicalCallableInterfacesV1::from_nominal_declarations(
            export,
            &properties,
            &nominals,
        )
        .map_err(|error| match error {
            crate::CallableInterfaceBuildError::Resource(error) => resource(error),
            other => PublicNominalShapeProjectionError::SharedDeclarations(other.to_string()),
        })?;
        Self::from_declarations(&nominals, &callables)
            .map_err(PublicNominalShapeProjectionError::Materialization)
    }
}

fn resource(error: scoop_wire::WireError) -> PublicNominalShapeProjectionError {
    PublicNominalShapeProjectionError::Materialization(
        NominalMaterializationClosureError::Resource(error),
    )
}

impl PublicNominalShapeRequirementsV1 {
    pub fn retain_materializable(
        mut self,
        closure: &NominalMaterializationClosure,
    ) -> Result<Self, PublicNominalShapeProjectionError> {
        let builtins = LANGUAGE_BUILTINS.map(|builtin| builtin.identity_record().id());
        self.roots
            .retain(|root| builtins.contains(&root.source) || closure.contains(root.source));
        Ok(self)
    }

    pub fn from_export_hir(export: &ExportHir) -> Result<Self, PublicNominalShapeProjectionError> {
        let requirements = Self::from_public_bindings(
            export.cone,
            &export.public_export_bindings,
            &export.export_binding_identities,
        )?;
        if requirements.roots().is_empty() {
            return Ok(requirements);
        }
        let closure = NominalMaterializationClosure::from_export_hir(export)?;
        requirements.retain_materializable(&closure)
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
        requirements.retain_materializable(&closure)
    }
}
