//! Applied default types share the original definition and its lexical bindings.

use crate::*;
use scoop_identity::SignatureTypeKey;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct SemanticDefaultApplication {
    definition: Arc<ExportDefaultTemplateV1>,
    provider: DefaultTemplateProviderShapeV1,
    arguments: CanonicalBinderUseListV1,
}

impl SemanticDefaultApplication {
    pub fn try_new(
        definition: Arc<ExportDefaultTemplateV1>,
        provider: DefaultTemplateProviderShapeV1,
        arguments: CanonicalBinderUseListV1,
    ) -> Result<Self, DefaultTemplateTypeSubstitutionError> {
        if arguments.len_u32() != provider.binder_arity() {
            return Err(DefaultTemplateTypeSubstitutionError::MappingArity {
                expected: provider.binder_arity(),
                actual: arguments.len_u32(),
            });
        }
        Ok(Self {
            definition,
            provider,
            arguments,
        })
    }

    pub fn definition(&self) -> &Arc<ExportDefaultTemplateV1> {
        &self.definition
    }

    pub const fn provider(&self) -> DefaultTemplateProviderShapeV1 {
        self.provider
    }

    pub const fn arguments(&self) -> &CanonicalBinderUseListV1 {
        &self.arguments
    }

    pub fn applied_type(
        &self,
        signature: &SignatureTypeKey,
    ) -> Result<SignatureTypeKey, DefaultTemplateTypeSubstitutionError> {
        self.applied_type_in_scope(signature, 0)
    }

    /// `introduced_binders` counts local generic scopes added inside the
    /// definition; ordinary expression types use `applied_type` instead.
    pub fn applied_type_in_scope(
        &self,
        signature: &SignatureTypeKey,
        introduced_binders: u32,
    ) -> Result<SignatureTypeKey, DefaultTemplateTypeSubstitutionError> {
        self.arguments.substitute_provider_type_in_scope(
            self.provider,
            signature,
            introduced_binders,
        )
    }
}
