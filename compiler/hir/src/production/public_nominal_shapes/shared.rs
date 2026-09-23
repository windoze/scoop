use super::*;
use crate::{
    CanonicalCallableInterfacesV1, CanonicalNominalInterfacesV1, ExportHir, SourceNominalId,
};
use scoop_wire::{BudgetMeter, WirePath};

mod roots;

impl NominalMaterializationClosure {
    pub fn from_export_hir(
        export: &ExportHir,
        meter: &mut BudgetMeter,
    ) -> Result<Self, PublicNominalShapeProjectionError> {
        let roots = roots::current(export, meter)?;
        Self::from_roots(export, &roots, meter)
    }

    pub fn from_current_declarations(
        export: &ExportHir,
        meter: &mut BudgetMeter,
    ) -> Result<Self, PublicNominalShapeProjectionError> {
        let roots = roots::all_current(export, meter)?;
        Self::from_roots(export, &roots, meter)
    }

    fn from_roots(
        export: &ExportHir,
        roots: &[SourceNominalId],
        meter: &mut BudgetMeter,
    ) -> Result<Self, PublicNominalShapeProjectionError> {
        let nominals = CanonicalNominalInterfacesV1::declarations_for_roots(export, roots, meter)
            .map_err(|error| match error {
            crate::NominalInterfaceBuildError::Resource(error) => resource(error),
            other => PublicNominalShapeProjectionError::SharedDeclarations(other.to_string()),
        })?;
        let properties = crate::CanonicalPropertyInterfacesV1::from_nominal_declarations(
            export, &nominals, meter,
        )
        .map_err(|error| match error {
            crate::PropertyInterfaceBuildError::Resource(error) => resource(error),
            other => PublicNominalShapeProjectionError::SharedDeclarations(other.to_string()),
        })?;
        let callables = CanonicalCallableInterfacesV1::from_nominal_declarations(
            export,
            &properties,
            &nominals,
            meter,
        )
        .map_err(|error| match error {
            crate::CallableInterfaceBuildError::Resource(error) => resource(error),
            other => PublicNominalShapeProjectionError::SharedDeclarations(other.to_string()),
        })?;
        Self::from_declarations(&nominals, &callables, meter)
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
        meter: &mut BudgetMeter,
    ) -> Result<Self, PublicNominalShapeProjectionError> {
        meter
            .charge_work(
                (self.roots.len() as u64)
                    .saturating_mul(1 + u64::from(closure.sources().len().max(1).ilog2())),
                &WirePath::root(),
            )
            .map_err(NominalMaterializationClosureError::Resource)
            .map_err(PublicNominalShapeProjectionError::Materialization)?;
        self.roots.retain(|root| closure.contains(root.source));
        Ok(self)
    }

    pub fn from_export_hir(
        export: &ExportHir,
        meter: &mut BudgetMeter,
    ) -> Result<Self, PublicNominalShapeProjectionError> {
        let requirements = Self::from_public_bindings(
            export.cone,
            &export.public_export_bindings,
            &export.export_binding_identities,
        )?;
        if requirements.roots().is_empty() {
            return Ok(requirements);
        }
        let closure = NominalMaterializationClosure::from_export_hir(export, meter)?;
        requirements.retain_materializable(&closure, meter)
    }

    pub fn from_shared_surface(
        surface: &CanonicalDirectPublicSurfaceV1,
        foundation: &CanonicalHirFoundation,
        nominals: &CanonicalNominalInterfacesV1,
        callables: &CanonicalCallableInterfacesV1,
        meter: &mut BudgetMeter,
    ) -> Result<Self, PublicNominalShapeProjectionError> {
        let requirements = Self::from_direct_surface(surface, foundation)?;
        for root in requirements.roots() {
            meter
                .charge_work(
                    1 + u64::from(nominals.declaration_count().max(1).ilog2()),
                    &WirePath::root(),
                )
                .map_err(NominalMaterializationClosureError::Resource)
                .map_err(PublicNominalShapeProjectionError::Materialization)?;
            if nominals
                .declaration(SourceNominalId::Concrete(root.source()))
                .is_none()
            {
                return Err(PublicNominalShapeProjectionError::Materialization(
                    NominalMaterializationClosureError::MissingNominal(root.source()),
                ));
            }
        }
        let closure = NominalMaterializationClosure::from_declarations(nominals, callables, meter)
            .map_err(PublicNominalShapeProjectionError::Materialization)?;
        requirements.retain_materializable(&closure, meter)
    }
}
