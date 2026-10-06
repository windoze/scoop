//! Per-artifact exported constant validation.

use scoop_hir::ExportConstValueSetSemanticValidationError;

use super::{SourceInterfaceValidatedCrossConeHirFrontSections, ValidatedSurfaceFront};
use crate::cross_cone_hir_authority::{
    CrossConeHirConstAuthorityError, ValidatedNominalProviderView,
};

/// One provider whose portable const records match its validated property
/// surface and the shared intrinsic declarations of their actual types.
pub struct ConstValidatedCrossConeHirFrontSections<'input>(
    pub(super) ValidatedSurfaceFront<'input>,
);

impl<'input> SourceInterfaceValidatedCrossConeHirFrontSections<'input> {
    pub(crate) fn validate_const_values<'dependency>(
        self,
        dependencies: Vec<ValidatedNominalProviderView<'dependency>>,
    ) -> Result<ConstValidatedCrossConeHirFrontSections<'input>, CrossConeHirConstSurfaceError>
    {
        let mut front = self.0;
        let input = front.hir_validation_parts();
        input.constants(dependencies)?;
        Ok(ConstValidatedCrossConeHirFrontSections(front))
    }
}

#[derive(Debug)]
pub enum CrossConeHirConstSurfaceError {
    Constants(ExportConstValueSetSemanticValidationError<CrossConeHirConstAuthorityError>),
    Annotations(crate::cross_cone_hir_authority::CrossConeHirAnnotationError),
}

impl std::fmt::Display for CrossConeHirConstSurfaceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Constants(error) => error.fmt(formatter),
            Self::Annotations(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CrossConeHirConstSurfaceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Constants(error) => Some(error),
            Self::Annotations(error) => Some(error),
        }
    }
}
