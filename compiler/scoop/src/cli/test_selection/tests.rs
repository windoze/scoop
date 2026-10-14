use super::*;
use scoop_slib::{TestCaseListing, TestContainerListing, TestSourceLocation};

fn directory() -> TestDirectory {
    let location = TestSourceLocation {
        path: "tests/cases.scoop".into(),
        line: 1,
        column: 1,
    };
    let case = |name: &str, ignored: bool| TestCaseListing {
        name: name.into(),
        display_name: format!("cone::Cases::{name}"),
        location: location.clone(),
        ignore_reason: ignored.then(String::new),
    };
    TestDirectory {
        containers: vec![
            TestContainerListing {
                qualified_name: "Cases".into(),
                location: location.clone(),
                cases: vec![
                    case("first 雪", false),
                    case("ignored", true),
                    case("second", false),
                ],
            },
            TestContainerListing {
                qualified_name: "Empty".into(),
                location,
                cases: Vec::new(),
            },
        ],
    }
}

#[test]
fn selection_keeps_compiled_indices_and_removes_empty_groups() {
    let directory = directory();
    for (filter, ignored, expected) in [
        (None, false, vec!["0", "2"]),
        (Some("Cases::first 雪"), false, vec!["0"]),
        (Some("Cases::"), true, vec!["1"]),
        (Some(""), false, vec!["0", "2"]),
    ] {
        let selected = select(&directory, filter, ignored);
        assert_eq!(selected.directory.containers.len(), 1);
        assert_eq!(selected.directory.containers[0].cases.len(), expected.len());
        assert_eq!(
            &selected.arguments[1..],
            expected.into_iter().map(OsString::from).collect::<Vec<_>>()
        );
        assert_eq!(selected.empty_reason, None);
    }
}

#[test]
fn empty_selection_retains_its_reason_and_explicit_empty_run() {
    for (directory, filter, ignored, reason) in [
        (TestDirectory::default(), None, false, "no tests"),
        (directory(), Some("FIRST"), false, "no matching tests"),
        (directory(), Some("first"), true, "no ignored tests"),
        (
            directory(),
            Some("ignored"),
            false,
            "all matching tests ignored",
        ),
    ] {
        let selected = select(&directory, filter, ignored);
        assert!(selected.directory.containers.is_empty());
        assert_eq!(selected.empty_reason, Some(reason));
        assert_eq!(
            selected.arguments,
            vec![OsString::from("--scoop-test-cases")]
        );
    }
}
