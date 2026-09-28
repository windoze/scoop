use std::sync::Arc;

use scoop_identity::{
    CallableDefinitionOwner, CanonicalScoopAbiFunctionSignature, ConeIdentity,
    PersistentCallableBodyId, PersistentSymbolKey, RepresentationRole,
};

use super::ExactDispatchError;
use crate::{
    CallableAbiReceiverInputV1, ExactCallableAbiExportV1, ExactValueLayoutV1, LirTargetProfile,
    ParamFreeLirCallableExportV1, StrongShapeDefinitionRefV1,
};

/// Borrows the existing ABI record for an actual dispatch implementation.
#[derive(Clone, Copy, Debug)]
pub enum DispatchCallableAbiV1<'a> {
    Exact {
        record: &'a ExactCallableAbiExportV1,
        receiver: CallableAbiReceiverInputV1<'a>,
    },
    Direct {
        provider: ConeIdentity,
        target: LirTargetProfile,
        record: &'a ParamFreeLirCallableExportV1,
        receiver: CallableAbiReceiverInputV1<'a>,
    },
}

impl DispatchCallableAbiV1<'_> {
    pub fn target(self) -> CallableDefinitionOwner {
        match self {
            Self::Exact { record, .. } => record.target(),
            Self::Direct { record, .. } => record.target().into(),
        }
    }

    pub fn target_profile(self) -> LirTargetProfile {
        match self {
            Self::Exact { record, .. } => record.target_profile(),
            Self::Direct { target, .. } => target,
        }
    }

    pub fn provider(self) -> ConeIdentity {
        match self {
            Self::Exact { record, .. } => record.physical_definition().provider(),
            Self::Direct { provider, .. } => provider,
        }
    }

    pub fn canonical_signature(&self) -> &CanonicalScoopAbiFunctionSignature {
        match self {
            Self::Exact { record, .. } => record.canonical_signature(),
            Self::Direct { record, .. } => record.abi_signature(),
        }
    }

    pub(super) fn calling_convention(self) -> crate::CallingConvention {
        match self {
            Self::Exact { record, .. } => record.calling_convention(),
            Self::Direct { record, .. } => record.calling_convention(),
        }
    }

    pub(super) fn body(self) -> Result<PersistentCallableBodyId, ExactDispatchError> {
        match self {
            Self::Exact { record, .. } => Ok(record.definition().semantic_id()),
            Self::Direct { record, .. } => match record.expected_symbol().key() {
                PersistentSymbolKey::CallableBody(body) => Ok(body),
                _ => Err(ExactDispatchError::AbiDefinition(record.target().into())),
            },
        }
    }

    pub(super) fn matches_definition(self, actual: StrongShapeDefinitionRefV1) -> bool {
        match self {
            Self::Exact { record, .. } => record.physical_definition() == actual,
            Self::Direct {
                provider, record, ..
            } => {
                provider == actual.provider()
                    && record.required_definition() == actual.definition()
                    && record.expected_symbol() == actual.symbol()
            }
        }
    }

    pub(super) fn receiver_layout(
        self,
    ) -> Result<Option<Arc<ExactValueLayoutV1>>, ExactDispatchError> {
        let receiver = match self {
            Self::Exact { receiver, .. } | Self::Direct { receiver, .. } => receiver,
        };
        match (
            self.canonical_signature()
                .signature()
                .receiver()
                .into_option(),
            receiver,
        ) {
            (None, CallableAbiReceiverInputV1::NoReceiver) => Ok(None),
            (Some(exact), CallableAbiReceiverInputV1::Receiver(layout))
                if layout.identity().exact() == exact
                    && layout.identity().target() == self.target_profile()
                    && layout.identity().layout_key().representation()
                        == RepresentationRole::ManagedValue =>
            {
                layout
                    .value_handle()
                    .map(Some)
                    .ok_or(ExactDispatchError::ReceiverLayout(self.target()))
            }
            _ => Err(ExactDispatchError::ReceiverLayout(self.target())),
        }
    }
}
