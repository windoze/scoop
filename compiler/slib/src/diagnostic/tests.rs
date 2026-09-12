use scoop_identity::{ConeIdentity, SemanticIdentityImportError};
use scoop_wire::{ResourceKind, WireError, WireErrorKind, WirePath};

use crate::{ArchiveReadError, MemberStableKey, SlibMember, SlibMemberRole, SlibReadError};

use super::{
    SlibDiagnostic, SlibErrorCode, SlibPrimaryOrigin, SlibResourceFailure, SlibResourceKind,
};

#[test]
fn stable_error_code_spellings_are_closed_and_golden() {
    use SlibErrorCode::*;

    let expected = [
        (ContainerBadMagic, "SLIB_CONTAINER_BAD_MAGIC"),
        (ContainerTruncated, "SLIB_CONTAINER_TRUNCATED"),
        (ContainerNonCanonical, "SLIB_CONTAINER_NON_CANONICAL"),
        (ContainerInvalidLength, "SLIB_CONTAINER_INVALID_LENGTH"),
        (WireUnexpectedEnd, "SLIB_WIRE_UNEXPECTED_END"),
        (WireWrongType, "SLIB_WIRE_WRONG_TYPE"),
        (WireIndefiniteLength, "SLIB_WIRE_INDEFINITE_LENGTH"),
        (WireNonCanonical, "SLIB_WIRE_NON_CANONICAL"),
        (WireTrailingData, "SLIB_WIRE_TRAILING_DATA"),
        (WireInvalidField, "SLIB_WIRE_INVALID_FIELD"),
        (WireUnknownTag, "SLIB_WIRE_UNKNOWN_TAG"),
        (WireInvalidLength, "SLIB_WIRE_INVALID_LENGTH"),
        (WireIntegerOutOfRange, "SLIB_WIRE_INTEGER_OUT_OF_RANGE"),
        (LimitExceeded, "SLIB_LIMIT_EXCEEDED"),
        (LimitAllocation, "SLIB_LIMIT_ALLOCATION"),
        (DirectoryMismatch, "SLIB_DIRECTORY_MISMATCH"),
        (
            DirectoryNonCanonicalOrder,
            "SLIB_DIRECTORY_NON_CANONICAL_ORDER",
        ),
        (CapabilityInvalid, "SLIB_CAPABILITY_INVALID"),
        (CapabilityUnsupported, "SLIB_CAPABILITY_UNSUPPORTED"),
        (
            CapabilityNativeBoundaryClosureRequired,
            "SLIB_CAPABILITY_NATIVE_BOUNDARY_CLOSURE_REQUIRED",
        ),
        (CompatibilitySchema, "SLIB_COMPAT_SCHEMA"),
        (CompatibilityMangling, "SLIB_COMPAT_MANGLING"),
        (CompatibilityProfile, "SLIB_COMPAT_PROFILE"),
        (IdentityMismatch, "SLIB_IDENTITY_MISMATCH"),
        (IdentityDuplicate, "SLIB_IDENTITY_DUPLICATE"),
        (IdentityMissing, "SLIB_IDENTITY_MISSING"),
        (IdentityCycle, "SLIB_IDENTITY_CYCLE"),
        (IdentityInvalid, "SLIB_IDENTITY_INVALID"),
        (ReferenceMissing, "SLIB_REFERENCE_MISSING"),
        (ReferenceFutureLayer, "SLIB_REFERENCE_FUTURE_LAYER"),
        (ReferenceInvalid, "SLIB_REFERENCE_INVALID"),
        (BridgeMismatch, "SLIB_BRIDGE_MISMATCH"),
        (FingerprintMismatch, "SLIB_FINGERPRINT_MISMATCH"),
        (SessionConflict, "SLIB_SESSION_CONFLICT"),
    ];

    for (code, spelling) in expected {
        assert_eq!(code.as_str(), spelling);
        assert_eq!(code.to_string(), spelling);
    }
}

#[test]
fn logical_limit_diagnostic_retains_path_offset_and_observation() {
    let path = WirePath::root().field(7).index(3);
    let error = WireError::new(
        WireErrorKind::LimitExceeded {
            resource: ResourceKind::OwnedBytes,
            limit: 10,
            observed: 11,
        },
        path.clone(),
        Some(42),
    );

    let diagnostic = error.diagnostic();
    assert_eq!(diagnostic.code(), SlibErrorCode::LimitExceeded);
    assert_eq!(diagnostic.path(), &path);
    assert_eq!(diagnostic.byte_offset(), Some(42));
    assert_eq!(diagnostic.primary_origin(), None);
    assert_eq!(
        diagnostic.resource(),
        Some(SlibResourceFailure::Limit {
            resource: SlibResourceKind::Decode(ResourceKind::OwnedBytes),
            limit: 10,
            observed: 11,
        })
    );
}

#[test]
fn nested_reader_error_adds_manifest_origin_without_losing_wire_path() {
    let path = WirePath::root().field(5).field(4);
    let error = SlibReadError::CanonicalWire(WireError::new(
        WireErrorKind::NonCanonicalCbor,
        path.clone(),
        Some(9),
    ));

    let diagnostic = error.diagnostic();
    assert_eq!(diagnostic.code(), SlibErrorCode::WireNonCanonical);
    assert_eq!(diagnostic.path(), &path);
    assert_eq!(diagnostic.byte_offset(), Some(9));
    assert_eq!(
        diagnostic.primary_origin(),
        Some(&SlibPrimaryOrigin::Manifest)
    );
}

#[test]
fn member_digest_error_uses_typed_member_origin_and_key_path() {
    let member = SlibMember::new(
        ConeIdentity::CORE,
        MemberStableKey::HirMetadata,
        SlibMemberRole::HirMetadata,
        b"payload".to_vec(),
    )
    .unwrap();
    let id = member.record().id();
    let diagnostic = ArchiveReadError::MemberDigestMismatch { id }.diagnostic();

    assert_eq!(diagnostic.code(), SlibErrorCode::FingerprintMismatch);
    assert_eq!(
        diagnostic.path(),
        &WirePath::root().field(8).key("slib-member", *id.as_array())
    );
    assert_eq!(
        diagnostic.primary_origin(),
        Some(&SlibPrimaryOrigin::Member(id))
    );
}

#[test]
fn semantic_import_conflict_has_the_single_session_code() {
    let diagnostic = SemanticIdentityImportError::OriginConflict {
        origin: ConeIdentity::CORE,
    }
    .diagnostic();

    assert_eq!(diagnostic.code(), SlibErrorCode::SessionConflict);
    assert_eq!(diagnostic.path(), &WirePath::root());
    assert_eq!(
        diagnostic.primary_origin(),
        Some(&SlibPrimaryOrigin::Cone(ConeIdentity::CORE))
    );
}
