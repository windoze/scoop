use scoop_lir::RegistrationDefinitionOwner;
use scoop_wire::{RuntimeEncodeError, RuntimeEncoder};

/// The relocation verifier already resolved this registration's local target.
pub(super) fn canonical_local_relocation(
    binding: &super::StrongRelocationBindingV1,
) -> super::callable_registrations::object_definition::CanonicalObjectRelocationV1 {
    use super::{
        FinalUndefinedSymbolRequirementV1 as Requirement, LinkDefinitionOwnerV1 as Owner,
        StrongRelocationResolutionV1 as Resolution,
    };
    let owner = match binding.resolution() {
        Resolution::ObjectLocalStrong { owner, .. }
        | Resolution::CurrentConeUndefinedStrong { owner, .. } => owner,
        Resolution::ExternalCandidate { .. } => {
            unreachable!("a local registration target is defined in this artifact")
        }
    };
    let requirement = match owner {
        Owner::StrongDefinition(owner) => Requirement::IntraConeStrong { owner },
        Owner::OdrDefinition(member) => Requirement::OdrMember { member },
        _ => unreachable!("a local registration target is a Strong or ODR definition"),
    };
    super::callable_registrations::object_definition::CanonicalObjectRelocationV1::unsigned64(
        binding.offset_within_atom(),
        requirement,
    )
}

pub(super) fn runtime_encode_registration_identity(
    encoder: &mut RuntimeEncoder,
    semantic: &[u8; 32],
    owner: RegistrationDefinitionOwner,
    definition: &[u8; 32],
) -> Result<(), RuntimeEncodeError> {
    let (linkage, group, member) = match owner {
        RegistrationDefinitionOwner::Strong => (1, [0; 32], [0; 32]),
        RegistrationDefinitionOwner::Odr { group, member } => {
            (2, *group.as_array(), *member.as_array())
        }
    };
    encoder.u32(linkage)?;
    encoder.fixed(semantic)?;
    encoder.fixed(&group)?;
    encoder.fixed(&member)?;
    encoder.fixed(definition)
}

pub(super) fn provisional_registration_identity(
    semantic: &[u8; 32],
    owner: RegistrationDefinitionOwner,
) -> [u8; 136] {
    let mut bytes = [0; 136];
    let linkage = match owner {
        RegistrationDefinitionOwner::Strong => 1_u32,
        RegistrationDefinitionOwner::Odr { group, member } => {
            bytes[40..72].copy_from_slice(group.as_array());
            bytes[72..104].copy_from_slice(member.as_array());
            2
        }
    };
    bytes[..4].copy_from_slice(&linkage.to_le_bytes());
    bytes[8..40].copy_from_slice(semantic);
    bytes
}
