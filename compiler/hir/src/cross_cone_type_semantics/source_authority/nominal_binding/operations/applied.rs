use super::*;

pub(in crate::cross_cone_type_semantics::source_authority) struct Applied<'s, 't> {
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
        Self::for_callable(sources, owner_type, 0, &[], meter, path)
    }
    pub fn for_callable(
        sources: &'s BoundNominalSourceContractsV1<'_, '_>,
        owner_type: &'t SignatureTypeKey,
        callable_arity: u32,
        type_arguments: &[SignatureTypeKey],
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
        if type_arguments.len() != callable_arity as usize {
            return Err(Error::CallableArity {
                owner,
                expected: callable_arity,
                actual: type_arguments.len(),
            });
        }
        let binders = DefaultTemplateProviderShapeV1::try_new(arity, callable_arity)
            .map_err(Error::Binders)?;
        meter.check_table_entries(u64::from(binders.binder_arity()), path)?;
        let mut mapping = Vec::new();
        meter.try_reserve_collection_slots(&mut mapping, binders.binder_arity() as usize, path)?;
        for argument in arguments.iter().chain(type_arguments) {
            mapping.push(
                copy_default_signature_type_metered(argument, meter, path)
                    .map_err(Error::transform)?,
            );
        }
        Ok(Self {
            source,
            owner_type,
            mapping: CanonicalBinderUseListV1::try_new(mapping).map_err(Error::Mapping)?,
            binders,
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
        Ok(DefaultAggregateOperationShapeV1::new(
            self.owner_type(meter, path)?,
            self.sequence(fields, meter, path)?,
        ))
    }
    pub fn sequence<'a>(
        &self,
        fields: impl ExactSizeIterator<Item = &'a SignatureTypeKey>,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<Vec<SignatureTypeKey>, Error> {
        meter.check_table_entries(fields.len() as u64, path)?;
        let mut output = Vec::new();
        meter.try_reserve_collection_slots(&mut output, fields.len(), path)?;
        for field in fields {
            output.push(self.field_type(field, meter, path)?);
        }
        Ok(output)
    }
}
