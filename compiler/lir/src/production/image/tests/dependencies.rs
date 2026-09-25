use super::*;
use crate::production::image::tests::image_fixture;
use scoop_wire::decode_canonical;

fn provider(name: &str) -> ConeIdentity {
    ConeCoordinate::new("test", name, "1.0.0")
        .unwrap()
        .identity()
        .unwrap()
}

#[test]
fn explicit_dependencies_are_preserved_for_every_producer() {
    for coordinate in [
        ConeCoordinate::reserved_core(),
        ConeCoordinate::reserved_single_file(),
        ConeCoordinate::new("test", "consumer", "1.0.0").unwrap(),
    ] {
        let (foundation, digests) = image_fixture(coordinate.clone(), None, true, true);
        let registrations =
            StrongRegistrationIdentitySurfaceV1::from_foundation(&foundation, &digests).unwrap();
        for dependencies in [vec![], vec![provider("first"), provider("second")]] {
            let mut expected = dependencies.clone();
            expected.sort_unstable();
            for input in [
                dependencies.clone(),
                dependencies.into_iter().rev().collect(),
            ] {
                let plan = ConeImagePlanV1::new(
                    coordinate.clone(),
                    &input,
                    &foundation,
                    &registrations,
                    &digests,
                )
                .unwrap();
                assert_eq!(plan.dependencies(), expected);
                let bytes = encode(&plan).unwrap();
                let decoded: DecodedConeImagePlanV1 = decode_canonical(&bytes).unwrap();
                let replayed = decoded
                    .validate(
                        &coordinate,
                        &expected,
                        &foundation,
                        &registrations,
                        &digests,
                    )
                    .unwrap();
                assert_eq!(encode(&replayed).unwrap(), bytes);
            }
        }
    }
}

#[test]
fn image_builder_rejects_self_and_duplicate_dependencies() {
    let coordinate = ConeCoordinate::reserved_single_file();
    let (foundation, digests) = image_fixture(coordinate.clone(), None, true, true);
    let registrations =
        StrongRegistrationIdentitySurfaceV1::from_foundation(&foundation, &digests).unwrap();
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

#[test]
fn reader_rejects_missing_extra_reordered_and_duplicate_image_dependencies() {
    let coordinate = ConeCoordinate::reserved_single_file();
    let dependencies = [provider("first"), provider("second")];
    let (foundation, digests) = image_fixture(coordinate.clone(), None, true, true);
    let registrations =
        StrongRegistrationIdentitySurfaceV1::from_foundation(&foundation, &digests).unwrap();
    let plan = ConeImagePlanV1::new(
        coordinate.clone(),
        &dependencies,
        &foundation,
        &registrations,
        &digests,
    )
    .unwrap();
    let bytes = encode(&plan).unwrap();
    for case in ["missing", "extra", "reordered", "duplicate", "wrong"] {
        let mut decoded: DecodedConeImagePlanV1 = decode_canonical(&bytes).unwrap();
        match case {
            "missing" => {
                decoded.dependencies.pop();
            }
            "extra" => decoded
                .dependencies
                .push(decode_canonical(&encode(&provider("third")).unwrap()).unwrap()),
            "reordered" => decoded.dependencies.swap(0, 1),
            "duplicate" => decoded.dependencies[1] = decoded.dependencies[0],
            "wrong" => {
                decoded.dependencies[0] =
                    decode_canonical(&encode(&provider("third")).unwrap()).unwrap();
            }
            _ => unreachable!(),
        }
        assert!(
            matches!(
                decoded.validate(
                    &coordinate,
                    &dependencies,
                    &foundation,
                    &registrations,
                    &digests,
                ),
                Err(ConeImagePlanValidationError::PlanMismatch)
            ),
            "{case}"
        );
    }
}
