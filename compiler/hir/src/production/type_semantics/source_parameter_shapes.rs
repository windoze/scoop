use super::CrossConeTypeSemanticsProductionError as Error;
use super::inheritance::source_errors::invalid;
use crate::production::{callable_interfaces, signatures::HirInterfaceSignatureProjector};
use crate::*;
use scoop_identity::SignatureTypeKey;

pub(super) fn project(
    export: &ExportHir,
    signatures: &HirInterfaceSignatureProjector<'_>,
    owner: ExportParameterOwner,
    binders: &[HirSignatureBinder],
    expected: &[SignatureTypeKey],
) -> Result<CanonicalSourceParameterShapesV1, Error> {
    callable_interfaces::source_parameter_shapes(export, signatures, owner, binders, expected)
        .map_err(invalid)
}
