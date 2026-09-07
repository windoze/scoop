//! Protocol message types and canonical-CBOR framing.

use core::fmt;
use std::io::{Read, Write};

use scoop_identity::{
    CborError, CborReader, CborWriter, ConeCoordinate, CoordinateError, Digest256,
};

use crate::{MAX_FRAME_BYTES, PROTOCOL_VERSION};

/// Protocol-level failure: IO error, malformed frame, or schema mismatch.
#[derive(Debug)]
pub enum ProtocolError {
    Io(std::io::Error),
    FrameTooLarge(u32),
    Malformed(CborError),
    UnknownMessageTag(u64),
    UnknownField(u64),
    MissingField(u64),
    WrongEntryCount { expected: u64, actual: u64 },
    NonUtf8Path,
    InvalidCoordinate(CoordinateError),
    ProtocolVersionMismatch { expected: u32, actual: u32 },
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProtocolError::Io(error) => write!(f, "protocol IO error: {error}"),
            ProtocolError::FrameTooLarge(length) => {
                write!(f, "protocol frame of {length} bytes exceeds the limit")
            }
            ProtocolError::Malformed(error) => write!(f, "malformed protocol message: {error}"),
            ProtocolError::UnknownMessageTag(tag) => write!(f, "unknown message tag {tag}"),
            ProtocolError::UnknownField(key) => write!(f, "unknown message field key {key}"),
            ProtocolError::MissingField(key) => write!(f, "missing message field key {key}"),
            ProtocolError::WrongEntryCount { expected, actual } => write!(
                f,
                "message record declares {actual} entries, schema requires {expected}"
            ),
            ProtocolError::NonUtf8Path => write!(f, "paths on the protocol must be UTF-8"),
            ProtocolError::InvalidCoordinate(error) => {
                write!(f, "invalid coordinate in protocol message: {error}")
            }
            ProtocolError::ProtocolVersionMismatch { expected, actual } => write!(
                f,
                "subprocess protocol version {actual} does not match the expected {expected}"
            ),
        }
    }
}

impl std::error::Error for ProtocolError {}

impl From<std::io::Error> for ProtocolError {
    fn from(error: std::io::Error) -> Self {
        ProtocolError::Io(error)
    }
}

impl From<CborError> for ProtocolError {
    fn from(error: CborError) -> Self {
        ProtocolError::Malformed(error)
    }
}

impl From<CoordinateError> for ProtocolError {
    fn from(error: CoordinateError) -> Self {
        ProtocolError::InvalidCoordinate(error)
    }
}

/// Toolchain identity advertised by `scoopc` and checked by `scoop`.
/// Patch-level compiler differences are diagnostics only; every schema
/// and ABI component must match exactly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilerIdentity {
    /// The subprocess protocol itself.
    pub protocol_version: u32,
    /// Producer version string; displayed but never used for matching.
    pub compiler_version: String,
    /// Container version of the `.slib` archive format.
    pub slib_container_version: u32,
    /// Wire schema versions carried in the artifact manifest.
    pub hir_wire_schema: u32,
    pub mir_wire_schema: u32,
    pub lir_wire_schema: u32,
    /// Identity/mangling schema used for persistent ids and symbols.
    pub identity_schema_version: u32,
}

impl CompilerIdentity {
    /// The identity this toolchain builds with. Both sides construct it
    /// from the same compile-time constants; a subprocess built from a
    /// different toolchain disagrees on at least one field.
    pub fn current() -> Self {
        CompilerIdentity {
            protocol_version: PROTOCOL_VERSION,
            compiler_version: env!("CARGO_PKG_VERSION").to_owned(),
            slib_container_version: 1,
            hir_wire_schema: 1,
            mir_wire_schema: 1,
            lir_wire_schema: 1,
            identity_schema_version: 1,
        }
    }

    /// Exact-match compatibility: no field-level downgrade tolerance.
    pub fn compatible(&self, other: &CompilerIdentity) -> bool {
        self.protocol_version == other.protocol_version
            && self.slib_container_version == other.slib_container_version
            && self.hir_wire_schema == other.hir_wire_schema
            && self.mir_wire_schema == other.mir_wire_schema
            && self.lir_wire_schema == other.lir_wire_schema
            && self.identity_schema_version == other.identity_schema_version
    }

    fn encode(&self, writer: &mut CborWriter) {
        writer.map(7);
        writer.field(1).unsigned(self.protocol_version as u64);
        writer.field(2).text(&self.compiler_version);
        writer.field(3).unsigned(self.slib_container_version as u64);
        writer.field(4).unsigned(self.hir_wire_schema as u64);
        writer.field(5).unsigned(self.mir_wire_schema as u64);
        writer.field(6).unsigned(self.lir_wire_schema as u64);
        writer
            .field(7)
            .unsigned(self.identity_schema_version as u64);
    }

    fn decode(reader: &mut CborReader<'_>) -> Result<Self, ProtocolError> {
        let mut record = reader.map()?;
        expect_entries(&mut record, 7)?;
        // The record shape is fixed: keys 1..=7 in declaration order.
        expect_key(&mut record, 1)?;
        let protocol_version = read_u32(record.unsigned()?)?;
        expect_key(&mut record, 2)?;
        let compiler_version = record.text()?.to_owned();
        expect_key(&mut record, 3)?;
        let slib_container_version = read_u32(record.unsigned()?)?;
        expect_key(&mut record, 4)?;
        let hir_wire_schema = read_u32(record.unsigned()?)?;
        expect_key(&mut record, 5)?;
        let mir_wire_schema = read_u32(record.unsigned()?)?;
        expect_key(&mut record, 6)?;
        let lir_wire_schema = read_u32(record.unsigned()?)?;
        expect_key(&mut record, 7)?;
        let identity_schema_version = read_u32(record.unsigned()?)?;
        Ok(CompilerIdentity {
            protocol_version,
            compiler_version,
            slib_container_version,
            hir_wire_schema,
            mir_wire_schema,
            lir_wire_schema,
            identity_schema_version,
        })
    }
}

fn read_u32(value: u64) -> Result<u32, ProtocolError> {
    u32::try_from(value)
        .map_err(|_| ProtocolError::Malformed(CborError::NonCanonicalInteger { value, info: 27 }))
}

/// Where a diagnostic originated, for stable ordering and attribution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticPhase {
    Manifest,
    InputClosure,
    Source,
    SlibReader,
    Codegen,
    Link,
}

impl DiagnosticPhase {
    fn tag(self) -> u64 {
        match self {
            DiagnosticPhase::Manifest => 1,
            DiagnosticPhase::InputClosure => 2,
            DiagnosticPhase::Source => 3,
            DiagnosticPhase::SlibReader => 4,
            DiagnosticPhase::Codegen => 5,
            DiagnosticPhase::Link => 6,
        }
    }

    fn from_tag(tag: u64) -> Result<Self, ProtocolError> {
        match tag {
            1 => Ok(DiagnosticPhase::Manifest),
            2 => Ok(DiagnosticPhase::InputClosure),
            3 => Ok(DiagnosticPhase::Source),
            4 => Ok(DiagnosticPhase::SlibReader),
            5 => Ok(DiagnosticPhase::Codegen),
            6 => Ok(DiagnosticPhase::Link),
            _ => Err(ProtocolError::UnknownMessageTag(tag)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

impl Severity {
    fn tag(self) -> u64 {
        match self {
            Severity::Error => 1,
            Severity::Warning => 2,
        }
    }

    fn from_tag(tag: u64) -> Result<Self, ProtocolError> {
        match tag {
            1 => Ok(Severity::Error),
            2 => Ok(Severity::Warning),
            _ => Err(ProtocolError::UnknownMessageTag(tag)),
        }
    }
}

/// Optional source attribution: a Cone-relative path plus a byte span.
/// Host absolute paths never appear here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceLocation {
    pub path: String,
    pub span_start: u32,
    pub span_end: u32,
}

/// One typed diagnostic crossing the subprocess boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolDiagnostic {
    pub severity: Severity,
    pub phase: DiagnosticPhase,
    pub message: String,
    pub source: Option<SourceLocation>,
}

impl ProtocolDiagnostic {
    fn encode(&self, writer: &mut CborWriter) {
        let entries = if self.source.is_some() { 4 } else { 3 };
        writer.map(entries);
        writer.field(1).unsigned(self.severity.tag());
        writer.field(2).unsigned(self.phase.tag());
        writer.field(3).text(&self.message);
        if let Some(source) = &self.source {
            writer.field(4).map(3);
            writer.field(1).text(&source.path);
            writer.field(2).unsigned(source.span_start as u64);
            writer.field(3).unsigned(source.span_end as u64);
        }
    }

    fn decode(reader: &mut CborReader<'_>) -> Result<Self, ProtocolError> {
        let mut record = reader.map()?;
        let count = record.remaining_entries();
        if !(3..=4).contains(&count) {
            return Err(ProtocolError::WrongEntryCount {
                expected: 3,
                actual: count,
            });
        }
        if record.next_key()? != Some(1) {
            return Err(ProtocolError::UnknownField(0));
        }
        let severity = Severity::from_tag(record.unsigned()?)?;
        if record.next_key()? != Some(2) {
            return Err(ProtocolError::UnknownField(0));
        }
        let phase = DiagnosticPhase::from_tag(record.unsigned()?)?;
        if record.next_key()? != Some(3) {
            return Err(ProtocolError::UnknownField(0));
        }
        let message = record.text()?.to_owned();
        let source = match record.next_key()? {
            None => None,
            Some(4) => {
                let mut location = record.map()?;
                expect_entries(&mut location, 3)?;
                if location.next_key()? != Some(1) {
                    return Err(ProtocolError::UnknownField(0));
                }
                let path = location.text()?.to_owned();
                if location.next_key()? != Some(2) {
                    return Err(ProtocolError::UnknownField(0));
                }
                let span_start = read_u32(location.unsigned()?)?;
                if location.next_key()? != Some(3) {
                    return Err(ProtocolError::UnknownField(0));
                }
                let span_end = read_u32(location.unsigned()?)?;
                if location.next_key()?.is_some() {
                    return Err(ProtocolError::UnknownField(0));
                }
                Some(SourceLocation {
                    path,
                    span_start,
                    span_end,
                })
            }
            Some(key) => return Err(ProtocolError::UnknownField(key)),
        };
        if record.next_key()?.is_some() {
            return Err(ProtocolError::UnknownField(0));
        }
        Ok(ProtocolDiagnostic {
            severity,
            phase,
            message,
            source,
        })
    }
}

/// One `scoopc build` invocation. Paths are location hints only: they
/// never enter artifact identity or fingerprints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildRequest {
    /// Root of the Cone to compile (`Cone.toml` directory).
    pub cone_root: String,
    /// Trusted core artifact from the sysroot slot.
    pub core_slib: String,
    /// Direct dependency artifacts, one per manifest dependency.
    pub direct_slibs: Vec<String>,
    /// Transitive support closure required by the direct artifacts.
    pub support_slibs: Vec<String>,
    /// Where the current Cone's `.slib` must be published.
    pub out_slib: String,
}

impl BuildRequest {
    fn encode(&self, writer: &mut CborWriter) {
        writer.map(5);
        writer.field(1).text(&self.cone_root);
        writer.field(2).text(&self.core_slib);
        writer.field(3).array(self.direct_slibs.len() as u64);
        for path in &self.direct_slibs {
            writer.text(path);
        }
        writer.field(4).array(self.support_slibs.len() as u64);
        for path in &self.support_slibs {
            writer.text(path);
        }
        writer.field(5).text(&self.out_slib);
    }

    fn decode(reader: &mut CborReader<'_>) -> Result<Self, ProtocolError> {
        let mut record = reader.map()?;
        expect_entries(&mut record, 5)?;
        expect_key(&mut record, 1)?;
        let cone_root = record.text()?.to_owned();
        expect_key(&mut record, 2)?;
        let core_slib = record.text()?.to_owned();
        expect_key(&mut record, 3)?;
        let direct_slibs = read_string_sequence(&mut record)?;
        expect_key(&mut record, 4)?;
        let support_slibs = read_string_sequence(&mut record)?;
        expect_key(&mut record, 5)?;
        let out_slib = record.text()?.to_owned();
        if record.next_key()?.is_some() {
            return Err(ProtocolError::UnknownField(0));
        }
        Ok(BuildRequest {
            cone_root,
            core_slib,
            direct_slibs,
            support_slibs,
            out_slib,
        })
    }
}

fn read_string_sequence(reader: &mut CborReader<'_>) -> Result<Vec<String>, ProtocolError> {
    let mut items = reader.array()?;
    let mut out = Vec::with_capacity(items.count() as usize);
    for _ in 0..items.count() {
        out.push(items.text()?.to_owned());
    }
    Ok(out)
}

/// Result of one build invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildOutcome {
    Success {
        /// The coordinate the artifact claims; the parent re-verifies it
        /// against the plan and the `.slib` itself.
        coordinate: ConeCoordinate,
        warnings: Vec<ProtocolDiagnostic>,
    },
    Failure {
        diagnostics: Vec<ProtocolDiagnostic>,
    },
}

impl BuildOutcome {
    fn encode(&self, writer: &mut CborWriter) {
        match self {
            BuildOutcome::Success {
                coordinate,
                warnings,
            } => {
                writer.map(3);
                writer.field(0).unsigned(1);
                writer.field(1).text(&coordinate.display());
                writer.field(2).array(warnings.len() as u64);
                for warning in warnings {
                    warning.encode(writer);
                }
            }
            BuildOutcome::Failure { diagnostics } => {
                writer.map(2);
                writer.field(0).unsigned(2);
                writer.field(1).array(diagnostics.len() as u64);
                for diagnostic in diagnostics {
                    diagnostic.encode(writer);
                }
            }
        }
    }

    fn decode(reader: &mut CborReader<'_>) -> Result<Self, ProtocolError> {
        let mut record = reader.map()?;
        let count = record.remaining_entries();
        if !(2..=3).contains(&count) {
            return Err(ProtocolError::WrongEntryCount {
                expected: 2,
                actual: count,
            });
        }
        if record.next_key()? != Some(0) {
            return Err(ProtocolError::UnknownField(0));
        }
        match record.unsigned()? {
            1 => {
                if record.next_key()? != Some(1) {
                    return Err(ProtocolError::UnknownField(0));
                }
                let display = record.text()?;
                let (group, rest) = display
                    .split_once(':')
                    .ok_or(CoordinateError::EmptyComponent("coordinate display"))?;
                let (name, version) = rest
                    .split_once(':')
                    .ok_or(CoordinateError::EmptyComponent("coordinate display"))?;
                let coordinate = ConeCoordinate::new(group, name, version)?;
                if record.next_key()? != Some(2) {
                    return Err(ProtocolError::UnknownField(0));
                }
                let mut items = record.array()?;
                let mut warnings = Vec::with_capacity(items.count() as usize);
                for _ in 0..items.count() {
                    warnings.push(ProtocolDiagnostic::decode(&mut items)?);
                }
                drop(items);
                if record.next_key()?.is_some() {
                    return Err(ProtocolError::UnknownField(0));
                }
                Ok(BuildOutcome::Success {
                    coordinate,
                    warnings,
                })
            }
            2 => {
                if record.next_key()? != Some(1) {
                    return Err(ProtocolError::UnknownField(0));
                }
                let mut items = record.array()?;
                let mut diagnostics = Vec::with_capacity(items.count() as usize);
                for _ in 0..items.count() {
                    diagnostics.push(ProtocolDiagnostic::decode(&mut items)?);
                }
                drop(items);
                if record.next_key()?.is_some() {
                    return Err(ProtocolError::UnknownField(0));
                }
                Ok(BuildOutcome::Failure { diagnostics })
            }
            tag => Err(ProtocolError::UnknownMessageTag(tag)),
        }
    }
}

/// Toolchain identity handshake.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hello {
    pub identity: CompilerIdentity,
    /// Digest the toolchain build was stamped with, if any; `scoop`
    /// compares it against its own toolchain stamp to refuse `PATH`
    /// picks of another compiler build.
    pub toolchain_stamp: Option<Digest256>,
}

/// The top-level message sum: `{0: variant tag, 1: payload}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    Hello(Hello),
    BuildRequest(BuildRequest),
    BuildOutcome(BuildOutcome),
}

impl Message {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = CborWriter::new();
        writer.map(2);
        match self {
            Message::Hello(hello) => {
                writer.field(0).unsigned(1);
                writer.field(1);
                hello.encode(&mut writer);
            }
            Message::BuildRequest(request) => {
                writer.field(0).unsigned(2);
                writer.field(1);
                request.encode(&mut writer);
            }
            Message::BuildOutcome(outcome) => {
                writer.field(0).unsigned(3);
                writer.field(1);
                outcome.encode(&mut writer);
            }
        }
        writer.into_bytes()
    }

    pub fn decode(data: &[u8]) -> Result<Self, ProtocolError> {
        let mut reader = CborReader::new(data, crate::MAX_NESTING);
        let mut record = reader.map()?;
        expect_entries(&mut record, 2)?;
        if record.next_key()? != Some(0) {
            return Err(ProtocolError::UnknownField(0));
        }
        let tag = record.unsigned()?;
        if record.next_key()? != Some(1) {
            return Err(ProtocolError::UnknownField(0));
        }
        let message = match tag {
            1 => Message::Hello(Hello::decode(&mut record)?),
            2 => Message::BuildRequest(BuildRequest::decode(&mut record)?),
            3 => Message::BuildOutcome(BuildOutcome::decode(&mut record)?),
            other => return Err(ProtocolError::UnknownMessageTag(other)),
        };
        if record.next_key()?.is_some() {
            return Err(ProtocolError::UnknownField(0));
        }
        drop(record);
        reader.finish()?;
        Ok(message)
    }
}

impl Hello {
    /// `{1: CompilerIdentity, 2: stamp bytes (empty when absent)}`.
    fn encode(&self, writer: &mut CborWriter) {
        writer.map(2);
        writer.field(1);
        self.identity.encode(writer);
        writer.field(2).bytes(match &self.toolchain_stamp {
            Some(stamp) => stamp.as_bytes(),
            None => &[],
        });
    }

    fn decode(reader: &mut CborReader<'_>) -> Result<Self, ProtocolError> {
        let mut record = reader.map()?;
        expect_entries(&mut record, 2)?;
        if record.next_key()? != Some(1) {
            return Err(ProtocolError::UnknownField(0));
        }
        let identity = CompilerIdentity::decode(&mut record)?;
        if record.next_key()? != Some(2) {
            return Err(ProtocolError::UnknownField(0));
        }
        let stamp_bytes = record.bytes()?;
        let toolchain_stamp = if stamp_bytes.is_empty() {
            None
        } else if stamp_bytes.len() == 32 {
            let mut raw = [0u8; 32];
            raw.copy_from_slice(stamp_bytes);
            Some(Digest256::from_bytes(raw))
        } else {
            return Err(ProtocolError::Malformed(CborError::LengthExceedsInput {
                claimed: stamp_bytes.len() as u64,
                remaining: 32,
            }));
        };
        Ok(Hello {
            identity,
            toolchain_stamp,
        })
    }
}

fn expect_entries(
    record: &mut scoop_identity::MapGuard<'_, '_>,
    expected: u64,
) -> Result<(), ProtocolError> {
    let actual = record.remaining_entries();
    if actual == expected {
        Ok(())
    } else {
        Err(ProtocolError::WrongEntryCount { expected, actual })
    }
}

fn expect_key(
    record: &mut scoop_identity::MapGuard<'_, '_>,
    expected: u64,
) -> Result<(), ProtocolError> {
    match record.next_key()? {
        Some(key) if key == expected => Ok(()),
        Some(key) => Err(ProtocolError::UnknownField(key)),
        None => Err(ProtocolError::MissingField(expected)),
    }
}

/// Writes one length-prefixed frame.
pub fn write_frame<W: Write>(sink: &mut W, message: &Message) -> Result<(), ProtocolError> {
    let payload = message.encode();
    let length =
        u32::try_from(payload.len()).map_err(|_| ProtocolError::FrameTooLarge(u32::MAX))?;
    if length > MAX_FRAME_BYTES {
        return Err(ProtocolError::FrameTooLarge(length));
    }
    sink.write_all(&length.to_le_bytes())?;
    sink.write_all(&payload)?;
    Ok(())
}

/// Reads one length-prefixed frame.
pub fn read_frame<R: Read>(source: &mut R) -> Result<Message, ProtocolError> {
    let mut length_bytes = [0u8; 4];
    source.read_exact(&mut length_bytes)?;
    let length = u32::from_le_bytes(length_bytes);
    if length > MAX_FRAME_BYTES {
        return Err(ProtocolError::FrameTooLarge(length));
    }
    let mut payload = vec![0u8; length as usize];
    source.read_exact(&mut payload)?;
    Message::decode(&payload)
}
