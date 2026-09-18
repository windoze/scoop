use super::*;

fn decoded_record(
    source: &str,
    point_offsets: impl IntoIterator<Item = u64>,
) -> DecodedSourceRecord {
    let record = SourceRecord::from_utf8(
        scoop_identity::SourceIdentity::single_file(),
        source,
        point_offsets,
    )
    .unwrap();
    scoop_wire::decode_canonical(
        &scoop_wire::encode(&record).unwrap(),
        scoop_wire::DecodeLimits::default(),
    )
    .unwrap()
}

#[test]
fn producer_builds_canonical_utf8_crlf_points_and_wire() {
    let source = "a\r\n雪\n";
    let record = SourceRecord::from_utf8(
        scoop_identity::SourceIdentity::single_file(),
        source,
        [7, 2, 3, 6, 0, 3],
    )
    .unwrap();

    assert_eq!(record.byte_length(), 7);
    assert_eq!(
        record.content_digest().to_string(),
        "da657a9242d365403e34959133543789fad5d1d7ba0a1a21011fa09c6cbc299d"
    );
    assert_eq!(record.line_starts(), &[0, 3, 7]);
    assert_eq!(
        record.points(),
        &[
            SourcePointRecord {
                byte_offset: 0,
                line: 1,
                column: 1,
            },
            SourcePointRecord {
                byte_offset: 2,
                line: 1,
                column: 3,
            },
            SourcePointRecord {
                byte_offset: 3,
                line: 2,
                column: 1,
            },
            SourcePointRecord {
                byte_offset: 6,
                line: 2,
                column: 2,
            },
            SourcePointRecord {
                byte_offset: 7,
                line: 3,
                column: 1,
            },
        ]
    );
    assert_eq!(record.point(6).unwrap().column(), 2);
    assert!(record.point(1).is_none());

    assert_eq!(
        scoop_wire::encode(&record).unwrap(),
        [
            b"\xa5\x01\xa2\x01\x58\x20".as_slice(),
            scoop_identity::ConeIdentity::SINGLE_FILE.as_array(),
            b"\x02\x6amain.scoop\x02\x07\x03\x58\x20".as_slice(),
            record.content_digest().as_array(),
            b"\x04\x83\x00\x03\x07\x05\x85\
              \xa3\x01\x00\x02\x01\x03\x01\
              \xa3\x01\x02\x02\x01\x03\x03\
              \xa3\x01\x03\x02\x02\x03\x01\
              \xa3\x01\x06\x02\x02\x03\x02\
              \xa3\x01\x07\x02\x03\x03\x01"
                .as_slice(),
        ]
        .concat()
    );
}

#[test]
fn producer_rejects_invalid_source_point_offsets() {
    let identity = scoop_identity::SourceIdentity::single_file();
    assert_eq!(
        SourceRecord::from_utf8(identity.clone(), "雪", [1]),
        Err(SourceRecordError::PointNotUtf8Boundary { byte_offset: 1 })
    );
    assert_eq!(
        SourceRecord::from_utf8(identity, "雪", [4]),
        Err(SourceRecordError::PointPastEnd {
            byte_offset: 4,
            byte_length: 3,
        })
    );
}

#[test]
fn reader_validates_canonical_record_with_or_without_source_attachment() {
    let source = "a\r\n雪\n";
    let expected = SourceRecord::from_utf8(
        scoop_identity::SourceIdentity::single_file(),
        source,
        [0, 2, 3, 6, 7],
    )
    .unwrap();
    let decoded = decoded_record(source, [0, 2, 3, 6, 7]);
    let coordinate = scoop_identity::ConeCoordinate::reserved_single_file();

    assert_eq!(
        decoded.clone().validate(&coordinate, None),
        Ok(expected.clone())
    );
    assert_eq!(decoded.validate(&coordinate, Some(source)), Ok(expected));
}

#[test]
fn reader_resolves_a_dependency_source_through_validated_cone_authority() {
    let coordinate =
        scoop_identity::ConeCoordinate::new("dev.example", "provider", "1.0.0").unwrap();
    let source = scoop_identity::SourceIdentity::new(
        coordinate.identity().unwrap(),
        scoop_identity::NormalizedSourcePath::new("src/api.scoop").unwrap(),
    )
    .unwrap();
    let expected = SourceRecord::from_utf8(source, "abc", [0, 3]).unwrap();
    let decoded: DecodedSourceRecord = scoop_wire::decode_canonical(
        &scoop_wire::encode(&expected).unwrap(),
        scoop_wire::DecodeLimits::default(),
    )
    .unwrap();
    let mut pending = scoop_identity::PendingIdentityValidation::new();
    pending
        .register_authority(coordinate.identity().unwrap())
        .unwrap();
    let mut identities = pending.finish().unwrap();

    assert_eq!(decoded.resolve(&mut identities, None), Ok(expected));
}

#[test]
fn required_source_points_must_be_present() {
    let record = decoded_record("abc", [0, 3])
        .validate(
            &scoop_identity::ConeCoordinate::reserved_single_file(),
            None,
        )
        .unwrap();

    assert_eq!(record.require_points([0, 3]), Ok(()));
    assert_eq!(
        record.require_points([0, 2]),
        Err(MissingSourcePointError { byte_offset: 2 })
    );
}

#[test]
fn reader_rejects_invalid_line_start_tables() {
    let coordinate = scoop_identity::ConeCoordinate::reserved_single_file();

    let mut missing_initial = decoded_record("abc", []);
    missing_initial.line_starts.clear();
    assert_eq!(
        missing_initial.validate(&coordinate, None),
        Err(SourceRecordValidationError::MissingInitialLineStart)
    );

    let mut nonzero_initial = decoded_record("abc", []);
    nonzero_initial.line_starts = vec![1];
    assert_eq!(
        nonzero_initial.validate(&coordinate, None),
        Err(SourceRecordValidationError::MissingInitialLineStart)
    );

    let mut duplicate = decoded_record("abc", []);
    duplicate.line_starts = vec![0, 0];
    assert_eq!(
        duplicate.validate(&coordinate, None),
        Err(SourceRecordValidationError::NonIncreasingLineStarts {
            first_index: 0,
            second_index: 1,
        })
    );

    let mut past_end = decoded_record("abc", []);
    past_end.line_starts = vec![0, 4];
    assert_eq!(
        past_end.validate(&coordinate, None),
        Err(SourceRecordValidationError::LineStartPastEnd {
            byte_offset: 4,
            byte_length: 3,
        })
    );
}

#[test]
fn reader_rejects_invalid_structural_source_points() {
    let coordinate = scoop_identity::ConeCoordinate::reserved_single_file();

    let mut duplicate = decoded_record("abc", [0]);
    duplicate.points.push(duplicate.points[0]);
    assert_eq!(
        duplicate.validate(&coordinate, None),
        Err(SourceRecordValidationError::NonIncreasingPoints {
            first_index: 0,
            second_index: 1,
        })
    );

    let mut past_end = decoded_record("abc", [0]);
    past_end.points[0].byte_offset = 4;
    assert_eq!(
        past_end.validate(&coordinate, None),
        Err(SourceRecordValidationError::PointPastEnd {
            index: 0,
            byte_offset: 4,
            byte_length: 3,
        })
    );

    let mut zero_line = decoded_record("abc", [0]);
    zero_line.points[0].line = 0;
    assert_eq!(
        zero_line.validate(&coordinate, None),
        Err(SourceRecordValidationError::InvalidPointLine { index: 0, line: 0 })
    );

    let mut undeclared_line = decoded_record("abc", [0]);
    undeclared_line.points[0].line = 2;
    assert_eq!(
        undeclared_line.validate(&coordinate, None),
        Err(SourceRecordValidationError::InvalidPointLine { index: 0, line: 2 })
    );

    let mut zero_column = decoded_record("abc", [0]);
    zero_column.points[0].column = 0;
    assert_eq!(
        zero_column.validate(&coordinate, None),
        Err(SourceRecordValidationError::ZeroPointColumn { index: 0 })
    );

    let mut outside_line = decoded_record("a\nb", [0]);
    outside_line.points[0].byte_offset = 2;
    assert_eq!(
        outside_line.validate(&coordinate, None),
        Err(SourceRecordValidationError::PointOutsideDeclaredLine {
            index: 0,
            byte_offset: 2,
            line: 1,
        })
    );
}

#[test]
fn source_attachment_strengthens_structural_validation() {
    let coordinate = scoop_identity::ConeCoordinate::reserved_single_file();

    let mut continuation = decoded_record("雪", [0]);
    continuation.points[0].byte_offset = 1;
    continuation.points[0].column = 2;
    assert!(continuation.clone().validate(&coordinate, None).is_ok());
    assert_eq!(
        continuation.validate(&coordinate, Some("雪")),
        Err(SourceRecordValidationError::SourceAttachment(
            SourceRecordError::PointNotUtf8Boundary { byte_offset: 1 }
        ))
    );

    let mut wrong_length = decoded_record("雪", [0]);
    wrong_length.byte_length = 4;
    assert_eq!(
        wrong_length.validate(&coordinate, Some("雪")),
        Err(SourceRecordValidationError::SourceLengthMismatch {
            declared: 4,
            actual: 3,
        })
    );

    let mut wrong_digest = decoded_record("abc", []);
    wrong_digest.content_digest = decoded_record("abd", []).content_digest;
    assert_eq!(
        wrong_digest.validate(&coordinate, Some("abc")),
        Err(SourceRecordValidationError::SourceDigestMismatch)
    );

    let mut wrong_line_starts = decoded_record("a\n", []);
    wrong_line_starts.line_starts = vec![0, 1];
    assert_eq!(
        wrong_line_starts.validate(&coordinate, Some("a\n")),
        Err(SourceRecordValidationError::SourceLineStartsMismatch)
    );

    let mut wrong_column = decoded_record("abc", [1]);
    wrong_column.points[0].column = 3;
    assert_eq!(
        wrong_column.validate(&coordinate, Some("abc")),
        Err(SourceRecordValidationError::SourcePointsMismatch)
    );
}
