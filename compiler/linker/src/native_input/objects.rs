use super::*;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct NativeArchiveMemberId {
    pub input: NativeInputId,
    pub ordinal: u64,
    pub header_offset: u64,
    pub payload_offset: u64,
    pub payload_length: u64,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum NativeObjectId {
    Direct(NativeInputId),
    ArchiveMember(NativeArchiveMemberId),
}

impl NativeObjectId {
    pub fn input(self) -> NativeInputId {
        match self {
            Self::Direct(id) => id,
            Self::ArchiveMember(member) => member.input,
        }
    }
}
impl std::fmt::Display for NativeObjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Direct(id) => id.fmt(f),
            Self::ArchiveMember(member) => write!(
                f,
                "{}-member-{}-{}",
                member.input, member.ordinal, member.header_offset
            ),
        }
    }
}
impl WireEncode for NativeObjectId {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Direct(id) => {
                e.array(2)?;
                e.unsigned(1)?;
                id.encode(e)
            }
            Self::ArchiveMember(member) => {
                e.array(6)?;
                e.unsigned(2)?;
                member.input.encode(e)?;
                e.unsigned(member.ordinal)?;
                e.unsigned(member.header_offset)?;
                e.unsigned(member.payload_offset)?;
                e.unsigned(member.payload_length)
            }
        }
    }
}

impl NativeFile {
    pub fn candidate(&self, symbol: &str) -> Option<(NativeObjectId, &NativeObjectIndex)> {
        match &self.content {
            NativeContent::System | NativeContent::Dynamic(_) | NativeContent::ElfDynamic(_) => {
                None
            }
            NativeContent::Object(index) => index
                .info
                .definitions
                .contains_key(symbol)
                .then_some((NativeObjectId::Direct(self.id), index)),
            NativeContent::Archive(members) => members
                .iter()
                .find(|member| member.index.info.definitions.contains_key(symbol))
                .map(|member| (NativeObjectId::ArchiveMember(member.id), &member.index)),
        }
    }

    pub fn objects(&self) -> Vec<(NativeObjectId, &NativeObjectIndex, Range<usize>)> {
        match &self.content {
            NativeContent::System | NativeContent::Dynamic(_) | NativeContent::ElfDynamic(_) => {
                Vec::new()
            }
            NativeContent::Object(index) => {
                vec![(NativeObjectId::Direct(self.id), index, self.slice.clone())]
            }
            NativeContent::Archive(members) => members
                .iter()
                .map(|member| {
                    (
                        NativeObjectId::ArchiveMember(member.id),
                        &member.index,
                        member.range.clone(),
                    )
                })
                .collect(),
        }
    }
}

impl NativeInputs {
    pub fn object(
        &self,
        id: NativeObjectId,
    ) -> Result<(&NativeObjectIndex, Range<usize>), LinkError> {
        self.files[&id.input()]
            .objects()
            .into_iter()
            .find(|(candidate, _, _)| *candidate == id)
            .map(|(_, index, range)| (index, range))
            .ok_or_else(|| error(format!("missing native object {id}")))
    }
}
