use std::fmt;

use scoop_identity::{DigestNodeId, PersistentExactTypeId};
use scoop_wire::HashError;

use crate::SlibMemberId;
use crate::link_object::{LayoutFingerprintV1, ObjectDefinitionFingerprintV1};

mod encoding;
use encoding::{
    TypeDescriptorITableDirectoryFingerprintInputV1, descriptor_fingerprint, layout_fingerprint,
};

mod reading;
use reading::atom_bytes;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TypeDependencyFingerprintV1 {
    exact_type: PersistentExactTypeId,
    descriptor_definition_node: DigestNodeId,
    descriptor_definition: ObjectDefinitionFingerprintV1,
    layout_node: DigestNodeId,
    layout: LayoutFingerprintV1,
}

impl TypeDependencyFingerprintV1 {
    pub const fn exact_type(self) -> PersistentExactTypeId {
        self.exact_type
    }

    pub const fn descriptor_definition_node(self) -> DigestNodeId {
        self.descriptor_definition_node
    }

    pub const fn descriptor_definition(self) -> ObjectDefinitionFingerprintV1 {
        self.descriptor_definition
    }

    pub const fn layout_node(self) -> DigestNodeId {
        self.layout_node
    }

    pub const fn layout(self) -> LayoutFingerprintV1 {
        self.layout
    }
}

pub(super) fn compute<D, C>(
    registrations: &super::TypeRegistrationDataV1<D, C>,
    objects: &std::collections::BTreeMap<SlibMemberId, &[u8]>,
) -> Result<Vec<TypeDependencyFingerprintV1>, StrongTypeDependencyFingerprintError>
where
    D: super::LinkDescriptorReference,
    C: super::LinkDispatchCallableReference + Clone,
{
    let builtins = registrations.builtins();
    let planned = registrations.plan().registrations();
    let mut fingerprints = Vec::with_capacity(planned.len());
    for plan in planned {
        let exact_type = plan.exact_type();
        let definition = plan.descriptor_definition_plan();
        let member_id = builtins
            .member_plan()
            .member_for_definition(definition)
            .ok_or(StrongTypeDependencyFingerprintError::MissingDefinition(
                definition,
            ))?;
        let member = builtins.member(member_id).ok_or(
            StrongTypeDependencyFingerprintError::MissingMember(member_id),
        )?;
        let object = objects.get(&member_id).copied().ok_or(
            StrongTypeDependencyFingerprintError::MissingObject(member_id),
        )?;
        let descriptor_bytes =
            atom_bytes(member, object, definition, plan.descriptor_primary_atom())?;
        let diagnostic_bytes = atom_bytes(member, object, definition, plan.diagnostic_atom())?;
        let itable_directory = match plan.itable_directory() {
            scoop_lir::TypeDescriptorITableDirectoryV1::Null => {
                TypeDescriptorITableDirectoryFingerprintInputV1::Null
            }
            scoop_lir::TypeDescriptorITableDirectoryV1::Defined(atom) => {
                TypeDescriptorITableDirectoryFingerprintInputV1::Defined {
                    atom,
                    bytes: atom_bytes(member, object, definition, atom)?,
                }
            }
        };
        let descriptor_definition =
            descriptor_fingerprint(descriptor_bytes, diagnostic_bytes, itable_directory, plan)
                .map_err(
                    |source| StrongTypeDependencyFingerprintError::DescriptorHash {
                        exact_type,
                        source,
                    },
                )?;
        let layout = layout_fingerprint(plan).map_err(|source| {
            StrongTypeDependencyFingerprintError::LayoutHash { exact_type, source }
        })?;
        fingerprints.push(TypeDependencyFingerprintV1 {
            exact_type,
            descriptor_definition_node: plan.descriptor_definition_node(),
            descriptor_definition,
            layout_node: plan.layout_fingerprint_node(),
            layout,
        });
    }

    Ok(fingerprints)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongTypeDependencyFingerprintError {
    MissingDefinition(scoop_identity::ObjectDefinitionPlanId),
    MissingMember(SlibMemberId),
    MissingObject(SlibMemberId),
    MissingAtom(scoop_identity::ObjectDefinitionAtomId),
    Range {
        atom: scoop_identity::ObjectDefinitionAtomId,
    },
    DescriptorHash {
        exact_type: PersistentExactTypeId,
        source: HashError,
    },
    LayoutHash {
        exact_type: PersistentExactTypeId,
        source: HashError,
    },
}

impl fmt::Display for StrongTypeDependencyFingerprintError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "failed to compute strong type dependency fingerprints: {self:?}"
        )
    }
}

impl std::error::Error for StrongTypeDependencyFingerprintError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::DescriptorHash { source, .. } | Self::LayoutHash { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
