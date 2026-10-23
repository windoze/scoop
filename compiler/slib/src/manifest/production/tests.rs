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
