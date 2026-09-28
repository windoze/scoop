use std::borrow::Cow;

use super::*;
use scoop_identity::DependencyCallableDeclarationId;

/// A borrowed callable definition with its complete lowering signatures.
/// Direct declarations and generated/adapted definitions share typed lookup.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirCallableRecordRefV1<'a> {
    Direct(&'a crate::ParamFreeMirCallableExportV1),
    Lowered(&'a ParamFreeMirCallableBindingV1),
}

impl<'a> MirCallableRecordRefV1<'a> {
    pub fn implementation(self) -> CallableDefinitionOwner {
        match self {
            Self::Direct(record) => record.implementation().into(),
            Self::Lowered(record) => record.implementation(),
        }
    }

    pub fn origin(self) -> Cow<'a, MirCallableOriginV1> {
        match self {
            Self::Direct(record) => Cow::Owned(match record.declaration() {
                DependencyCallableDeclarationId::Function(id) => MirCallableOriginV1::Function(id),
                DependencyCallableDeclarationId::PropertyAccessor(id) => {
                    MirCallableOriginV1::Accessor(id)
                }
            }),
            Self::Lowered(record) => Cow::Borrowed(record.origin()),
        }
    }

    pub fn semantic_signature(self) -> &'a MirBridgeCallableSignatureV1 {
        match self {
            Self::Direct(record) => record.bridge_signature(),
            Self::Lowered(record) => record.semantic_signature(),
        }
    }

    pub fn lowered_signature(self) -> &'a MirBridgeCallableSignatureV1 {
        match self {
            Self::Direct(record) => record.bridge_signature(),
            Self::Lowered(record) => record.lowered_signature(),
        }
    }

    pub fn lowering_role(self) -> &'a MirCallableLoweringRoleV1 {
        match self {
            Self::Direct(record) => match record.declaration() {
                DependencyCallableDeclarationId::Function(_) => {
                    &MirCallableLoweringRoleV1::Ordinary
                }
                DependencyCallableDeclarationId::PropertyAccessor(_) => {
                    &MirCallableLoweringRoleV1::Accessor
                }
            },
            Self::Lowered(record) => record.lowering_role(),
        }
    }
}

/// One index over existing export records; no signatures or bodies are copied.
#[derive(Debug)]
pub struct MirTypeBridgeCallableIndexV1<'a> {
    records: Vec<MirCallableRecordRefV1<'a>>,
}

impl<'a> MirTypeBridgeCallableIndexV1<'a> {
    pub fn try_new(
        lowered: &[&'a CanonicalMirCallableBindingsV1],
        direct: &[&'a crate::CrossConeMirBridgeSectionV1],
    ) -> Result<Self, MirTypeBridgeLookupError> {
        let count = lowered
            .iter()
            .map(|table| table.entries().len())
            .chain(direct.iter().map(|table| table.exports().len()))
            .try_fold(0usize, |count, len| {
                count
                    .checked_add(len)
                    .ok_or(MirTypeBridgeLookupError::RecordCountOverflow)
            })?;
        let mut records = Vec::new();
        scoop_wire::allocation::try_reserve(&mut records, count, &WirePath::root())?;
        for table in lowered {
            records.extend(table.entries().iter().map(MirCallableRecordRefV1::Lowered));
        }
        for table in direct {
            records.extend(table.exports().iter().map(MirCallableRecordRefV1::Direct));
        }
        records.sort_unstable_by_key(|record| record.implementation());
        if let Some(pair) = records.windows(2).find(|pair| {
            pair[0].implementation() == pair[1].implementation()
                && !(matches!(pair[0].implementation(), CallableDefinitionOwner::Odr(_))
                    && pair[0] == pair[1])
        }) {
            return Err(MirTypeBridgeLookupError::DuplicateCallable {
                target: pair[0].implementation(),
            });
        }
        records.dedup_by_key(|record| record.implementation());
        Ok(Self { records })
    }
}

impl sealed::Sealed for MirTypeBridgeCallableIndexV1<'_> {}
impl MirTypeBridgeCallableLookupV1 for MirTypeBridgeCallableIndexV1<'_> {
    fn get(&self, target: CallableDefinitionOwner) -> Option<MirCallableRecordRefV1<'_>> {
        self.records
            .binary_search_by_key(&target, |record| record.implementation())
            .ok()
            .map(|index| self.records[index])
    }

    fn record_count(&self) -> usize {
        self.records.len()
    }
}
