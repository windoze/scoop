//! Unit tests for the canonical CBOR codec and identity primitives.

use crate::capability::{
    CapabilityError, CapabilityId, ObjectFormatId, TargetProfileWireId,
    generated_c_bridge_link_object_v1, scoop_lir_link_object_v1,
};
use crate::cbor::{CborError, CborReader, CborWriter};
use crate::coordinate::{ConeCoordinate, CoordinateError, Version};
use crate::digest::DomainHasher;
use crate::{ConeIdentity, Digest256};

/// Encodes one small record for round-trip checks:
/// `{1: 42, 2: b"abc", 3: "text", 4: [10, 20], 5: {0: 2, 1: -1}}`.
fn sample_record() -> Vec<u8> {
    let mut writer = CborWriter::new();
    writer.map(5);
    writer.field(1).unsigned(42);
    writer.field(2).bytes(b"abc");
    writer.field(3).text("text");
    writer.field(4).array(2).unsigned(10).unsigned(20);
    writer
        .field(5)
        .map(2)
        .field(0)
        .unsigned(2)
        .field(1)
        .negative(-1);
    writer.into_bytes()
}

#[test]
fn cbor_round_trip() {
    let data = sample_record();
    let mut reader = CborReader::new(&data, 16);
    let mut record = reader.map().expect("map header");
    assert_eq!(record.next_key(), Ok(Some(1)));
    assert_eq!(record.unsigned(), Ok(42));
    assert_eq!(record.next_key(), Ok(Some(2)));
    assert_eq!(record.bytes(), Ok(b"abc".as_slice()));
    assert_eq!(record.next_key(), Ok(Some(3)));
    assert_eq!(record.text(), Ok("text"));
    assert_eq!(record.next_key(), Ok(Some(4)));
    {
        let mut items = record.array().expect("array header");
        assert_eq!(items.count(), 2);
        assert_eq!(items.unsigned(), Ok(10));
        assert_eq!(items.unsigned(), Ok(20));
    }
    assert_eq!(record.next_key(), Ok(Some(5)));
    {
        let mut inner = record.map().expect("inner map header");
        assert_eq!(inner.next_key(), Ok(Some(0)));
        assert_eq!(inner.unsigned(), Ok(2));
        assert_eq!(inner.next_key(), Ok(Some(1)));
        assert_eq!(inner.integer(), Ok(-1));
        assert_eq!(inner.next_key(), Ok(None));
    }
    assert_eq!(record.next_key(), Ok(None));
    drop(record);
    reader.finish().expect("fully consumed");
}

#[test]
fn cbor_shortest_integer_encodings() {
    // Boundary values and their exact canonical encodings.
    let cases: &[(u64, &[u8])] = &[
        (0, &[0x00]),
        (23, &[0x17]),
        (24, &[0x18, 0x18]),
        (255, &[0x18, 0xFF]),
        (256, &[0x19, 0x01, 0x00]),
        (65535, &[0x19, 0xFF, 0xFF]),
        (65536, &[0x1A, 0x00, 0x01, 0x00, 0x00]),
        (0xFFFF_FFFF, &[0x1A, 0xFF, 0xFF, 0xFF, 0xFF]),
        (0x1_0000_0000, &[0x1B, 0, 0, 0, 1, 0, 0, 0, 0]),
    ];
    for (value, expected) in cases {
        let mut writer = CborWriter::new();
        writer.unsigned(*value);
        assert_eq!(&writer.into_bytes(), expected, "encoding of {value}");
        let mut reader = CborReader::new(expected, 4);
        assert_eq!(reader.unsigned(), Ok(*value));
        reader.finish().expect("consumed");
    }
}

#[test]
fn cbor_rejects_non_canonical_integers() {
    let bad: &[(Vec<u8>, CborError)] = &[
        (
            vec![0x18, 0x00],
            CborError::NonCanonicalInteger { value: 0, info: 24 },
        ),
        (
            vec![0x18, 0x17],
            CborError::NonCanonicalInteger {
                value: 23,
                info: 24,
            },
        ),
        (
            vec![0x19, 0x00, 0xFF],
            CborError::NonCanonicalInteger {
                value: 0xFF,
                info: 25,
            },
        ),
        (
            vec![0x1A, 0x00, 0x00, 0xFF, 0xFF],
            CborError::NonCanonicalInteger {
                value: 0xFFFF,
                info: 26,
            },
        ),
        (
            vec![0x1B, 0, 0, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF],
            CborError::NonCanonicalInteger {
                value: 0xFFFF_FFFF,
                info: 27,
            },
        ),
    ];
    for (bytes, expected) in bad {
        let mut reader = CborReader::new(bytes, 4);
        assert_eq!(
            &reader.unsigned().unwrap_err(),
            expected,
            "bytes {bytes:02x?}"
        );
    }
}

#[test]
fn cbor_rejects_floats_tags_indefinite_and_reserved() {
    // Float (major 7).
    let float = [0xF9, 0x3C, 0x00]; // 1.0 half-precision
    let mut reader = CborReader::new(&float, 4);
    assert_eq!(
        reader.unsigned().unwrap_err(),
        CborError::UnsupportedMajorType(7)
    );
    // Tag (major 6).
    let tag = [0xC0]; // tag 0
    let mut reader = CborReader::new(&tag, 4);
    assert_eq!(
        reader.unsigned().unwrap_err(),
        CborError::UnsupportedMajorType(6)
    );
    // Indefinite array.
    let indefinite = [0x9F, 0x00, 0xFF];
    let mut reader = CborReader::new(&indefinite, 4);
    assert_eq!(reader.array().unwrap_err(), CborError::IndefiniteLength(4));
    // Reserved additional info 28.
    let reserved = [0x3C]; // major 1, additional info 28
    let mut reader = CborReader::new(&reserved, 4);
    assert_eq!(
        reader.integer().unwrap_err(),
        CborError::UnsupportedAdditionalInfo { major: 1, info: 28 }
    );
}

#[test]
fn cbor_rejects_truncation_trailing_and_bad_utf8() {
    let mut reader = CborReader::new(&[0x18], 4);
    assert_eq!(reader.unsigned().unwrap_err(), CborError::UnexpectedEof);

    let mut reader = CborReader::new(&[0x01, 0x02], 4);
    reader.unsigned().expect("first item");
    assert_eq!(reader.finish().unwrap_err(), CborError::TrailingBytes(1));

    let bad_text = [0x63, 0xFF, 0xFE, 0xFD]; // 3-byte text, invalid UTF-8
    let mut reader = CborReader::new(&bad_text, 4);
    assert_eq!(reader.text().unwrap_err(), CborError::InvalidUtf8);
}

#[test]
fn cbor_rejects_out_of_order_and_duplicate_map_keys() {
    // Keys 2 then 1.
    let mut writer = CborWriter::new();
    writer.map(2);
    writer.field(2).unsigned(1);
    writer.field(1).unsigned(2);
    let data = writer.into_bytes();
    let mut reader = CborReader::new(&data, 4);
    let mut record = reader.map().expect("map header");
    assert_eq!(record.next_key(), Ok(Some(2)));
    assert_eq!(record.unsigned(), Ok(1));
    assert_eq!(
        record.next_key().unwrap_err(),
        CborError::MapKeyOutOfOrder {
            previous: 2,
            current: 1
        }
    );
    drop(record);

    // Duplicate key.
    let mut writer = CborWriter::new();
    writer.map(2);
    writer.field(1).unsigned(1);
    writer.field(1).unsigned(2);
    let data = writer.into_bytes();
    let mut reader = CborReader::new(&data, 4);
    let mut record = reader.map().expect("map header");
    assert_eq!(record.next_key(), Ok(Some(1)));
    assert_eq!(record.unsigned(), Ok(1));
    assert_eq!(
        record.next_key().unwrap_err(),
        CborError::MapKeyOutOfOrder {
            previous: 1,
            current: 1
        }
    );
}

#[test]
fn cbor_rejects_absurd_declared_lengths() {
    // Array claiming 1000 items with 0 bytes remaining.
    let mut writer = CborWriter::new();
    writer.array(1000);
    let data = writer.into_bytes();
    let mut reader = CborReader::new(&data, 4);
    assert_eq!(
        reader.array().unwrap_err(),
        CborError::LengthExceedsInput {
            claimed: 1000,
            remaining: 0
        }
    );

    // Map claiming more entries than bytes can hold (2 bytes per entry).
    let mut writer = CborWriter::new();
    writer.map(3);
    writer.field(1).unsigned(1);
    let data = writer.into_bytes();
    let mut reader = CborReader::new(&data, 4);
    assert!(matches!(
        reader.map().unwrap_err(),
        CborError::LengthExceedsInput { .. }
    ));
}

fn descend(reader: &mut CborReader<'_>, remaining: usize) -> crate::cbor::Result<()> {
    let mut seq = reader.array()?;
    if remaining == 1 {
        assert_eq!(seq.unsigned(), Ok(0));
    } else {
        descend(&mut seq, remaining - 1)?;
    }
    Ok(())
}

#[test]
fn cbor_nesting_limit_is_enforced() {
    let mut data = Vec::new();
    let depth = 6;
    for _ in 0..depth {
        data.extend_from_slice(&[0x81]); // array(1)
    }
    data.push(0x00);
    // A budget below the actual nesting depth fails ...
    let mut reader = CborReader::new(&data, depth as u32 - 1);
    assert_eq!(
        descend(&mut reader, depth).unwrap_err(),
        CborError::NestingLimitExceeded(5)
    );
    // ... while a budget at or above it succeeds.
    let mut reader = CborReader::new(&data, depth as u32);
    descend(&mut reader, depth).expect("within budget");
}

#[test]
fn coordinate_grammar() {
    assert!(ConeCoordinate::new("dev.example", "app", "0.1.0").is_ok());
    assert!(ConeCoordinate::new("scoop", "scoop.core", "0.1.0").is_ok());
    assert!(ConeCoordinate::new("a", "b", "1.2.3-alpha.1+build.5").is_ok());
    assert!(ConeCoordinate::new("a", "b", "1.0.0-0.3.7").is_ok());

    // Group/name rejections.
    assert!(matches!(
        ConeCoordinate::new("Dev.example", "app", "1.0.0").unwrap_err(),
        CoordinateError::SegmentDoesNotStartWithLowercase { .. }
    ));
    assert!(matches!(
        ConeCoordinate::new("dev..example", "app", "1.0.0").unwrap_err(),
        CoordinateError::EmptySegment { .. }
    ));
    assert!(matches!(
        ConeCoordinate::new("dev.exa_mple", "app", "1.0.0").unwrap_err(),
        CoordinateError::InvalidCharacter { byte: b'_', .. }
    ));
    assert!(matches!(
        ConeCoordinate::new("", "app", "1.0.0").unwrap_err(),
        CoordinateError::EmptyComponent { .. }
    ));

    // Version rejections: leading zeros, v prefix, wildcard, ranges.
    for version in [
        "v1.0.0",
        "1.0.0.0",
        "01.0.0",
        "1.02.0",
        "1.0.00",
        "^1.0.0",
        "~0.1",
        "latest",
        "1.0.0-",
        "1.0.0+",
        "1.0.0-alpha..1",
        "1.0.0-01",
        "1.0.0+",
        "",
    ] {
        assert!(
            ConeCoordinate::new("a", "b", version).is_err(),
            "version {version} should be rejected"
        );
    }
    assert!(matches!(
        ConeCoordinate::new("a", "b", "1.0.0-alpha.01").unwrap_err(),
        CoordinateError::VersionDetail(_)
    ));
}

#[test]
fn coordinate_ordering_and_display() {
    let a = ConeCoordinate::new("dev.example", "app", "0.1.0").unwrap();
    let b = ConeCoordinate::new("dev.example", "app", "0.2.0").unwrap();
    let c = ConeCoordinate::new("dev.example", "zzz", "0.1.0").unwrap();
    let d = ConeCoordinate::new("org.foo", "app", "9.9.9").unwrap();
    assert!(a < b);
    assert!(a < c);
    assert!(a < d);
    assert_eq!(a.display(), "dev.example:app:0.1.0");

    // Pre-release sorts before release.
    let pre = ConeCoordinate::new("a", "b", "1.0.0-rc.1").unwrap();
    let rel = ConeCoordinate::new("a", "b", "1.0.0").unwrap();
    assert!(pre < rel);
    // Numeric identifiers are lower than alphanumeric ones.
    let numeric = ConeCoordinate::new("a", "b", "1.0.0-1").unwrap();
    let alpha = ConeCoordinate::new("a", "b", "1.0.0-alpha").unwrap();
    assert!(numeric < alpha);
}

#[test]
fn cone_identity_is_domain_separated_and_stable() {
    let a = ConeCoordinate::new("dev.example", "app", "0.1.0").unwrap();
    let same = ConeCoordinate::new("dev.example", "app", "0.1.0").unwrap();
    let bumped = ConeCoordinate::new("dev.example", "app", "0.1.1").unwrap();
    assert_eq!(ConeIdentity::of(&a), ConeIdentity::of(&same));
    assert_ne!(ConeIdentity::of(&a), ConeIdentity::of(&bumped));

    // The digest is the domain-hashed canonical CBOR of the coordinate.
    let expected = DomainHasher::new(b"scoop-cone-id-v1")
        .field(&a.canonical_cbor())
        .finish();
    assert_eq!(ConeIdentity::of(&a).as_bytes(), expected.as_bytes());

    // Canonical CBOR is a fixed-shape three-field record.
    let canonical = a.canonical_cbor();
    let mut reader = CborReader::new(&canonical, 4);
    let mut record = reader.map().expect("map");
    assert_eq!(record.next_key(), Ok(Some(1)));
    assert_eq!(record.text(), Ok("dev.example"));
    assert_eq!(record.next_key(), Ok(Some(2)));
    assert_eq!(record.text(), Ok("app"));
    assert_eq!(record.next_key(), Ok(Some(3)));
    assert_eq!(record.text(), Ok("0.1.0"));
    assert_eq!(record.next_key(), Ok(None));
    drop(record);
    reader.finish().expect("consumed");
}

#[test]
fn reserved_core_coordinate_is_scoop_core() {
    let core = ConeCoordinate::reserved_core();
    assert_eq!(core.display(), "scoop:scoop.core:0.1.0");
    assert!(core.is_reserved_core());
    assert!(
        !ConeCoordinate::new("scoop", "scoop.core", "0.2.0")
            .unwrap()
            .is_reserved_core()
    );
}

#[test]
fn version_keeps_exact_text() {
    let version = Version::parse("2.1.3-rc.42+build.7").unwrap();
    assert_eq!(version.as_str(), "2.1.3-rc.42+build.7");
    assert_eq!(
        (version.major(), version.minor(), version.patch()),
        (2, 1, 3)
    );
    // Build metadata with leading zeros is legal.
    assert!(Version::parse("1.0.0+007").is_ok());
}

#[test]
fn capability_grammar_and_builtins() {
    assert!(CapabilityId::new("org.scoop-lang.link-object", "scoop-lir", 1).is_ok());
    assert!(CapabilityId::new("a", "b", u32::MAX).is_ok());
    assert_eq!(
        CapabilityId::new("a", "b", 0).unwrap_err(),
        CapabilityError::ZeroMajorVersion
    );
    assert!(matches!(
        CapabilityId::new("A.b", "c", 1).unwrap_err(),
        CapabilityError::InvalidNamespaceByte(b'A')
    ));
    assert_eq!(
        CapabilityId::new("a", "b_c", 1).unwrap_err(),
        CapabilityError::InvalidNameByte(b'_')
    );
    assert_eq!(
        CapabilityId::new("a..b", "c", 1).unwrap_err(),
        CapabilityError::EmptySegment
    );
    let long_name = "a".repeat(64);
    assert_eq!(
        CapabilityId::new("a", &long_name, 1).unwrap_err(),
        CapabilityError::NameTooLong(64)
    );
    let long_segment = format!("a{}.c", "b".repeat(63));
    assert!(matches!(
        CapabilityId::new(&long_segment, "c", 1).unwrap_err(),
        CapabilityError::NamespaceTooLong(_)
    ));

    // Built-ins use the registry coordinates from DESIGN 4.1.
    assert_eq!(
        TargetProfileWireId::darwin_aarch64_v1()
            .as_capability()
            .namespace(),
        "org.scoop-lang.target-profile"
    );
    assert_eq!(
        ObjectFormatId::mach_o_relocatable_v1()
            .as_capability()
            .name(),
        "mach-o-relocatable"
    );
    assert_eq!(
        scoop_lir_link_object_v1(),
        CapabilityId::new("org.scoop-lang.link-object", "scoop-lir", 1).unwrap()
    );
    assert_eq!(
        generated_c_bridge_link_object_v1(),
        CapabilityId::new("org.scoop-lang.link-object", "generated-c-bridge", 1).unwrap()
    );

    // Wire form is the three-field map.
    let cbor = scoop_lir_link_object_v1().canonical_cbor();
    let mut reader = CborReader::new(&cbor, 4);
    let mut record = reader.map().expect("map");
    assert_eq!(record.next_key(), Ok(Some(1)));
    assert_eq!(record.text(), Ok("org.scoop-lang.link-object"));
    assert_eq!(record.next_key(), Ok(Some(2)));
    assert_eq!(record.text(), Ok("scoop-lir"));
    assert_eq!(record.next_key(), Ok(Some(3)));
    assert_eq!(record.unsigned(), Ok(1));
    assert_eq!(record.next_key(), Ok(None));
}

#[test]
fn digest_ordering_is_byte_order() {
    let mut high_bytes = [0x00; 32];
    high_bytes[0] = 0x01;
    let low = Digest256::from_bytes([0x00; 32]);
    let high = Digest256::from_bytes(high_bytes);
    assert!(low < high);
}
