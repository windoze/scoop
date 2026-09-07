//! Canonical SysV/GNU short-name `ar` container (DESIGN section 4.1).
//!
//! The writer emits exactly one byte spelling: `!<arch>\n`, then 60-byte
//! headers whose name field is `name/` space-padded to 16 bytes, zero
//! mtime/uid/gid, mode `100644`, minimal decimal size space-padded to 10
//! bytes, the `` `'\n' `` terminator, and a single `0x0A` pad byte after
//! odd-sized payloads. The reader accepts only that spelling, so two producers of
//! the same logical content produce identical bytes and any re-spelling
//! is a corruption error.

use core::fmt;

const GLOBAL_MAGIC: &[u8; 8] = b"!<arch>\n";
const HEADER_SIZE: usize = 60;

/// Archive container error, with the member name when one is involved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArchiveError {
    MissingGlobalMagic,
    TruncatedHeader,
    TruncatedPayload {
        name: String,
        declared: usize,
        available: usize,
    },
    NonCanonicalField {
        name: String,
        field: &'static str,
    },
    InvalidName(String),
    ReservedMemberName(String),
    DuplicateMemberName(String),
    NonUtf8Name,
    BadPadByte {
        name: String,
    },
    TrailingBytes(usize),
}

impl fmt::Display for ArchiveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArchiveError::MissingGlobalMagic => {
                write!(f, "archive is missing the `!<arch>\\n` magic")
            }
            ArchiveError::TruncatedHeader => write!(f, "truncated archive header"),
            ArchiveError::TruncatedPayload {
                name,
                declared,
                available,
            } => write!(
                f,
                "member {name:?} declares {declared} bytes but only {available} remain"
            ),
            ArchiveError::NonCanonicalField { name, field } => {
                write!(f, "member {name:?} has a non-canonical {field} field")
            }
            ArchiveError::InvalidName(name) => write!(f, "invalid member name {name:?}"),
            ArchiveError::ReservedMemberName(name) => write!(
                f,
                "reserved archive member name {name:?} is not allowed in a .slib"
            ),
            ArchiveError::DuplicateMemberName(name) => {
                write!(f, "duplicate archive member name {name:?}")
            }
            ArchiveError::NonUtf8Name => write!(f, "archive member name is not UTF-8"),
            ArchiveError::BadPadByte { name } => write!(
                f,
                "member {name:?} with odd length is not followed by the 0x0A pad byte"
            ),
            ArchiveError::TrailingBytes(count) => {
                write!(f, "{count} trailing byte(s) after the last archive member")
            }
        }
    }
}

impl std::error::Error for ArchiveError {}

/// One decoded archive member.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveMember<'a> {
    pub name: String,
    pub payload: &'a [u8],
}

/// Serializes members in the given order using the canonical spelling.
pub fn write_archive(members: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(GLOBAL_MAGIC);
    for (name, payload) in members {
        write_header(&mut out, name, payload.len());
        out.extend_from_slice(payload);
        if payload.len() % 2 == 1 {
            out.push(b'\n');
        }
    }
    out
}

fn write_header(out: &mut Vec<u8>, name: &str, size: usize) {
    debug_assert!(name.len() < 16, "physical names fit the 16-byte field");
    let mut field = |text: &str, width: usize| {
        let bytes = text.as_bytes();
        debug_assert!(bytes.len() <= width);
        out.extend_from_slice(bytes);
        for _ in bytes.len()..width {
            out.push(b' ');
        }
    };
    field(&format!("{name}/"), 16);
    field("0", 12); // mtime
    field("0", 6); // uid
    field("0", 6); // gid
    field("100644", 8); // mode (octal)
    field(&size.to_string(), 10);
    out.push(b'`');
    out.push(b'\n');
}

/// Parses an archive, enforcing the canonical spelling field by field.
pub fn read_archive(data: &[u8]) -> Result<Vec<ArchiveMember<'_>>, ArchiveError> {
    if data.len() < GLOBAL_MAGIC.len() || &data[..8] != GLOBAL_MAGIC {
        return Err(ArchiveError::MissingGlobalMagic);
    }
    let mut position = GLOBAL_MAGIC.len();
    let mut members = Vec::new();
    let mut seen_names = std::collections::HashSet::new();
    while position < data.len() {
        if data.len() - position < HEADER_SIZE {
            return Err(ArchiveError::TruncatedHeader);
        }
        let header = &data[position..position + HEADER_SIZE];
        position += HEADER_SIZE;
        let name = parse_name_field(&header[0..16])?;
        if !field_is(&header[16..28], "0") {
            return Err(ArchiveError::NonCanonicalField {
                name: name.clone(),
                field: "mtime",
            });
        }
        if !field_is(&header[28..34], "0") {
            return Err(ArchiveError::NonCanonicalField {
                name: name.clone(),
                field: "uid",
            });
        }
        if !field_is(&header[34..40], "0") {
            return Err(ArchiveError::NonCanonicalField {
                name: name.clone(),
                field: "gid",
            });
        }
        if !field_is(&header[40..48], "100644") {
            return Err(ArchiveError::NonCanonicalField {
                name: name.clone(),
                field: "mode",
            });
        }
        let size = parse_size_field(&header[48..58], &name)?;
        if header[58] != b'`' || header[59] != b'\n' {
            return Err(ArchiveError::NonCanonicalField {
                name: name.clone(),
                field: "terminator",
            });
        }
        if data.len() - position < size {
            return Err(ArchiveError::TruncatedPayload {
                name,
                declared: size,
                available: data.len() - position,
            });
        }
        let payload = &data[position..position + size];
        position += size;
        if size % 2 == 1 {
            match data.get(position) {
                Some(b'\n') => position += 1,
                _ => {
                    return Err(ArchiveError::BadPadByte { name });
                }
            }
        }
        if !seen_names.insert(name.clone()) {
            return Err(ArchiveError::DuplicateMemberName(name));
        }
        members.push(ArchiveMember { name, payload });
    }
    Ok(members)
}

/// A header field is canonical when it holds exactly `text` followed by
/// spaces to the field width.
fn field_is(field: &[u8], text: &str) -> bool {
    let bytes = text.as_bytes();
    field.len() >= bytes.len()
        && &field[..bytes.len()] == bytes
        && field[bytes.len()..].iter().all(|b| *b == b' ')
}

fn parse_name_field(field: &[u8]) -> Result<String, ArchiveError> {
    let slash = field.iter().position(|b| *b == b'/');
    let name_end = match slash {
        Some(index) => index,
        None => {
            let raw = String::from_utf8_lossy(field).into_owned();
            return Err(ArchiveError::NonCanonicalField {
                name: raw,
                field: "name",
            });
        }
    };
    if !field[name_end + 1..].iter().all(|b| *b == b' ') {
        let raw = String::from_utf8_lossy(field).into_owned();
        return Err(ArchiveError::NonCanonicalField {
            name: raw,
            field: "name",
        });
    }
    let name_bytes = &field[..name_end];
    let name = String::from_utf8(name_bytes.to_vec()).map_err(|_| ArchiveError::NonUtf8Name)?;
    if name.is_empty() {
        return Err(ArchiveError::ReservedMemberName(name));
    }
    // Thin-archive, long-name-table and symbol-table members never appear
    // in a self-contained `.slib`.
    if name.starts_with('/') || name == "//" || name.starts_with("__.SYMDEF") {
        return Err(ArchiveError::ReservedMemberName(name));
    }
    if name.contains('/') || name.contains('\\') || name.contains('\0') || name == ".." {
        return Err(ArchiveError::InvalidName(name));
    }
    Ok(name)
}

fn parse_size_field(field: &[u8], name: &str) -> Result<usize, ArchiveError> {
    let digits_end = field.iter().position(|b| *b == b' ').unwrap_or(field.len());
    let digits = &field[..digits_end];
    let tail_canonical = field[digits_end..].iter().all(|b| *b == b' ');
    if digits.is_empty() || !tail_canonical {
        return Err(ArchiveError::NonCanonicalField {
            name: name.to_owned(),
            field: "size",
        });
    }
    if !digits.iter().all(|b| b.is_ascii_digit()) {
        return Err(ArchiveError::NonCanonicalField {
            name: name.to_owned(),
            field: "size",
        });
    }
    if digits.len() > 1 && digits[0] == b'0' {
        return Err(ArchiveError::NonCanonicalField {
            name: name.to_owned(),
            field: "size",
        });
    }
    std::str::from_utf8(digits)
        .expect("digits are ASCII")
        .parse::<usize>()
        .map_err(|_| ArchiveError::NonCanonicalField {
            name: name.to_owned(),
            field: "size",
        })
}
