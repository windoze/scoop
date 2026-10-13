use super::*;

fn example() -> TestDirectory {
    let location = TestSourceLocation {
        path: "tests/名字.scoop".into(),
        line: 7,
        column: 3,
    };
    TestDirectory {
        containers: vec![TestContainerListing {
            qualified_name: "example.Tests".into(),
            location: location.clone(),
            cases: vec![TestCaseListing {
                name: "case::名字".into(),
                display_name: "cone::example.Tests::case::名字".into(),
                location,
                ignore_reason: Some(String::new()),
            }],
        }],
    }
}

#[test]
fn directory_preserves_relationships_unicode_and_empty_ignore_reason() {
    let directory = example();
    let section = directory.section().unwrap();
    assert_eq!(section.required_for(), MemberPurposeSet::NONE);
    assert_eq!(TestDirectory::read(section.payload()).unwrap(), directory);
    let empty = TestDirectory::default();
    assert_eq!(
        TestDirectory::read(empty.section().unwrap().payload()).unwrap(),
        empty
    );
}

#[test]
fn directory_rejects_invalid_locations_and_trailing_payload() {
    for (path, line, column) in [
        ("../escape.scoop", 1, 1),
        ("tests/./case.scoop", 1, 1),
        ("tests/case.scoop", 0, 1),
        ("tests/case.scoop", 1, 0),
    ] {
        let mut directory = example();
        directory.containers[0].cases[0].location = TestSourceLocation {
            path: path.into(),
            line,
            column,
        };
        assert!(TestDirectory::read(&scoop_wire::encode(&directory).unwrap()).is_err());
    }
    let mut payload = scoop_wire::encode(&example()).unwrap();
    payload.push(0);
    assert!(TestDirectory::read(&payload).is_err());
}
