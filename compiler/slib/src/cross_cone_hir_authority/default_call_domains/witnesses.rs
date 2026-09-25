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

            let source = candidates.next().ok_or(Error::InheritedProvider)?;
            if candidates.next().is_some()
                || !signatures::equal_arguments(&source.arguments, template.type_parameters())
            {
                return Err(Error::ProviderMapping);
            }
        }
        let references = template.references();
        self.check_records(references.callables(), Kind::Callable, template, &publisher)?;
        self.check_records(
            references.constructors(),
            Kind::Constructor,
            template,
            &publisher,
        )?;
        self.check_records(references.types(), Kind::Type, template, &publisher)?;
        self.check_records(references.globals(), Kind::Global, template, &publisher)?;
        self.check_records(
            references.singleton_values(),
            Kind::Singleton,
            template,
            &publisher,
        )?;
        self.check_records(references.fields(), Kind::Field, template, &publisher)
    }

    fn check_records<T>(
        &mut self,
        references: &[ExportDefaultReferenceV1<T>],
        kind: Kind,
        template: &ExportDefaultTemplateV1,
        domains: &Domains<'_>,
    ) -> Result<(), Error> {
        for (index, reference) in references.iter().enumerate() {
            let invalid = |reason| Error::Witness {
                kind,
                index,
                reason,
            };
            let witness = reference.witness();

            if witness.owner() != template.key().owner() {
                return Err(invalid("publisher identity mismatch"));
            }
            if !self.equal_domains(witness.direct_call_domain(), &domains.direct) {
                return Err(invalid(
                    "direct call domain differs from the actual source declaration",
                ));
            }
            let slot_matches = match (witness.slot_call_domain(), domains.slot.as_deref()) {
                (None, None) => true,
                (Some(actual), Some(expected)) => self.equal_domains(actual, expected),
                _ => false,
            };
            if !slot_matches {
                return Err(invalid(
                    "slot call domain differs from the actual inherited contract",
                ));
            }
            for call_domain in
                std::iter::once(domains.direct.as_ref()).chain(domains.slot.as_deref())
            {
                if !self
                    .authority
                    .source_domain_is_subset(call_domain, witness.target_domain())?
                {
                    return Err(invalid(
                        "target access does not cover the complete call domain",
                    ));
                }
            }
        }
        Ok(())
    }

    fn equal_domains(&mut self, left: &SourceAccessDomainV1, right: &SourceAccessDomainV1) -> bool {
        left == right
    }
}
