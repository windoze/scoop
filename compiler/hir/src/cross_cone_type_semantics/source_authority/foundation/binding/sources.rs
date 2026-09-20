use super::*;
use scoop_identity::DefinitionOriginSubject;

pub(super) fn validate_all(
    bound: &BoundTypeFoundationSourcesV1<'_>,
    meter: &mut BudgetMeter,
) -> Result<(), TypeFoundationBindingError> {
    use TypeFoundationBindingError as Error;
    let entries = bound.source.entries();
    let mut access_authority = AccessAuthority(bound);
    for (index, source) in entries.sources.records().iter().enumerate() {
        let path = WirePath::root().field(3).index(index as u64);
        let owner = source.owner();
        let key = bound.nominal_key(owner)?;
        NominalRepresentationSupportV1::charge_source_key_resources(key, meter, &path)?;
        let owners = source.access().lexical_owners().len() as u64;
        meter.check_semantic_depth(owners.saturating_add(1), &path)?;
        meter.charge_work(
            owners
                .saturating_add(1)
                .saturating_pow(2)
                .saturating_mul(64),
            &path,
        )?;
        let origin = source.access().definition_origin();
        let subject = match owner {
            SourceNominalId::Concrete(id) => DefinitionOriginSubject::Type(id),
            SourceNominalId::GenericTemplate(id) => DefinitionOriginSubject::GenericType(id),
        };
        meter.charge_work(
            u64::from(
                bound
                    .foundation
                    .as_canonical()
                    .counts()
                    .definition_origins
                    .max(1)
                    .ilog2(),
            ) + 1,
            &path,
        )?;
        if bound
            .foundation
            .definition_origin(subject)
            .map(|record| record.origin())
            != Some(origin.origin())
        {
            return Err(Error::DeclarationOrigin(owner));
        }
        let source_bytes = origin.origin().source().logical_path().as_str().len() as u64;
        meter.charge_work(
            source_bytes.saturating_mul(
                u64::from(entries.definition_sources.sources().len().max(1).ilog2()) + owners + 1,
            ),
            &path,
        )?;
        source
            .access()
            .validate_for_declaration(key, &mut access_authority)
            .map_err(|error| Error::Access {
                owner,
                reason: error.to_string(),
            })?;
    }
    for representation in entries.representations.records() {
        let owner = representation.owner();
        let source = bound.nominal_key(SourceNominalId::Concrete(owner))?;
        if source.declaration_kind() != representation.shape().source_kind() {
            return Err(Error::RepresentationKind(owner));
        }
        if let NominalRepresentationShapeV1::Object { backing_class, .. } = representation.shape() {
            meter.charge_work(128, &WirePath::root().field(4))?;
            if bound.generated_key(*backing_class)?
                != &(GeneratedNominalKey::ObjectBackingClass { object: owner })
            {
                return Err(Error::ObjectBacking(owner));
            }
            let expected = ExactTypeKey::Nominal(*backing_class);
            let exact = PersistentExactTypeId::from_key(&expected)
                .map_err(|error| Error::Identity(error.to_string()))?;
            if bound.exact_type_key(exact)? != &expected {
                return Err(Error::ObjectBacking(owner));
            }
        }
    }
    Ok(())
}

struct AccessAuthority<'b, 'a>(&'b BoundTypeFoundationSourcesV1<'a>);

impl ExportDefinitionSourceSemanticAuthority<TypeFoundationBindingError>
    for AccessAuthority<'_, '_>
{
    fn current_cone(&self) -> ConeIdentity {
        self.0.source.entries().provider
    }

    fn validate_export_definition_source(
        &mut self,
        source: &ExportDefinitionSourceV1,
    ) -> Result<(), TypeFoundationBindingError> {
        if self.0.contains_definition_source(source) {
            Ok(())
        } else {
            Err(TypeFoundationBindingError::MissingDefinitionSource)
        }
    }
}

impl DeclarationAccessSourceSemanticAuthority<TypeFoundationBindingError>
    for AccessAuthority<'_, '_>
{
    fn nominal_declaration_key(
        &self,
        owner: SourceNominalId,
    ) -> Result<&SourceDeclarationKey, TypeFoundationBindingError> {
        self.0.nominal_key(owner)
    }

    fn nominal_definition_source(
        &self,
        owner: SourceNominalId,
    ) -> Result<&ExportDefinitionSourceV1, TypeFoundationBindingError> {
        self.0
            .nominal_source(owner)
            .map(|source| source.access().definition_origin())
    }
}
