use std::collections::BTreeSet;
use std::fmt;

use super::CrossConeHirInterfaceSectionV1;
use crate::{
    ExternalHirReferenceRoleV1 as Role, ExternalHirReferenceSemanticAuthority, ExternalHirTargetV1,
};
use scoop_identity::ConeIdentity;
use scoop_wire::WireError;

impl CrossConeHirInterfaceSectionV1 {
    pub fn validate_inheritance_reference_closure<
        A: ExternalHirReferenceSemanticAuthority<E>,
        E,
    >(
        &self,
        authority: &mut A,
    ) -> Result<(), ExternalHirInheritanceClosureValidationError<E>> {
        use ExternalHirInheritanceClosureValidationError as Error;
        let mut required = BTreeSet::new();
        for nominal in self.nominal_interfaces().all_records() {
            for choice in nominal
                .declaration_details()
                .dispatch_selections()
                .records()
            {
                let Some(target) = choice.callable_target() else {
                    continue;
                };
                let target = ExternalHirTargetV1::Callable(target);
                let expected = authority
                    .external_hir_target_origin(target)
                    .map_err(|error| Error::TargetOrigin { target, error })?;
                if expected == authority.current_cone() {
                    continue;
                }
                let index = self
                    .external_references()
                    .find_index(target)
                    .ok_or(Error::MissingReference(target))?;
                let reference = &self.external_references().records()[index];
                if reference.origin() != expected {
                    return Err(Error::Origin {
                        target,
                        expected,
                        actual: reference.origin(),
                    });
                }
                if !reference.roles().contains(Role::InheritanceDependency) {
                    return Err(Error::MissingRole(target));
                }

                required.insert(target);
            }
        }
        for reference in self.external_references().records() {
            if reference.roles().contains(Role::InheritanceDependency)
                && !required.contains(&reference.target())
            {
                return Err(Error::ExtraRole(reference.target()));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum ExternalHirInheritanceClosureValidationError<E> {
    TargetOrigin {
        target: ExternalHirTargetV1,
        error: E,
    },
    MissingReference(ExternalHirTargetV1),
    MissingRole(ExternalHirTargetV1),
    ExtraRole(ExternalHirTargetV1),
    Origin {
        target: ExternalHirTargetV1,
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    Resource(WireError),
}

impl<E: fmt::Display> fmt::Display for ExternalHirInheritanceClosureValidationError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TargetOrigin { target, error } => write!(
                f,
                "inheritance target {target:?} has no canonical origin: {error}"
            ),
            Self::MissingReference(target) => write!(
                f,
                "inheritance target {target:?} is absent from the external HIR references"
            ),
            Self::MissingRole(target) => write!(
                f,
                "inheritance target {target:?} is missing its dependency role"
            ),
            Self::ExtraRole(target) => write!(
                f,
                "external target {target:?} has no declared inheritance use"
            ),
            Self::Origin {
                target,
                expected,
                actual,
            } => write!(
                f,
                "inheritance target {target:?} has provider {actual}, expected {expected}"
            ),
            Self::Resource(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error
    for ExternalHirInheritanceClosureValidationError<E>
{
}
