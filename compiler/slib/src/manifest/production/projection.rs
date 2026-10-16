//! Shared production projection reconstructed from plans and final objects.

use super::*;

mod inputs;
pub(crate) use inputs::ProductionPlanInputs;

pub(crate) fn build_production_code_projection_common<D, C, I>(
    cone: &ConeRecord,
    dependency_identities: &[scoop_identity::ConeIdentity],
    plans: ProductionPlanInputs<'_>,
    link_objects: &FinalCodeObjectSetV1<D, C, I>,
) -> Result<SingleConeProductionCodeProjectionV1, ProductionCodeProjectionError>
where
    D: scoop_lir::StrongDescriptorReference,
    C: Clone,
{
    let final_objects = link_objects.final_objects();
    let runtime_image = final_objects.runtime_images().fingerprint();
    let registrations = runtime_image.registrations();

    let distribution = link_distribution(cone, dependency_identities)?;
    let output = output(final_objects)?;
    let odr_members = CanonicalOdrMemberDirectoryV1::from_patch_set(registrations)
        .map_err(ProductionCodeProjectionError::OdrMembers)?;
    let projection = SingleConeProductionCodeProjectionV1 {
        distribution,
        output,
        image_owner_member: runtime_image.image().member(),
        runtime_registration_projection: plans.identities.clone(),

        runtime_image_fingerprint: runtime_image.fingerprint(),
        odr_members,
        optimization: scoop_lir::OptimizationMode::Debug,
    };
    Ok(projection)
}

pub(crate) fn distribution(
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

fn link_distribution(
    cone: &ConeRecord,
    dependencies: &[scoop_identity::ConeIdentity],
) -> Result<ArtifactDistributionClassV1, ProductionCodeProjectionError> {
    match cone.source_form() {
        ConeSourceForm::Manifest => Ok(ArtifactDistributionClassV1::DistributableCone),
        ConeSourceForm::SingleFile
            if cone.kind() == ConeKind::Executable
                && dependencies == [scoop_identity::ConeIdentity::CORE] =>
        {
            Ok(ArtifactDistributionClassV1::LocalExecutableRoot)
        }
        ConeSourceForm::SingleFile => Err(ProductionCodeProjectionError::InvalidSingleFileLinkRoot),
    }
}

fn output<D, C, I>(
    final_objects: &crate::link_object::VerifiedEntryPatchSetV1<D, C, I>,
) -> Result<SingleConeProductionOutputV1, ProductionCodeProjectionError>
where
    D: scoop_lir::StrongDescriptorReference,
    C: Clone,
{
    match final_objects.entry().branch() {
        EntryProductionBranchV1::Library => Ok(SingleConeProductionOutputV1::Library),
        EntryProductionBranchV1::Executable(entry) => {
            let plan = entry.plan();
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
    }
}
