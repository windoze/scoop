//! Stable identity primitives shared across compiler stages and facts.
//!
//! This base crate owns cross-stage identifiers such as site IDs, stable
//! hash/key primitives, canonical text wrappers, and body-version keys. It must
//! not depend on `scoopc`, stage crates, fact crates, backend crates, or
//! repository tools.

#![forbid(unsafe_code)]

use std::fmt;
use std::path::PathBuf;

use scoopc_span::Span;
use scoopc_types::{EffectRow, TypeId};
use sha2::{Digest, Sha256};

/// Versioned hash scopes shared by ABI, private symbols, RTTI, and dumps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StableHashScope {
    AbiV0,
    PrivateV0,
    RttiV0,
    DumpV0,
}

impl StableHashScope {
    /// Returns the fixed textual prefix mandated by `STABLE_ID.md`.
    pub const fn prefix(self) -> &'static str {
        match self {
            Self::AbiV0 => "abi0:",
            Self::PrivateV0 => "priv0:",
            Self::RttiV0 => "rtti0:",
            Self::DumpV0 => "dump0:",
        }
    }
}

/// Common trait implemented by every stable identity key.
pub trait StableCanonicalKey {
    fn canonical_text(&self) -> String;
}

/// Ad-hoc stable key wrapper for call sites that already computed canonical text.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CanonicalTextKey(String);

impl CanonicalTextKey {
    pub fn new(text: impl Into<String>) -> Self {
        Self(text.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl StableCanonicalKey for CanonicalTextKey {
    fn canonical_text(&self) -> String {
        self.0.clone()
    }
}

/// Stable keys that can also contribute a human-readable symbol prefix.
pub trait StableSymbolKey: StableCanonicalKey {
    fn readable_path(&self) -> &str;
}

/// A stable effect/call site identity scoped to a single body.
///
/// `SiteId` is intentionally stage-independent so facts can key site-level data
/// without depending on MIR-owned node definitions.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SiteId(u32);

impl SiteId {
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

impl fmt::Debug for SiteId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "site{}", self.0)
    }
}

/// Stable block identity scoped to a single body.
///
/// This intentionally lives outside MIR node definitions so fact products can
/// publish block-level summaries without depending on MIR internals.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BodyBlockId(u32);

impl BodyBlockId {
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

impl fmt::Debug for BodyBlockId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "bb{}", self.0)
    }
}

/// Stage-independent identity for a generic callable template.
///
/// The current compiler still uses source path and declaration span to locate
/// bodies during materialization, while exported identities use stable keys.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct TemplateKey {
    pub fqn: String,
    pub source_path: PathBuf,
    pub decl_span: Span,
}

impl fmt::Debug for TemplateKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}@{}:{:?}",
            self.fqn,
            self.source_path.display(),
            self.decl_span
        )
    }
}

/// Stage-independent identity for one monomorphic callable instance.
///
/// This key intentionally lives in the base identity crate so HIR compatibility
/// scaffolding and MIR materialization do not point at each other.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct InstanceKey {
    pub template: TemplateKey,
    pub type_args: Vec<TypeId>,
    pub eff_args: Vec<EffectRow>,
}

impl fmt::Debug for InstanceKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InstanceKey")
            .field("template", &self.template)
            .field("type_args", &TypeIdList(&self.type_args))
            .field("eff_args", &EffectRowList(&self.eff_args))
            .finish()
    }
}

struct TypeIdList<'a>(&'a [TypeId]);

impl fmt::Debug for TypeIdList<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entries(self.0.iter().copied().map(TypeIdRepr))
            .finish()
    }
}

struct TypeIdRepr(TypeId);

impl fmt::Debug for TypeIdRepr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "t{}", self.0.as_u32())
    }
}

struct EffectRowList<'a>(&'a [EffectRow]);

impl fmt::Debug for EffectRowList<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entries(self.0.iter().map(EffectRowRepr))
            .finish()
    }
}

struct EffectRowRepr<'a>(&'a EffectRow);

impl fmt::Debug for EffectRowRepr<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_pure() {
            return write!(f, "Pure");
        }
        f.debug_list()
            .entries(self.0.terms.iter().copied().map(TypeIdRepr))
            .finish()
    }
}

/// Stage-independent stable identity for an effect-facts callable instance.
///
/// The current monolithic compiler may still derive this from a MIR
/// `InstanceKey`, but the fact product only stores canonical text and a readable
/// path so it remains independent of MIR templates and bodies.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StableEffectInstanceKey {
    canonical_text: String,
    readable_path: String,
}

impl StableEffectInstanceKey {
    pub fn new(canonical_text: impl Into<String>, readable_path: impl Into<String>) -> Self {
        Self {
            canonical_text: canonical_text.into(),
            readable_path: readable_path.into(),
        }
    }

    pub fn from_symbol_key<K>(key: &K) -> Self
    where
        K: StableSymbolKey + ?Sized,
    {
        Self::new(key.canonical_text(), key.readable_path())
    }

    pub fn as_str(&self) -> &str {
        &self.canonical_text
    }

    pub fn readable_path(&self) -> &str {
        &self.readable_path
    }
}

impl StableCanonicalKey for StableEffectInstanceKey {
    fn canonical_text(&self) -> String {
        self.canonical_text.clone()
    }
}

impl StableSymbolKey for StableEffectInstanceKey {
    fn readable_path(&self) -> &str {
        &self.readable_path
    }
}

/// Stage-independent stable identity for a callable published by LIR facts.
///
/// The current monolithic compiler may still derive this from a stage-owned
/// semantic instance key, but the fact product only stores canonical text and a
/// readable path so it remains independent of MIR/LIR implementation types.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StableLirCallableKey {
    canonical_text: String,
    readable_path: String,
}

impl StableLirCallableKey {
    pub fn new(canonical_text: impl Into<String>, readable_path: impl Into<String>) -> Self {
        Self {
            canonical_text: canonical_text.into(),
            readable_path: readable_path.into(),
        }
    }

    pub fn from_symbol_key<K>(key: &K) -> Self
    where
        K: StableSymbolKey + ?Sized,
    {
        Self::new(key.canonical_text(), key.readable_path())
    }

    pub fn as_str(&self) -> &str {
        &self.canonical_text
    }

    pub fn readable_path(&self) -> &str {
        &self.readable_path
    }
}

impl StableCanonicalKey for StableLirCallableKey {
    fn canonical_text(&self) -> String {
        self.canonical_text.clone()
    }
}

impl StableSymbolKey for StableLirCallableKey {
    fn readable_path(&self) -> &str {
        &self.readable_path
    }
}

/// Reserved stable identity for future body-versioned facts.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BodyVersionKey {
    owner_canonical_text: String,
    role: String,
    version: u32,
}

impl BodyVersionKey {
    pub fn new<K>(owner: &K, role: impl Into<String>, version: u32) -> Self
    where
        K: StableCanonicalKey + ?Sized,
    {
        Self::from_parts(owner.canonical_text(), role, version)
    }

    pub fn from_parts(
        owner_canonical_text: impl Into<String>,
        role: impl Into<String>,
        version: u32,
    ) -> Self {
        Self {
            owner_canonical_text: owner_canonical_text.into(),
            role: role.into(),
            version,
        }
    }

    pub fn owner_canonical_text(&self) -> &str {
        &self.owner_canonical_text
    }

    pub fn role(&self) -> &str {
        &self.role
    }

    pub fn version(&self) -> u32 {
        self.version
    }
}

impl StableCanonicalKey for BodyVersionKey {
    fn canonical_text(&self) -> String {
        canonical_record(
            "body_version",
            [
                self.owner_canonical_text.clone(),
                self.role.clone(),
                self.version.to_string(),
            ],
        )
    }
}

/// Stable identity for a versioned artifact published by a compiler stage.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StageArtifactKey {
    stage: String,
    owner_canonical_text: String,
    role: String,
    revision: u32,
}

impl StageArtifactKey {
    /// Create a stage artifact key from any stable owner key.
    pub fn new<K>(
        stage: impl Into<String>,
        owner: &K,
        role: impl Into<String>,
        revision: u32,
    ) -> Self
    where
        K: StableCanonicalKey + ?Sized,
    {
        Self::from_parts(stage, owner.canonical_text(), role, revision)
    }

    /// Create a stage artifact key from precomputed canonical owner text.
    pub fn from_parts(
        stage: impl Into<String>,
        owner_canonical_text: impl Into<String>,
        role: impl Into<String>,
        revision: u32,
    ) -> Self {
        Self {
            stage: stage.into(),
            owner_canonical_text: owner_canonical_text.into(),
            role: role.into(),
            revision,
        }
    }

    /// Return the publishing stage label.
    pub fn stage(&self) -> &str {
        &self.stage
    }

    /// Return the canonical owner text this artifact is scoped under.
    pub fn owner_canonical_text(&self) -> &str {
        &self.owner_canonical_text
    }

    /// Return the artifact role within the publishing stage.
    pub fn role(&self) -> &str {
        &self.role
    }

    /// Return the monotonically assigned revision within the role.
    pub fn revision(&self) -> u32 {
        self.revision
    }
}

impl StableCanonicalKey for StageArtifactKey {
    fn canonical_text(&self) -> String {
        canonical_record(
            "stage_artifact",
            [
                self.stage.clone(),
                self.owner_canonical_text.clone(),
                self.role.clone(),
                self.revision.to_string(),
            ],
        )
    }
}

/// Stable local call-site identity for dump labels and private helpers.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StableCallSiteKey {
    owner_canonical_text: String,
    source_path: String,
    span: Span,
    site_kind: String,
}

impl StableCallSiteKey {
    pub fn new<K>(
        owner: &K,
        source_path: impl Into<String>,
        span: Span,
        site_kind: impl Into<String>,
    ) -> Self
    where
        K: StableCanonicalKey + ?Sized,
    {
        Self {
            owner_canonical_text: owner.canonical_text(),
            source_path: source_path.into(),
            span,
            site_kind: site_kind.into(),
        }
    }
}

impl StableCanonicalKey for StableCallSiteKey {
    fn canonical_text(&self) -> String {
        canonical_record(
            "call_site",
            [
                self.owner_canonical_text.clone(),
                self.source_path.clone(),
                self.span.start.to_string(),
                self.span.end.to_string(),
                self.site_kind.clone(),
            ],
        )
    }
}

/// ABI-visible symbol namespaces defined by the shared mangler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AbiSymbolKind {
    Fun,
    Global,
    Type,
}

impl AbiSymbolKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Fun => "fun",
            Self::Global => "global",
            Self::Type => "type",
        }
    }
}

/// Shared exported-symbol mangler.
#[derive(Debug, Default, Clone, Copy)]
pub struct AbiMangler;

impl AbiMangler {
    pub fn mangle<K>(self, kind: AbiSymbolKind, key: &K) -> String
    where
        K: StableSymbolKey + ?Sized,
    {
        let canonical = key.canonical_text();
        let readable = sanitize_symbol_component(key.readable_path());
        format!(
            "__scoop_abi0_{}__{}__h{}",
            kind.as_str(),
            readable,
            stable_hash128_hex(StableHashScope::AbiV0, &canonical)
        )
    }

    pub fn fun_symbol<K>(self, key: &K) -> String
    where
        K: StableSymbolKey + ?Sized,
    {
        self.mangle(AbiSymbolKind::Fun, key)
    }

    pub fn global_symbol<K>(self, key: &K) -> String
    where
        K: StableSymbolKey + ?Sized,
    {
        self.mangle(AbiSymbolKind::Global, key)
    }

    pub fn type_symbol<K>(self, key: &K) -> String
    where
        K: StableSymbolKey + ?Sized,
    {
        self.mangle(AbiSymbolKind::Type, key)
    }
}

/// Shared compiler-private symbol mangler.
#[derive(Debug, Default, Clone, Copy)]
pub struct PrivateSymbolMangler;

impl PrivateSymbolMangler {
    fn canonical_private_text<K>(role: &str, key: &K) -> (String, String)
    where
        K: StableCanonicalKey + ?Sized,
    {
        let role = sanitize_symbol_component(role);
        let canonical = canonical_record("private", [role.clone(), key.canonical_text()]);
        (role, canonical)
    }

    pub fn mangle<K>(self, role: &str, key: &K) -> String
    where
        K: StableCanonicalKey + ?Sized,
    {
        let (role, canonical) = Self::canonical_private_text(role, key);
        format!(
            "__scoop_priv0__{}__h{}",
            role,
            stable_hash128_hex(StableHashScope::PrivateV0, &canonical)
        )
    }

    pub fn hash_suffix<K>(self, role: &str, key: &K) -> String
    where
        K: StableCanonicalKey + ?Sized,
    {
        let (_, canonical) = Self::canonical_private_text(role, key);
        stable_hash128_hex(StableHashScope::PrivateV0, &canonical)
    }

    pub fn type_name<K>(self, family: &str, role: &str, key: &K) -> String
    where
        K: StableCanonicalKey + ?Sized,
    {
        let family = sanitize_symbol_component(family);
        let hash = self.hash_suffix(role, key);
        format!("scoop.lowered.{family}__h{hash}")
    }
}

/// Hashes canonical text with a fixed version prefix using SHA-256.
pub fn stable_digest(scope: StableHashScope, canonical_text: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(scope.prefix().as_bytes());
    hasher.update(canonical_text.as_bytes());
    hasher.finalize().into()
}

/// Returns the linker-visible 128-bit truncated hash as lowercase hex.
pub fn stable_hash128_hex(scope: StableHashScope, canonical_text: &str) -> String {
    let digest = stable_digest(scope, canonical_text);
    hex_lower(&digest[..16])
}

/// Returns the runtime-only 64-bit truncated hash.
pub fn stable_hash64(scope: StableHashScope, canonical_text: &str) -> u64 {
    let digest = stable_digest(scope, canonical_text);
    let bytes: [u8; 8] = digest[..8]
        .try_into()
        .expect("sha256 output is always 32 bytes");
    u64::from_le_bytes(bytes)
}

/// Shared RTTI type-id helper for descriptor names and runtime-match type names.
pub fn stable_rtti_type_id(canonical_name: &str) -> u64 {
    stable_hash64(StableHashScope::RttiV0, canonical_name)
}

/// Builds a short dump label from a semantic role plus canonical text.
pub fn stable_dump_label(role: &str, canonical_text: &str) -> String {
    format!(
        "{role}#h{}",
        stable_hash128_hex(StableHashScope::DumpV0, canonical_text)
    )
}

/// Builds a short stable local label directly from a stable key.
pub fn stable_local_label<K>(role: &str, key: &K) -> String
where
    K: StableCanonicalKey + ?Sized,
{
    stable_dump_label(role, &key.canonical_text())
}

pub fn canonical_record<I>(tag: &str, parts: I) -> String
where
    I: IntoIterator<Item = String>,
{
    let mut out = String::new();
    out.push_str(tag);
    out.push('(');
    let mut first = true;
    for part in parts {
        if !first {
            out.push(';');
        }
        first = false;
        out.push_str(&part.len().to_string());
        out.push(':');
        out.push_str(&part);
    }
    out.push(')');
    out
}

pub fn canonical_list(parts: &[String]) -> String {
    canonical_record("list", parts.iter().cloned())
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn sanitize_symbol_component(text: &str) -> String {
    let mut out = String::with_capacity(text.len().max(4));
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        out.push_str("anon");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn site_id_debug_and_raw_round_trip_are_stable() {
        let site = SiteId::from_raw(42);

        assert_eq!(site.as_u32(), 42);
        assert_eq!(format!("{site:?}"), "site42");
    }

    #[test]
    fn hash_scopes_do_not_collide() {
        let canonical = "N(pkg.Token)";

        assert_ne!(
            stable_hash128_hex(StableHashScope::AbiV0, canonical),
            stable_hash128_hex(StableHashScope::PrivateV0, canonical)
        );
        assert_ne!(
            stable_hash64(StableHashScope::RttiV0, canonical),
            stable_hash64(StableHashScope::DumpV0, canonical)
        );
    }

    #[test]
    fn body_version_key_has_stable_canonical_text() {
        let owner = CanonicalTextKey::new("def(pkg.main)");
        let key = BodyVersionKey::new(&owner, "late_lowered", 3);

        assert_eq!(key.owner_canonical_text(), "def(pkg.main)");
        assert_eq!(key.role(), "late_lowered");
        assert_eq!(key.version(), 3);
        assert_eq!(
            key.canonical_text(),
            canonical_record(
                "body_version",
                [
                    "def(pkg.main)".to_string(),
                    "late_lowered".to_string(),
                    "3".to_string(),
                ],
            )
        );
    }

    #[test]
    fn stage_artifact_key_has_stable_canonical_text() {
        let owner = CanonicalTextKey::new("cone(pkg.app)");
        let key = StageArtifactKey::new("mir", &owner, "snapshot", 2);

        assert_eq!(key.stage(), "mir");
        assert_eq!(key.owner_canonical_text(), "cone(pkg.app)");
        assert_eq!(key.role(), "snapshot");
        assert_eq!(key.revision(), 2);
        assert_eq!(
            key.canonical_text(),
            canonical_record(
                "stage_artifact",
                [
                    "mir".to_string(),
                    "cone(pkg.app)".to_string(),
                    "snapshot".to_string(),
                    "2".to_string(),
                ],
            )
        );
    }
}
