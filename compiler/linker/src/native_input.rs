//! Immutable native files selected by the artifact's logical library requirements.
use std::collections::BTreeMap;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use scoop_identity::{NativeLibraryGrouping, NativeLinkRequirementId, NativeLinkRequirementKey};
use scoop_toolchain::ValidatedFinalLinkProfile;
use scoop_wire::{Digest256, Encoder, WireEncode, domain_separated_cbor_hash, sha256};

use crate::{LinkError, error, native_object::NativeObjectIndex};

mod archive;
mod objects;
mod plan;
pub(crate) use objects::{NativeArchiveMemberId, NativeObjectId};
pub(crate) mod locate;
mod slice;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct NativeInputId(Digest256);
impl std::fmt::Display for NativeInputId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
impl WireEncode for NativeInputId {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.0.encode(e)
    }
}

pub(crate) struct NativeFile {
    pub id: NativeInputId,
    pub bytes: Arc<[u8]>,
    pub slice: Range<usize>,
    pub locator: PathBuf,
    pub content: NativeContent,
}

pub(crate) enum NativeContent {
    Object(NativeObjectIndex),
    Archive(Vec<archive::Member>),
    Dynamic(Vec<Arc<crate::dynamic::DynamicProvider>>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NativeFileKind {
    Object = 1,
    Archive = 2,
    Dylib = 3,
    TextStub = 4,
    Framework = 5,
}

impl NativeInputId {
    pub fn from_bytes(
        bytes: &[u8],
        kind: NativeFileKind,
        profile: &ValidatedFinalLinkProfile,
    ) -> Result<Self, LinkError> {
        Ok(Self(
            domain_separated_cbor_hash(
                "scoop-native-input-v1",
                &FileKey {
                    bytes,
                    kind: kind as u64,
                    profile,
                },
            )
            .map_err(error)?,
        ))
    }
}

pub(crate) struct LibraryInput {
    pub key: NativeLinkRequirementKey,
    pub origins: Vec<String>,
    pub input: NativeInputId,
}

#[derive(Default)]
pub(crate) struct NativeInputs {
    pub files: BTreeMap<NativeInputId, NativeFile>,
    pub libraries: BTreeMap<NativeLinkRequirementId, LibraryInput>,
    pub selected: BTreeMap<NativeObjectId, String>,
    pub references: BTreeMap<NativeObjectId, crate::native_object::NativeReferences>,
}

impl NativeInputs {
    pub fn read(
        requirements: BTreeMap<NativeLinkRequirementId, (NativeLinkRequirementKey, Vec<String>)>,
        roots: &[PathBuf],
        profile: &ValidatedFinalLinkProfile,
    ) -> Result<Self, LinkError> {
        let mut result = Self::default();
        let mut positions = BTreeMap::new();
        for (id, (key, origins)) in requirements {
            if let NativeLibraryGrouping::OrderedGroup { name, position } = key.grouping()
                && let Some(previous) = positions.insert((name.clone(), *position), id)
                && previous != id
            {
                return Err(error(format!(
                    "native library group {} position {position} conflicts: {previous}, {id}; origins: {}",
                    name.as_str(),
                    origins.join(", ")
                )));
            }
            let file = locate::library(&key, roots, profile).map_err(|err| {
                error(format!(
                    "{err}; requirement {id}; origins: {}",
                    origins.join(", ")
                ))
            })?;
            result.libraries.insert(
                id,
                LibraryInput {
                    key,
                    origins,
                    input: file.id,
                },
            );
            result.files.entry(file.id).or_insert(file);
        }
        Ok(result)
    }

    pub fn ordered_files(&self) -> Vec<&NativeFile> {
        let mut libraries: Vec<_> = self.libraries.iter().collect();
        libraries.sort_by_key(|(id, value)| (value.key.grouping(), **id));
        let mut seen = std::collections::BTreeSet::new();
        libraries
            .into_iter()
            .filter(|(_, library)| seen.insert(library.input))
            .map(|(_, library)| &self.files[&library.input])
            .collect()
    }
}

struct FileKey<'a> {
    profile: &'a ValidatedFinalLinkProfile,
    bytes: &'a [u8],
    kind: u64,
}
impl WireEncode for FileKey<'_> {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.array(4)?;
        self.profile.target().wire_id().encode(e)?;
        e.unsigned(self.kind)?;
        e.unsigned(self.bytes.len() as u64)?;
        sha256(self.bytes).encode(e)
    }
}
