//! Canonical local table and request-local lookup relation for one template.

use std::collections::HashMap;

use scoop_identity::LocalValueSelector;

use super::{entities::DefaultEntityProjector, errors::DefaultTemplateEnvelopeProjectionError};
use crate::{
    CanonicalBooleanV1, CanonicalTemplateLocalTableV1, HirSignatureBinder, Local, LocalId,
    LocalValueDefinitionSite, TemplateLocalDefinitionV1, TemplateLocalRecordV1,
};

pub(super) struct TemplateLocalProjection {
    selectors: Vec<LocalValueSelector>,
    bindings: HashMap<crate::BindingId, LocalValueSelector>,
    captures: HashMap<crate::BindingId, crate::DefaultCaptureSourceV1>,
    constructor_parameters: HashMap<crate::ConstructorParamId, LocalValueSelector>,
    initializing_receiver: Option<scoop_identity::SignatureTypeKey>,
}

impl TemplateLocalProjection {
    pub(super) fn project(
        entities: &DefaultEntityProjector<'_>,
        locals: &la_arena::Arena<Local>,
        binders: &[HirSignatureBinder],
    ) -> Result<(Self, CanonicalTemplateLocalTableV1), DefaultTemplateEnvelopeProjectionError> {
        let mut selectors = Vec::with_capacity(locals.len());
        let mut bindings = HashMap::with_capacity(locals.len());
        let mut records = Vec::with_capacity(locals.len());
        for (local_id, local) in locals.iter() {
            let selector = local.selector.clone();
            if bindings.insert(local.binding, selector.clone()).is_some() {
                return Err(
                    DefaultTemplateEnvelopeProjectionError::DuplicateLocalBinding(
                        local.binding.into_raw(),
                    ),
                );
            }
            let definition = match local.definition {
                LocalValueDefinitionSite::Source(origin) => TemplateLocalDefinitionV1::Source(
                    super::super::definition_sources::project_definition_source(
                        entities.export(),
                        origin,
                    )
                    .map_err(DefaultTemplateEnvelopeProjectionError::DefinitionOrigin)?,
                ),
                LocalValueDefinitionSite::Synthetic => TemplateLocalDefinitionV1::Synthetic,
            };
            let record = TemplateLocalRecordV1::try_new(
                selector.clone(),
                entities
                    .type_key(local.ty, binders)
                    .map_err(DefaultTemplateEnvelopeProjectionError::Provider)?,
                CanonicalBooleanV1::from(local.mutable),
                definition,
            )
            .map_err(
                |source| DefaultTemplateEnvelopeProjectionError::LocalRecord {
                    local: super::raw_index(local_id),
                    source,
                },
            )?;
            selectors.push(selector);
            records.push(record);
        }
        let table = CanonicalTemplateLocalTableV1::try_new(records)
            .map_err(DefaultTemplateEnvelopeProjectionError::LocalTable)?;
        Ok((
            Self {
                selectors,
                bindings,
                captures: HashMap::new(),
                constructor_parameters: HashMap::new(),
                initializing_receiver: None,
            },
            table,
        ))
    }

    pub(super) fn selector(
        &self,
        local: LocalId,
    ) -> Result<LocalValueSelector, super::DefaultBodyProjectionError> {
        let selector = self.selectors.get(super::raw_index(local) as usize).ok_or(
            super::DefaultBodyProjectionError::UnknownLocal(super::raw_index(local)),
        )?;

        Ok(selector.clone())
    }

    pub(super) fn capture_source(
        &self,
        binding: crate::BindingId,
    ) -> Result<crate::DefaultCaptureSourceV1, super::DefaultBodyProjectionError> {
        if let Some(source) = self.captures.get(&binding) {
            return Ok(source.clone());
        }
        let selector = self.bindings.get(&binding).ok_or(
            super::DefaultBodyProjectionError::UnknownBinding(binding.into_raw()),
        )?;

        Ok(crate::DefaultCaptureSourceV1::Local(selector.clone()))
    }

    pub(super) fn bind_capture(
        &mut self,
        binding: crate::BindingId,
        source: crate::DefaultCaptureSourceV1,
    ) {
        self.captures.insert(binding, source);
    }

    pub(super) fn bind_constructor_inputs(
        &mut self,
        parameters: &[crate::ConstructorParameter],
        receiver: scoop_identity::SignatureTypeKey,
    ) {
        for (index, parameter) in parameters.iter().enumerate() {
            let selector = LocalValueSelector::Parameter {
                declaration_index: u32::try_from(index)
                    .expect("a constructor parameter position fits its source interface"),
            };
            self.constructor_parameters
                .insert(parameter.id, selector.clone());
            self.bindings.insert(parameter.binding, selector);
        }
        self.initializing_receiver = Some(receiver);
    }

    pub(super) fn constructor_parameter(
        &self,
        parameter: crate::ConstructorParamId,
    ) -> Result<LocalValueSelector, super::DefaultBodyProjectionError> {
        self.constructor_parameters.get(&parameter).cloned().ok_or(
            super::DefaultBodyProjectionError::UnknownConstructorParameter(parameter.into_raw()),
        )
    }

    pub(super) fn initializing_receiver(
        &self,
    ) -> Result<&scoop_identity::SignatureTypeKey, super::DefaultBodyProjectionError> {
        self.initializing_receiver
            .as_ref()
            .ok_or(super::DefaultBodyProjectionError::MissingInitializingReceiver)
    }
}
