use scoop_source::{PositionEncoding, PositionError, SourceText, TextPosition};

#[test]
fn unicode_offsets_round_trip_in_all_supported_encodings() {
    let source = SourceText::new("Aé中😀e\u{301}\0".to_owned());
    let points = [
        (0, 0, 0),
        (1, 1, 1),
        (3, 2, 2),
        (6, 3, 3),
        (10, 5, 4),
        (11, 6, 5),
        (13, 7, 6),
        (14, 8, 7),
    ];
    for (byte, utf16, utf32) in points {
        for (encoding, column) in [
            (PositionEncoding::Utf8, byte),
            (PositionEncoding::Utf16, utf16),
            (PositionEncoding::Utf32, utf32),
        ] {
            let position = TextPosition::new(0, column);
            assert_eq!(source.line_index().position(byte, encoding), Ok(position));
            assert_eq!(source.line_index().offset(position, encoding), Ok(byte));
        }
    }
}

#[test]
fn universal_newlines_preserve_original_bytes_and_empty_final_line() {
    let source = SourceText::new("a\r\n中\rb\n".to_owned());
    let index = source.line_index();
    assert_eq!(source.as_str(), "a\r\n中\rb\n");
    assert_eq!(index.line_count(), 4);
    for (line, range) in [0..1, 3..6, 7..8, 9..9].into_iter().enumerate() {
        assert_eq!(index.line_range(line), Ok(range));
    }
    for encoding in [
        PositionEncoding::Utf8,
        PositionEncoding::Utf16,
        PositionEncoding::Utf32,
    ] {
        assert_eq!(
            index.position(2, encoding),
            Err(PositionError::InsideLineEnding)
        );
        assert_eq!(index.position(3, encoding), Ok(TextPosition::new(1, 0)));
        assert_eq!(index.position(9, encoding), Ok(TextPosition::new(3, 0)));
        assert_eq!(
            index.offset(TextPosition::new(1, usize::MAX), encoding),
            Ok(6)
        );
    }
}

#[test]
fn partial_unicode_units_and_out_of_range_locations_are_rejected() {
    let source = SourceText::new("x😀y".to_owned());
    let index = source.line_index();
    for offset in 2..5 {
        assert_eq!(
            index.position(offset, PositionEncoding::Utf8),
            Err(PositionError::InsideCharacter)
        );
    }
    assert_eq!(
        index.offset(TextPosition::new(0, 2), PositionEncoding::Utf16),
        Err(PositionError::InsideCharacter)
    );
    assert_eq!(
        index.offset(TextPosition::new(0, 3), PositionEncoding::Utf8),
        Err(PositionError::InsideCharacter)
    );
    assert_eq!(
        index.offset(TextPosition::new(1, 0), PositionEncoding::Utf32),
        Err(PositionError::LineOutOfBounds {
            line: 1,
            line_count: 1,
        })
    );
    assert_eq!(
        index.position(7, PositionEncoding::Utf32),
        Err(PositionError::OffsetOutOfBounds {
            offset: 7,
            length: 6,
        })
    );
}

#[test]
fn empty_text_has_one_position_and_clamps_columns() {
    let source = SourceText::new(String::new());
    let index = source.line_index();
    assert_eq!(index.line_count(), 1);
    assert_eq!(index.line_range(0), Ok(0..0));
    assert_eq!(
        index.offset(TextPosition::new(0, usize::MAX), PositionEncoding::Utf16),
        Ok(0)
    );
    assert_eq!(
        index.position(0, PositionEncoding::Utf16),
        Ok(TextPosition::new(0, 0))
    );
}
