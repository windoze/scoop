use super::*;

pub(super) struct Applied<'s, 't> {
    pub source: &'s NominalSourceContractV1,
    owner_type: &'t SignatureTypeKey,
    mapping: CanonicalBinderUseListV1,
    binders: DefaultTemplateProviderShapeV1,
}
impl<'s, 't> Applied<'s, 't> {
    pub fn new(
        sources: &'s BoundNominalSourceContractsV1<'_, '_>,
        owner_type: &'t SignatureTypeKey,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<Self, Error> {
        let (owner, arguments) = match owner_type {
            SignatureTypeKey::Nominal(id) => (SourceNominalId::Concrete(*id), &[][..]),
            SignatureTypeKey::NominalApplication { origin, arguments } => (
                SourceNominalId::GenericTemplate(*origin),
                arguments.as_slice(),
            ),
            _ => return Err(Error::NonNominalOwner),
        };
        query(sources.table.records().len(), meter, path)?;
        let source = sources.nominal_source(owner)?;
        let arity = source.type_parameters().len_u32();
        if arguments.len() != arity as usize {
            return Err(Error::Arity {
                owner,
                expected: arity,
                actual: arguments.len(),
            });
        }
        let mut mapping = Vec::new();
        meter.try_reserve_collection_slots(&mut mapping, arguments.len(), path)?;
        for argument in arguments {
            mapping.push(
                copy_default_signature_type_metered(argument, meter, path)
                    .map_err(Error::transform)?,
            );
        }
        Ok(Self {
            source,
            owner_type,
            mapping: CanonicalBinderUseListV1::try_new(mapping).map_err(Error::Mapping)?,
            binders: DefaultTemplateProviderShapeV1::try_new(arity, 0).map_err(Error::Binders)?,
        })
    }
    pub fn require_kind(&self, expected: PublicNominalKindV1) -> Result<(), Error> {
        if self.source.kind() != expected {
            return Err(Error::Kind {
                owner: self.source.owner(),
                expected,
                actual: self.source.kind(),
            });
        }
        Ok(())
    }
    pub fn struct_shape(&self) -> Result<&StructSourceShapeV1, Error> {
        self.require_kind(PublicNominalKindV1::Struct)?;
        match self.source.source_shape() {
            NominalSourceShapeV1::Struct(shape) => Ok(shape),
            _ => Err(Error::StructRepresentation(self.source.owner())),
        }
    }
    pub fn owner_type(
        &self,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<SignatureTypeKey, Error> {
        copy_default_signature_type_metered(self.owner_type, meter, path).map_err(Error::transform)
    }
    pub fn field_type(
        &self,
        value: &SignatureTypeKey,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<SignatureTypeKey, Error> {
        self.mapping
            .substitute_provider_type_metered(self.binders, value, meter, path)
            .map_err(Error::transform)
    }
    pub fn aggregate<'a>(
        &self,
        fields: impl ExactSizeIterator<Item = &'a SignatureTypeKey>,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<DefaultAggregateOperationShapeV1, Error> {
        meter.check_table_entries(fields.len() as u64, path)?;
        let mut output = Vec::new();
        meter.try_reserve_collection_slots(&mut output, fields.len(), path)?;
        for field in fields {
            output.push(self.field_type(field, meter, path)?);
        }
        Ok(DefaultAggregateOperationShapeV1::new(
            self.owner_type(meter, path)?,
            output,
        ))
    }
}
