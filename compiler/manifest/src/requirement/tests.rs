use super::*;

#[test]
fn compatibility_and_prerelease_boundaries() {
    for (requirement, accepted, rejected) in [
        ("1.4.0", "1.9.8", "2.0.0"),
        ("^0.3.2", "0.3.9", "0.4.0"),
        ("0.0.3", "0.0.3", "0.0.4"),
        ("~1.4.2", "1.4.8", "1.5.0"),
        (">=1.4.0, <1.8.0", "1.7.9", "1.8.0"),
        ("^1.4.0-rc.1", "1.4.0-rc.2", "1.5.0-rc.1"),
        ("^1.4.0-rc.1", "1.5.0", "1.4.0-beta.1"),
        ("^1.4.0", "1.5.0+build.3", "1.5.0-rc.1"),
        ("=1.4.2", "1.4.2", "1.4.2+build.1"),
        ("=1.4.2+build.1", "1.4.2+build.1", "1.4.2+build.2"),
        (">=1.0.0, =1.4.2", "1.4.2", "1.4.2+build.1"),
    ] {
        let parsed: VersionRequirement = requirement.parse().unwrap();
        assert!(
            parsed.matches(&Version::parse(accepted).unwrap()),
            "{requirement}: {accepted}"
        );
        assert!(
            !parsed.matches(&Version::parse(rejected).unwrap()),
            "{requirement}: {rejected}"
        );
    }
}

#[test]
fn requires_complete_versions_and_supported_operators() {
    for text in [
        "",
        "*",
        "1",
        "1.2",
        "1.*",
        ">=1.0",
        "latest",
        "v1.0.0",
        "01.0.0",
        "^1.2.3+local",
        "1.0.0 || 2.0.0",
        ">=1.0.0,",
        "^1.0.0, <2.0.0",
    ] {
        assert!(text.parse::<VersionRequirement>().is_err(), "{text}");
    }
}

#[test]
fn exact_locators_require_one_explicit_equality() {
    for text in ["1.0.0", "^1.0.0", ">=1.0.0, <=1.0.0", "=1.0.0, <2.0.0"] {
        assert!(
            text.parse::<VersionRequirement>()
                .unwrap()
                .exact_version()
                .is_none()
        );
    }
    assert_eq!(
        "=1.2.3+local"
            .parse::<VersionRequirement>()
            .unwrap()
            .exact_version(),
        Some(&Version::parse("1.2.3+local").unwrap())
    );
}
