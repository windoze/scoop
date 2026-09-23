use super::*;
use scoop_hir::{ExportDefaultReferenceKindV1 as Kind, ExportDefaultReferenceV1};

impl Query<'_, '_> {
    pub(super) fn validate_template(
        &mut self,
        template: &ExportDefaultTemplateV1,
        path: &WirePath,
    ) -> Result<(), Error> {
        let publisher = self.resolve(template.key().owner(), path)?;
        let original = template.definition_root().declaration();
        let provider = self.resolve(original, path)?;
        if !provider.inherited.is_empty() {
            return Err(Error::OriginalProviderOverride);
        }
        if original != template.key().owner() {
            let mut candidates = publisher
                .inherited
                .iter()
                .filter(|candidate| candidate.source.declaration.declaration() == original);
            self.authority
                .meter
                .charge_work((publisher.inherited.len() as u64).saturating_mul(65), path)?;
            let source = candidates.next().ok_or(Error::InheritedProvider)?;
            if candidates.next().is_some()
                || !signatures::equal_arguments(
                    &source.arguments,
                    template.type_parameters(),
                    self.authority.meter,
                    path,
                )?
            {
                return Err(Error::ProviderMapping);
            }
        }
        let references = template.references();
        self.check_records(
            references.callables(),
            Kind::Callable,
            template,
            &publisher,
            path,
        )?;
        self.check_records(
            references.constructors(),
            Kind::Constructor,
            template,
            &publisher,
            path,
        )?;
        self.check_records(references.types(), Kind::Type, template, &publisher, path)?;
        self.check_records(
            references.globals(),
            Kind::Global,
            template,
            &publisher,
            path,
        )?;
        self.check_records(
            references.singleton_values(),
            Kind::Singleton,
            template,
            &publisher,
            path,
        )?;
        self.check_records(references.fields(), Kind::Field, template, &publisher, path)
    }

    fn check_records<T>(
        &mut self,
        references: &[ExportDefaultReferenceV1<T>],
        kind: Kind,
        template: &ExportDefaultTemplateV1,
        domains: &Domains<'_>,
        path: &WirePath,
    ) -> Result<(), Error> {
        self.authority
            .meter
            .check_table_entries(references.len() as u64, path)?;
        for (index, reference) in references.iter().enumerate() {
            let invalid = |reason| Error::Witness {
                kind,
                index,
                reason,
            };
            let witness = reference.witness();
            self.authority.meter.charge_work(65, path)?;
            if witness.owner() != template.key().owner() {
                return Err(invalid("publisher identity mismatch"));
            }
            if !self.equal_domains(witness.direct_call_domain(), &domains.direct, path)? {
                return Err(invalid(
                    "direct call domain differs from the actual source declaration",
                ));
            }
            match (witness.slot_call_domain(), domains.slot.as_deref()) {
                (None, None) => self.authority.meter.charge_work(1, path)?,
                (Some(actual), Some(expected)) if self.equal_domains(actual, expected, path)? => {
                    self.authority.meter.charge_work(1, path)?
                }
                _ => {
                    return Err(invalid(
                        "slot call domain differs from the actual inherited contract",
                    ));
                }
            }
            for call_domain in
                std::iter::once(domains.direct.as_ref()).chain(domains.slot.as_deref())
            {
                if !self.authority.source_domain_is_subset(
                    call_domain,
                    witness.target_domain(),
                    path,
                )? {
                    return Err(invalid(
                        "target access does not cover the complete call domain",
                    ));
                }
            }
        }
        Ok(())
    }

    fn equal_domains(
        &mut self,
        left: &SourceAccessDomainV1,
        right: &SourceAccessDomainV1,
        path: &WirePath,
    ) -> Result<bool, Error> {
        let cost = scoop_wire::encoded_length(left)
            .and_then(|left| {
                scoop_wire::encoded_length(right).map(|right| left.saturating_add(right))
            })
            .map_err(|e| Error::Encoding(e.to_string()))?;
        self.authority.meter.charge_work(cost, path)?;
        Ok(left == right)
    }
}
