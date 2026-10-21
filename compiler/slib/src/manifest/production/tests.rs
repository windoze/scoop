use scoop_wire::encode;

use super::*;

#[test]
fn distribution_and_library_output_have_fixed_closed_sum_vectors() {
    assert_eq!(
        encode(&ArtifactDistributionClassV1::DistributableCone).unwrap(),
        vec![0xa1, 0x00, 0x01]
    );
    assert_eq!(
        encode(&ArtifactDistributionClassV1::LocalExecutableRoot).unwrap(),
        vec![0xa1, 0x00, 0x02]
    );
    assert_eq!(
        encode(&SingleConeProductionOutputV1::Library).unwrap(),
        vec![0xa1, 0x00, 0x01]
    );
}

#[test]
fn distribution_rejects_a_single_file_that_is_not_the_exact_local_root_shape() {
    let cone = ConeRecord::new(
        scoop_identity::ConeCoordinate::reserved_single_file(),
        ConeKind::Executable,
        ConeSourceForm::SingleFile,
    )
    .unwrap();
    assert_eq!(
        distribution(&cone, &[], 1),
        Ok(ArtifactDistributionClassV1::LocalExecutableRoot)
    );
    assert_eq!(
        distribution(&cone, &[scoop_identity::ConeIdentity::CORE], 2),
        Err(ProductionCodeProjectionError::InvalidSingleFileRoot {
            kind: ConeKind::Executable,
            source_count: 2,
            dependencies: vec![scoop_identity::ConeIdentity::CORE],
        })
    );
    assert_eq!(
        distribution(&cone, &[scoop_identity::ConeIdentity::CORE], 1),
        Ok(ArtifactDistributionClassV1::LocalExecutableRoot)
    );
    let std = scoop_identity::ConeCoordinate::new("scoop", "scoop.std", "0.1.0")
        .unwrap()
        .identity()
        .unwrap();
    assert_eq!(
        distribution(&cone, &[scoop_identity::ConeIdentity::CORE, std], 1),
        Ok(ArtifactDistributionClassV1::LocalExecutableRoot)
    );
}
