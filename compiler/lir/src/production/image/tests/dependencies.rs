use super::*;
use crate::production::image::tests::image_fixture;

fn provider(name: &str) -> ConeIdentity {
    ConeCoordinate::new("test", name, "1.0.0")
        .unwrap()
        .identity()
        .unwrap()
}

#[test]
fn image_builder_rejects_self_and_duplicate_dependencies() {
    let coordinate = ConeCoordinate::reserved_single_file();
    let (foundation, digests) = image_fixture(coordinate.clone(), None, true, true);
    let registrations = RegistrationIdentitySurfaceV1::from_foundation(&foundation).unwrap();
    for (dependencies, error) in [
        (
            vec![ConeIdentity::SINGLE_FILE],
            ConeImagePlanBuildError::SelfDependency(ConeIdentity::SINGLE_FILE),
        ),
        (
            vec![provider("first"); 2],
            ConeImagePlanBuildError::DuplicateDependency(provider("first")),
        ),
    ] {
        assert_eq!(
            ConeImagePlanV1::new(
                coordinate.clone(),
                &dependencies,
                &foundation,
                &registrations,
                &digests,
            ),
            Err(error),
        );
    }
}
