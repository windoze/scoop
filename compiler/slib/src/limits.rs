//! Centralized `.slib` decode resource limits (DESIGN section 4.5).
//!
//! Every reader in this crate consumes these constants; no decoder keeps
//! a private budget. Values are v1 defaults and shared by `scoop`,
//! direct `scoopc` and program-link.

/// Maximum total archive length: 2 GiB.
pub const ARCHIVE_MAX_BYTES: u64 = 2 * 1024 * 1024 * 1024;
/// Maximum bootstrap `manifest.cbor` size: 64 MiB.
pub const MANIFEST_MAX_BYTES: u64 = 64 * 1024 * 1024;
/// Maximum number of non-manifest members: 65,536.
pub const MEMBERS_MAX_COUNT: u64 = 65_536;
/// Maximum single metadata section: 256 MiB.
pub const METADATA_SECTION_MAX_BYTES: u64 = 256 * 1024 * 1024;
/// Maximum single link object or extension blob: 1 GiB.
pub const LINK_MEMBER_MAX_BYTES: u64 = 1024 * 1024 * 1024;
/// Maximum single diagnostic attachment: 512 MiB.
pub const DIAGNOSTIC_MEMBER_MAX_BYTES: u64 = 512 * 1024 * 1024;
/// Maximum CBOR structural nesting: 128.
pub const CBOR_NESTING_LIMIT: u32 = 128;
/// Maximum entries in any single table: 16,777,216.
pub const TABLE_MAX_ENTRIES: u64 = 16_777_216;
/// Maximum single text/bytes semantic field: 16 MiB.
pub const SEMANTIC_FIELD_MAX_BYTES: u64 = 16 * 1024 * 1024;
/// Maximum recursive type/body validation depth: 1,024.
pub const RECURSIVE_VALIDATION_DEPTH: u32 = 1_024;

/// Per-artifact decode budget passed through every reader.
#[derive(Debug, Clone, Copy)]
pub struct SlibDecodeLimits {
    pub archive_max_bytes: u64,
    pub manifest_max_bytes: u64,
    pub members_max_count: u64,
    pub metadata_section_max_bytes: u64,
    pub link_member_max_bytes: u64,
    pub diagnostic_member_max_bytes: u64,
    pub cbor_nesting_limit: u32,
    pub table_max_entries: u64,
    pub semantic_field_max_bytes: u64,
}

impl Default for SlibDecodeLimits {
    fn default() -> Self {
        SlibDecodeLimits {
            archive_max_bytes: ARCHIVE_MAX_BYTES,
            manifest_max_bytes: MANIFEST_MAX_BYTES,
            members_max_count: MEMBERS_MAX_COUNT,
            metadata_section_max_bytes: METADATA_SECTION_MAX_BYTES,
            link_member_max_bytes: LINK_MEMBER_MAX_BYTES,
            diagnostic_member_max_bytes: DIAGNOSTIC_MEMBER_MAX_BYTES,
            cbor_nesting_limit: CBOR_NESTING_LIMIT,
            table_max_entries: TABLE_MAX_ENTRIES,
            semantic_field_max_bytes: SEMANTIC_FIELD_MAX_BYTES,
        }
    }
}

impl SlibDecodeLimits {
    pub fn strict() -> Self {
        SlibDecodeLimits {
            archive_max_bytes: 8 * 1024 * 1024,
            manifest_max_bytes: 1024 * 1024,
            members_max_count: 64,
            metadata_section_max_bytes: 1024 * 1024,
            link_member_max_bytes: 1024 * 1024,
            diagnostic_member_max_bytes: 1024 * 1024,
            cbor_nesting_limit: 16,
            table_max_entries: 1_024,
            semantic_field_max_bytes: 64 * 1024,
        }
    }
}
