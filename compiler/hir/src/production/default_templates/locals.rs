//! Canonical local table and request-local lookup relation for one template.

use std::collections::HashMap;

use scoop_identity::LocalValueSelector;

use super::{entities::DefaultEntityProjector, errors::DefaultTemplateEnvelopeProjectionError};
use crate::{
    CanonicalBooleanV1, CanonicalTemplateLocalTableV1, ExportDefaultExpr, HirSignatureBinder,
    LocalId, LocalValueDefinitionSite, TemplateLocalDefinitionV1, TemplateLocalRecordV1,
};

pub(super) struct TemplateLocalProjection {
    selectors: Vec<LocalValueSelector>,
    bindings: HashMap<crate::BindingId, LocalValueSelector>,
}

impl TemplateLocalProjection {
    pub(super) fn project(
        entities: &DefaultEntityProjector<'_, '_, '_>,
        template: &ExportDefaultExpr,
        binders: &[HirSignatureBinder],
    ) -> Result<(Self, CanonicalTemplateLocalTableV1), DefaultTemplateEnvelopeProjectionError> {
        let resources = &entities.resources;
        resources.collection::<LocalValueSelector>(template.locals.len())?;
        resources.collection::<(crate::BindingId, LocalValueSelector)>(template.locals.len())?;
        resources.collection::<TemplateLocalRecordV1>(template.locals.len())?;
        resources.sort(template.locals.len())?;
        let mut selectors = Vec::with_capacity(template.locals.len());
        let mut bindings = HashMap::with_capacity(template.locals.len());
        let mut records = Vec::with_capacity(template.locals.len());
        for (local_id, local) in template.locals.iter() {
            // Include the lookup, binding and output copies, plus an error copy.
            for _ in 0..4 {
                resources.selector(&local.selector)?;
            }
            let comparisons = u64::from(template.locals.len().max(1).ilog2()) + 1;
            resources.with_meter(|meter, _| {
                let length = match &local.selector {
                    LocalValueSelector::This | LocalValueSelector::Parameter { .. } => 1,
                    LocalValueSelector::LocalDeclaration { path }
                    | LocalValueSelector::BoundReceiver { path }
                    | LocalValueSelector::Synthetic { path, .. } => path.segments().len(),
                    LocalValueSelector::SuspensionResult { site } => site.segments().len(),
                };
                meter.charge_work(
                    (length as u64)
                        .saturating_mul(comparisons)
                        .saturating_mul(4),
                    &scoop_wire::WirePath::root(),
                )
            })?;
            let selector = local.selector.clone();
            if bindings.insert(local.binding, selector.clone()).is_some() {
                return Err(
                    DefaultTemplateEnvelopeProjectionError::DuplicateLocalBinding(
                        local.binding.into_raw(),
                    ),
                );
            }
            let definition = match local.definition {
                LocalValueDefinitionSite::Source(origin) => {
                    entities.charge_origin(origin)?;
                    TemplateLocalDefinitionV1::Source(
                        super::super::definition_sources::project_definition_source(
                            entities.export(),
                            origin,
                        )
                        .map_err(DefaultTemplateEnvelopeProjectionError::DefinitionOrigin)?,
                    )
                }
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
            },
            table,
        ))
    }

    pub(super) fn selector(
        &self,
        local: LocalId,
        resources: &super::resources::ProjectionResources<'_>,
    ) -> Result<LocalValueSelector, super::DefaultBodyProjectionError> {
        let selector = self.selectors.get(super::raw_index(local) as usize).ok_or(
            super::DefaultBodyProjectionError::UnknownLocal(super::raw_index(local)),
        )?;
        resources.selector(selector)?;
        Ok(selector.clone())
    }

    pub(super) fn binding_selector(
        &self,
        binding: crate::BindingId,
        resources: &super::resources::ProjectionResources<'_>,
    ) -> Result<LocalValueSelector, super::DefaultBodyProjectionError> {
        let selector = self.bindings.get(&binding).ok_or(
            super::DefaultBodyProjectionError::UnknownBinding(binding.into_raw()),
        )?;
        resources.selector(selector)?;
        Ok(selector.clone())
    }
}
