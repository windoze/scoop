use scoop_identity::StructuralDefinitionSiteRole;
use scoop_wire::{BudgetMeter, WirePath};

use super::*;
use crate::{NominalInterfaceShapeAuthority, compare_default_signature_reference_targets};

impl DefaultTemplateContractViewV1<'_> {
    /// Replays the raw provider contract and complete publishing substitution.
    /// Override uniqueness, body typing and access are checked separately.
    pub fn validate<A, E>(
        &self,
        publisher: &DefaultTemplateDeclarationContractV1<'_>,
        provider: &DefaultTemplateDeclarationContractV1<'_>,
        authority: &mut A,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), DefaultTemplateDeclarationContractError<E>>
    where
        A: NominalInterfaceShapeAuthority<E>,
    {
        use DefaultTemplateDeclarationContractError as Error;
        meter.check_semantic_depth(1, path)?;
        meter.charge_nodes(1, path)?;
        meter.charge_work(1, path)?;
        if publisher.declaration != self.owner || provider.declaration != self.root.declaration() {
            return Err(Error::Declaration);
        }
        if publisher.parameter.position() != self.position
            || provider.parameter.position() != self.position
        {
            return Err(Error::ParameterPosition);
        }
        if publisher.parameter.parameters().len_u32() != provider.parameter.parameters().len_u32() {
            return Err(Error::ParameterArity);
        }
        meter.charge_work(self.definition_path.segments().len() as u64 + 1, path)?;
        if !matches!(self.definition_path.segments(), [segment]
            if segment.site_role() == StructuralDefinitionSiteRole::DefaultValue
                && segment.ordinal() == provider.default_ordinal)
        {
            return Err(Error::DefinitionPath);
        }
        if self.mapping.len_u32() != provider.shape.binder_arity() {
            return Err(Error::MappingArity);
        }
        let direct = publisher.declaration == provider.declaration;
        if direct && publisher.shape != provider.shape {
            return Err(Error::DirectShape);
        }
        let scope = publisher.shape.signature_scope();
        meter.check_table_entries(self.mapping.len_u32() as u64, path)?;
        meter.check_table_entries(provider.parameter.parameters().len_u32() as u64, path)?;
        meter.charge_work(
            self.mapping.len_u32() as u64 + provider.parameter.parameters().len_u32() as u64,
            path,
        )?;
        for (index, argument) in self.mapping.arguments().iter().enumerate() {
            scope
                .validate_signature_semantics_metered(argument, authority, meter, path)
                .map_err(|error| Error::Signature(Box::new(error)))?;
            if direct && provider.shape.identity_binder_at(index as u32).as_ref() != Some(argument)
            {
                return Err(Error::DirectMapping { index });
            }
        }
        for (index, (source, target)) in provider
            .parameter
            .parameters()
            .parameters()
            .iter()
            .zip(publisher.parameter.parameters().parameters())
            .enumerate()
        {
            let mapped = self
                .mapping
                .substitute_provider_type_metered(provider.shape, source.value_type(), meter, path)
                .map_err(Error::Substitution)?;
            if !compare_default_signature_reference_targets(
                &mapped,
                target.value_type(),
                meter,
                path,
            )?
            .is_eq()
            {
                return Err(Error::ParameterType { index });
            }
        }
        self.receiver
            .validate_provider_semantics_metered(
                provider.receiver.as_deref(),
                self.locals,
                meter,
                path,
            )
            .map_err(Error::Receiver)?;
        self.value_parameters
            .validate_provider_prefix_types_metered(
                provider
                    .parameter
                    .prefix()
                    .iter()
                    .map(|parameter| parameter.value_type()),
                self.locals,
                meter,
                path,
            )
            .map_err(Error::Prefix)?;
        if !compare_default_signature_reference_targets(
            self.result,
            provider.parameter.current().value_type(),
            meter,
            path,
        )?
        .is_eq()
        {
            return Err(Error::ResultType);
        }
        if self.allows_suspend != CanonicalBooleanV1::from(provider.execution == Effect::Suspend)
            || publisher.execution != provider.execution
        {
            return Err(Error::SuspendPermission);
        }
        Ok(())
    }
}
