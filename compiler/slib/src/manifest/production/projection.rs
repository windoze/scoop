//! Shared production projection reconstructed from plans and final objects.

use super::*;

mod inputs;
mod registrations;
pub(crate) use inputs::ProductionPlanInputs;
use registrations::registration_identities_match;

pub(crate) fn verify_production_code_projection_common<D, C, I>(
    cone: &ConeRecord,
    dependency_identities: &[scoop_identity::ConeIdentity],
    source_count: usize,
    plans: ProductionPlanInputs<'_>,
    link_objects: &VerifiedCodeLinkObjectMemberSetV1<D, C, I>,
) -> Result<SingleConeProductionCodeProjectionV1, ProductionCodeProjectionError>
where
    D: scoop_lir::StrongDescriptorReference,
    C: Clone,
{
    let final_objects = link_objects.final_objects();
    let runtime_image = final_objects.runtime_images().fingerprint();
    let registrations = runtime_image.registrations();

    if link_objects.producer() != cone.identity()
        || plans.image.cone().identity() != cone.identity()
        || plans.image.cone().coordinate() != cone.coordinate()
    {
        return Err(ProductionCodeProjectionError::ConeMismatch);
    }
    if plans.digest != final_objects.entry().patch_sites().digest_plan() {
        return Err(ProductionCodeProjectionError::DigestPlanMismatch);
    }
    if plans.image != runtime_image.image().plan() {
        return Err(ProductionCodeProjectionError::ImagePlanMismatch);
    }
    if plans.entry != final_objects.entry().plan() {
        return Err(ProductionCodeProjectionError::EntryPlanMismatch);
    }
    if plans.generated
        != final_objects
            .entry()
            .patch_sites()
            .builtins()
            .c_bridge_production()
            .bridge_plan()
    {
        return Err(ProductionCodeProjectionError::GeneratedBridgePlanMismatch);
    }
    if !registration_identities_match(plans.identities, registrations) {
        return Err(ProductionCodeProjectionError::RegistrationIdentityMismatch);
    }

    if dependency_identities != plans.image.dependencies() {
        return Err(ProductionCodeProjectionError::DependencyMismatch);
    }

    let distribution = distribution(cone, dependency_identities, source_count)?;
    let output = output(cone, final_objects)?;
    let strong_registration_set =
        CanonicalStrongRegistrationFingerprintSetV1::from_patch_set(registrations)
            .map_err(ProductionCodeProjectionError::StrongRegistrations)?;
    let projection = SingleConeProductionCodeProjectionV1 {
        distribution,
        output,
        image_owner_member: runtime_image.image().member(),
        runtime_registration_projection: plans.identities.clone(),
        strong_registration_set,
        runtime_image_fingerprint: runtime_image.fingerprint(),
    };
    Ok(projection)
}

pub(super) fn distribution(
    cone: &ConeRecord,
    dependencies: &[scoop_identity::ConeIdentity],
    source_count: usize,
) -> Result<ArtifactDistributionClassV1, ProductionCodeProjectionError> {
    match cone.source_form() {
        ConeSourceForm::Manifest => Ok(ArtifactDistributionClassV1::DistributableCone),
        ConeSourceForm::SingleFile
            if cone.kind() == ConeKind::Executable
                && source_count == 1
                && dependencies == [scoop_identity::ConeIdentity::CORE] =>
        {
            Ok(ArtifactDistributionClassV1::LocalExecutableRoot)
        }
        ConeSourceForm::SingleFile => Err(ProductionCodeProjectionError::InvalidSingleFileRoot {
            kind: cone.kind(),
            source_count,
            dependencies: dependencies.to_vec(),
        }),
    }
}

fn output<D, C, I>(
    cone: &ConeRecord,
    final_objects: &crate::link_object::VerifiedEntryPatchSetV1<D, C, I>,
) -> Result<SingleConeProductionOutputV1, ProductionCodeProjectionError>
where
    D: scoop_lir::StrongDescriptorReference,
    C: Clone,
{
    match (
        cone.kind(),
        final_objects.entry().plan(),
        final_objects.entry().branch(),
    ) {
        (
            ConeKind::Library,
            EntryProductionPlanV1::Library,
            VerifiedEntryProductionBranchV1::Library,
        ) => Ok(SingleConeProductionOutputV1::Library),
        (
            ConeKind::Executable,
            EntryProductionPlanV1::Executable(plan),
            VerifiedEntryProductionBranchV1::Executable(entry),
        ) => {
            let gateway = final_objects
                .runtime_images()
                .fingerprint()
                .registrations()
                .callables()
                .fingerprints()
                .iter()
                .find(|fingerprint| fingerprint.body() == plan.gateway())
                .copied()
                .ok_or(ProductionCodeProjectionError::MissingGatewayFingerprint(
                    plan.gateway(),
                ))?;
            Ok(SingleConeProductionOutputV1::Executable(Box::new(
                ExecutableRootProjectionV1 {
                    main: plan.main(),
                    source_signature_fingerprint: plan.source_signature_fingerprint(),
                    gateway: plan.gateway(),
                    gateway_definition_fingerprint: gateway.body_definition(),
                    failure_root: plan.failure_root(),
                    entry_owner_member: entry.member(),
                },
            )))
        }
        _ => Err(ProductionCodeProjectionError::OutputMismatch),
    }
}
