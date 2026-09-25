use super::*;

impl DecodedCanonicalExactTypeFactShapesV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<CanonicalExactTypeFactShapesV1, TypeFactShapeSourceError>
    where
        R: PersistentIdResolver<PersistentExactTypeId, Error = E>
            + PersistentIdResolver<PersistentEnumVariantId, Error = E>,
        E: fmt::Display,
    {
        let path = WirePath::root();

        let mut records = Vec::new();
        scoop_wire::allocation::try_reserve(&mut records, self.records.len(), &path)?;
        let mut previous = None;
        for (index, record) in self.records.into_iter().enumerate() {
            let path = path.clone().index(index as u64);

            let exact = resolver.resolve(record.exact).map_err(reference)?;
            if previous.is_some_and(|previous| previous >= exact) {
                return Err(TypeFactShapeSourceError::NonCanonicalOrder(exact));
            }
            previous = Some(exact);
            let shape = record.shape.resolve_at(resolver, &path.clone().field(2))?;
            records.push(ExactTypeFactShapeRecordV1::new(exact, shape));
        }
        CanonicalExactTypeFactShapesV1::from_ordered(records)
    }
}

impl DecodedExactTypeFactShapeV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<ExactTypeFactShapeV1, TypeFactShapeSourceError>
    where
        R: PersistentIdResolver<PersistentExactTypeId, Error = E>
            + PersistentIdResolver<PersistentEnumVariantId, Error = E>,
        E: fmt::Display,
    {
        self.resolve_at(resolver, &WirePath::root())
    }

    fn resolve_at<R, E>(
        self,
        resolver: &mut R,

        path: &WirePath,
    ) -> Result<ExactTypeFactShapeV1, TypeFactShapeSourceError>
    where
        R: PersistentIdResolver<PersistentExactTypeId, Error = E>
            + PersistentIdResolver<PersistentEnumVariantId, Error = E>,
        E: fmt::Display,
    {
        let value = match self {
            Self::Unit => ExactTypeFactShapeV1::Unit,
            Self::Scalar => ExactTypeFactShapeV1::Scalar,
            Self::Pointer => ExactTypeFactShapeV1::Pointer,
            Self::Reference => ExactTypeFactShapeV1::Reference,
            Self::OrdinaryStruct { fields } => ExactTypeFactShapeV1::OrdinaryStruct {
                fields: resolve_fields(fields, resolver, path)?,
            },
            Self::CLayoutStruct { fields } => ExactTypeFactShapeV1::CLayoutStruct {
                fields: resolve_fields(fields, resolver, path)?,
            },
            Self::Tuple { elements } => ExactTypeFactShapeV1::Tuple {
                elements: resolve_fields(elements, resolver, path)?,
            },
            Self::Enum { variants } => {
                let mut resolved = Vec::new();
                scoop_wire::allocation::try_reserve(&mut resolved, variants.len(), path)?;
                for (index, item) in variants.into_iter().enumerate() {
                    let path = path.clone().field(1).index(index as u64);

                    let variant = resolver.resolve(item.variant).map_err(reference)?;
                    resolved.push(ExactEnumVariantFactsV1 {
                        variant,
                        fields: resolve_fields(item.fields, resolver, &path.clone().field(2))?,
                        gc: item.gc,
                    });
                }
                ExactTypeFactShapeV1::Enum { variants: resolved }
            }
        };
        validate_shape(&value)?;
        Ok(value)
    }
}

fn resolve_fields<R>(
    fields: Vec<DecodedPersistentId<PersistentExactTypeId>>,
    resolver: &mut R,

    path: &WirePath,
) -> Result<Vec<PersistentExactTypeId>, TypeFactShapeSourceError>
where
    R: PersistentIdResolver<PersistentExactTypeId>,
    R::Error: fmt::Display,
{
    let mut resolved = Vec::new();
    scoop_wire::allocation::try_reserve(&mut resolved, fields.len(), path)?;
    for field in fields {
        resolved.push(resolver.resolve(field).map_err(reference)?);
    }
    Ok(resolved)
}

fn reference(error: impl fmt::Display) -> TypeFactShapeSourceError {
    TypeFactShapeSourceError::Reference(error.to_string())
}
