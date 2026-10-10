use scoop_wire::WirePath;

use super::{
    CrossConeHirInterfaceSectionV1, default_reference_closure::DefaultReferenceClosureValidator,
};
use crate::{
    ExternalHirDefaultClosureValidationError, ExternalHirDefaultUseSiteV1,
    ExternalHirReferenceRoleV1, ExternalHirReferenceSemanticAuthority,
};

impl CrossConeHirInterfaceSectionV1 {
    pub(super) fn validate_annotation_reference_closure<A, E>(
        &self,
        authority: &mut A,
        path: &WirePath,
    ) -> Result<(), ExternalHirDefaultClosureValidationError<E>>
    where
        A: ExternalHirReferenceSemanticAuthority<E>,
    {
        let mut validator = DefaultReferenceClosureValidator::new(
            self.external_references(),
            authority,
            &path.clone().field(10),
        )?;
        validator.role = ExternalHirReferenceRoleV1::AnnotationDependency;
        for (reference_index, target) in self
            .annotations()
            .declaration_targets(path)
            .map_err(ExternalHirDefaultClosureValidationError::Resource)?
            .into_iter()
            .enumerate()
        {
            validator.observe(
                target,
                ExternalHirDefaultUseSiteV1::Annotation { reference_index },
            )?;
        }
        validator.finish()
    }
}
