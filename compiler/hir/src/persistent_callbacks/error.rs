use crate::{ForeignCallbackRegistrationId, HirSignatureTypeMappingError};

#[derive(Debug)]
pub struct HirCallbackRegistrationIdentityError {
    registration: Option<ForeignCallbackRegistrationId>,
    detail: HirCallbackRegistrationIdentityErrorDetail,
}

impl HirCallbackRegistrationIdentityError {
    pub(super) const fn new(
        registration: ForeignCallbackRegistrationId,
        detail: HirCallbackRegistrationIdentityErrorDetail,
    ) -> Self {
        Self {
            registration: Some(registration),
            detail,
        }
    }

    pub(super) const fn invalid_unit() -> Self {
        Self {
            registration: None,
            detail: HirCallbackRegistrationIdentityErrorDetail::InvalidUnitType,
        }
    }

    pub(super) const fn invalid_path(registration: ForeignCallbackRegistrationId) -> Self {
        Self::new(
            registration,
            HirCallbackRegistrationIdentityErrorDetail::InvalidDefinitionPath,
        )
    }

    pub(super) const fn invalid_function_type(registration: ForeignCallbackRegistrationId) -> Self {
        Self::new(
            registration,
            HirCallbackRegistrationIdentityErrorDetail::InvalidFunctionType,
        )
    }

    pub(super) const fn invalid_context_index(registration: ForeignCallbackRegistrationId) -> Self {
        Self::new(
            registration,
            HirCallbackRegistrationIdentityErrorDetail::InvalidContextIndex,
        )
    }

    pub(super) const fn invalid_native_signature(
        registration: ForeignCallbackRegistrationId,
    ) -> Self {
        Self::new(
            registration,
            HirCallbackRegistrationIdentityErrorDetail::InvalidNativeSignature,
        )
    }

    pub(super) const fn signature_relation(registration: ForeignCallbackRegistrationId) -> Self {
        Self::new(
            registration,
            HirCallbackRegistrationIdentityErrorDetail::SignatureRelation,
        )
    }

    pub(super) const fn invalid_mode(registration: ForeignCallbackRegistrationId) -> Self {
        Self::new(
            registration,
            HirCallbackRegistrationIdentityErrorDetail::InvalidMode,
        )
    }

    pub(super) const fn signature_type(
        registration: ForeignCallbackRegistrationId,
        error: HirSignatureTypeMappingError,
    ) -> Self {
        Self::new(
            registration,
            HirCallbackRegistrationIdentityErrorDetail::InvalidSignatureType(error),
        )
    }

    pub(super) const fn identity(
        registration: ForeignCallbackRegistrationId,
        error: scoop_wire::HashError,
    ) -> Self {
        Self::new(
            registration,
            HirCallbackRegistrationIdentityErrorDetail::InvalidIdentity(error),
        )
    }

    pub(super) const fn conflicting_site(registration: ForeignCallbackRegistrationId) -> Self {
        Self::new(
            registration,
            HirCallbackRegistrationIdentityErrorDetail::ConflictingDefinitionSite,
        )
    }

    pub(super) const fn duplicate_identity(registration: ForeignCallbackRegistrationId) -> Self {
        Self::new(
            registration,
            HirCallbackRegistrationIdentityErrorDetail::DuplicateIdentity,
        )
    }

    pub const fn registration(&self) -> Option<ForeignCallbackRegistrationId> {
        self.registration
    }
}

#[derive(Debug)]
pub(super) enum HirCallbackRegistrationIdentityErrorDetail {
    InvalidUnitType,
    InvalidDefinitionPath,
    UnknownFunction,
    InvalidDefinitionRoot,
    AmbiguousLexicalParent,
    CyclicLexicalParent,
    InvalidFunctionIdentity,
    InvalidBinderRelation,
    TooManyBinders,
    BinderDepthOverflow,
    InvalidFunctionType,
    InvalidContextIndex,
    InvalidNativeSignature,
    SignatureRelation,
    InvalidMode,
    InvalidSignatureType(HirSignatureTypeMappingError),
    InvalidIdentity(scoop_wire::HashError),
    ConflictingDefinitionSite,
    DuplicateIdentity,
}

impl std::fmt::Display for HirCallbackRegistrationIdentityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use HirCallbackRegistrationIdentityErrorDetail as Detail;
        match &self.detail {
            Detail::InvalidUnitType => formatter.write_str("HIR Unit type relation is invalid"),
            Detail::InvalidDefinitionPath => {
                formatter.write_str("callback registration has no callback-conversion path")
            }
            Detail::UnknownFunction => {
                formatter.write_str("callback registration references an unknown function")
            }
            Detail::InvalidDefinitionRoot => {
                formatter.write_str("callback registration has an invalid definition root")
            }
            Detail::AmbiguousLexicalParent => {
                formatter.write_str("callback registration has multiple lexical parents")
            }
            Detail::CyclicLexicalParent => {
                formatter.write_str("callback registration has a cyclic lexical parent")
            }
            Detail::InvalidFunctionIdentity => formatter
                .write_str("callback registration parent has no lexical callable identity"),
            Detail::InvalidBinderRelation => formatter
                .write_str("callback registration parent has an inconsistent binder relation"),
            Detail::TooManyBinders => {
                formatter.write_str("callback registration parent has too many type parameters")
            }
            Detail::BinderDepthOverflow => {
                formatter.write_str("callback registration binder depth exceeds u32")
            }
            Detail::InvalidFunctionType => {
                formatter.write_str("callback registration has an invalid function type")
            }
            Detail::InvalidContextIndex => {
                formatter.write_str("callback registration context index is out of bounds")
            }
            Detail::InvalidNativeSignature => formatter.write_str(
                "callback registration native signature must be ordinary and cannot take Unit",
            ),
            Detail::SignatureRelation => formatter.write_str(
                "callback registration managed signature does not remove exactly its context parameter",
            ),
            Detail::InvalidMode => {
                formatter.write_str("callback registration has an invalid callback mode")
            }
            Detail::InvalidSignatureType(error) => error.fmt(formatter),
            Detail::InvalidIdentity(error) => error.fmt(formatter),
            Detail::ConflictingDefinitionSite => formatter.write_str(
                "one callback-conversion definition site has conflicting declarations",
            ),
            Detail::DuplicateIdentity => {
                formatter.write_str("distinct callback registrations have the same identity")
            }
        }
    }
}

impl std::error::Error for HirCallbackRegistrationIdentityError {}
