use super::*;
use crate::{
    CheckedProtectedSourceInterfaceV1, ProtectedDefaultTemplateKeyV1, ProtectedDefaultTemplateV1,
    ProtectedDefaultWitnessSourceProfileV1,
};

/// Complete provider default proofs. A consumer must still commit a legal
/// source winner; only the param-free variant can enter M23-6 expansion.
#[derive(Debug)]
pub struct CheckedProtectedDefaultTemplatesV1<'a> {
    pub(super) table: &'a CanonicalProtectedDefaultTemplatesV1,
    pub(super) records: Vec<CheckedProtectedDefaultTemplateV1<'a>>,
}
impl<'a> CheckedProtectedDefaultTemplatesV1<'a> {
    pub const fn table(&self) -> &'a CanonicalProtectedDefaultTemplatesV1 {
        self.table
    }
    pub fn records(&self) -> &[CheckedProtectedDefaultTemplateV1<'a>] {
        &self.records
    }
    pub fn get(
        &self,
        key: ProtectedDefaultTemplateKeyV1,
    ) -> Option<CheckedProtectedDefaultTemplateV1<'a>> {
        self.records
            .binary_search_by_key(&key, CheckedProtectedDefaultTemplateV1::key)
            .ok()
            .map(|index| self.records[index])
    }
}
#[derive(Clone, Copy, Debug)]
struct Default<'a> {
    template: &'a ProtectedDefaultTemplateV1,
    source: CheckedProtectedSourceInterfaceV1<'a>,
}
#[derive(Clone, Copy, Debug)]
pub enum CheckedProtectedDefaultTemplateV1<'a> {
    ParamFree(CheckedParamFreeProtectedDefaultTemplateV1<'a>),
    GenericSourceMetadata(CheckedGenericProtectedDefaultTemplateV1<'a>),
}
#[derive(Clone, Copy, Debug)]
pub struct CheckedParamFreeProtectedDefaultTemplateV1<'a>(Default<'a>);
#[derive(Clone, Copy, Debug)]
pub struct CheckedGenericProtectedDefaultTemplateV1<'a>(Default<'a>);
impl<'a> CheckedProtectedDefaultTemplateV1<'a> {
    pub const fn key(&self) -> ProtectedDefaultTemplateKeyV1 {
        self.template().key()
    }
    pub const fn template(&self) -> &'a ProtectedDefaultTemplateV1 {
        self.value().template
    }
    pub const fn source(&self) -> CheckedProtectedSourceInterfaceV1<'a> {
        self.value().source
    }
    const fn value(&self) -> &Default<'a> {
        match self {
            Self::ParamFree(value) => &value.0,
            Self::GenericSourceMetadata(value) => &value.0,
        }
    }
}
impl<'a> CheckedParamFreeProtectedDefaultTemplateV1<'a> {
    pub const fn template(&self) -> &'a ProtectedDefaultTemplateV1 {
        self.0.template
    }
    pub const fn source(&self) -> CheckedProtectedSourceInterfaceV1<'a> {
        self.0.source
    }
}
impl<'a> CheckedGenericProtectedDefaultTemplateV1<'a> {
    pub const fn template(&self) -> &'a ProtectedDefaultTemplateV1 {
        self.0.template
    }
    pub const fn source(&self) -> CheckedProtectedSourceInterfaceV1<'a> {
        self.0.source
    }
}
pub(super) fn checked<'a>(
    template: &'a ProtectedDefaultTemplateV1,
    source: CheckedProtectedSourceInterfaceV1<'a>,
    profile: ProtectedDefaultWitnessSourceProfileV1,
) -> CheckedProtectedDefaultTemplateV1<'a> {
    let value = Default { template, source };
    match profile {
        ProtectedDefaultWitnessSourceProfileV1::ParamFree => {
            CheckedProtectedDefaultTemplateV1::ParamFree(
                CheckedParamFreeProtectedDefaultTemplateV1(value),
            )
        }
        ProtectedDefaultWitnessSourceProfileV1::GenericSourceMetadata => {
            CheckedProtectedDefaultTemplateV1::GenericSourceMetadata(
                CheckedGenericProtectedDefaultTemplateV1(value),
            )
        }
    }
}
