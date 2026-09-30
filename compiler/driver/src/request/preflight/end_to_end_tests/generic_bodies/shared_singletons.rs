#[test]
fn singleton_reads_share_initialization_and_storage_through_artifacts() {
    super::members::check_fixture_cases(
        "m23-shared-singleton-reads",
        &["standalone", "combined"],
        &["bad-local-nogc", "bad-dependency-nogc", "bad-default-type"],
        "downstream",
    );
}
