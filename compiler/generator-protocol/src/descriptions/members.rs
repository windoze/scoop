use super::*;

pub fn member_description(
    queries: &mut dyn hir::SemanticQueries,
    function: hir::FunctionId,
    ty: hir::TypeId,
) -> Result<Value> {
    let context = hir::SemanticContext::Function(function);
    let Some(nominal) = queries.nominal_declaration(ty)? else {
        return Ok(Value::Null);
    };
    let fields = queries.nominal_fields(context, ty)?;
    let properties = queries.nominal_properties(context, ty)?.unwrap_or_default();
    let constructors = queries
        .nominal_constructors(context, ty)?
        .unwrap_or_default();
    let methods = queries.nominal_methods(context, ty)?.unwrap_or_default();
    let arguments = nominal
        .arguments
        .iter()
        .map(|&ty| queries.type_signature(context.into(), ty))
        .collect::<Result<Vec<_>>>()?;
    let arguments = hir::CanonicalBinderUseListV1::try_new(arguments)
        .map_err(|cause| error(nominal.origin.span, cause))?;
    let binders = nominal.declaration_binders();
    let references = binders.signature_binders();
    let projection = MemberProjection {
        declaration: SignatureProjection {
            queries,
            context: (&binders).into(),
            source: &nominal.origin.source,
            binders: &references,
            application: None,
        },
        applied: SignatureProjection {
            queries,
            context: (&binders).into(),
            source: &nominal.origin.source,
            binders: &references,
            application: Some((&arguments, 0)),
        },
    };
    let (fields, variants) = match &fields {
        Some(fields) => (
            projection.fields(&fields.fields)?,
            fields
                .variants
                .iter()
                .map(|variant| projection.variant(variant))
                .collect::<Result<Vec<_>>>()?,
        ),
        None => (Vec::new(), Vec::new()),
    };
    Ok(json!({
        "type": QueryJson(&queries.type_signature(context.into(), ty)?),
        "declaration": NominalId::from(nominal.declaration),
        "parameters": nominal_parameters(queries, &nominal)?,
        "fields": fields, "variants": variants,
        "properties": properties.iter().map(|property| projection.property(property)).collect::<Result<Vec<_>>>()?,
        "constructors": constructors.iter().map(|constructor| projection.constructor(constructor)).collect::<Result<Vec<_>>>()?,
        "methods": methods.iter().map(|method| methods::method(queries, method, &arguments)).collect::<Result<Vec<_>>>()?,
    }))
}

impl MemberProjection<'_> {
    fn fields(&self, fields: &[hir::SemanticFieldDescription]) -> Result<Vec<Value>> {
        fields.iter().map(|field| Ok(json!({
            "declaration": FieldId::from(field.identity), "name": field.name,
            "valueType": self.type_use(field.value_type)?, "storage": QueryJson(&field.storage),
            "annotations": self.annotations(field.annotations), "origin": QueryJson(&field.origin),
        }))).collect()
    }

    fn variant(&self, variant: &hir::SemanticVariantDescription) -> Result<Value> {
        Ok(json!({
            "declaration": Reference::from(variant.identity), "name": variant.name,
            "style": QueryJson(&variant.style), "fields": self.fields(&variant.fields)?,
            "annotations": self.annotations(variant.annotations), "origin": QueryJson(&variant.origin),
        }))
    }

    fn property(&self, property: &hir::SemanticPropertyDescription) -> Result<Value> {
        Ok(json!({
            "declaration": Reference::from(property.identity), "name": property.name,
            "valueType": self.type_use(property.value_type)?,
            "visibility": QueryJson(&property.visibility), "lookupDomain": QueryJson(&property.lookup_domain),
            "getter": self.accessor(&property.getter)?,
            "setter": property.setter.as_ref().map(|setter| self.accessor(setter)).transpose()?,
            "overrides": property.overrides.iter().copied().map(Reference::from).collect::<Vec<_>>(),
            "annotations": self.annotations(property.annotations), "origin": QueryJson(&property.origin),
        }))
    }

    fn accessor(&self, accessor: &hir::SemanticAccessorDescription) -> Result<Value> {
        Ok(json!({
            "declaration": Reference::from(accessor.identity), "access": QueryJson(&accessor.access),
            "implementation": QueryJson(&accessor.implementation),
            "body": self.body(CallableDeclaration::PropertyAccessor(accessor.identity.into()), accessor.callable, &accessor.origin)?,
            "annotations": self.annotations(accessor.annotations), "origin": QueryJson(&accessor.origin),
        }))
    }

    fn constructor(&self, constructor: &hir::SemanticConstructorDescription) -> Result<Value> {
        Ok(json!({
            "declaration": CallableDeclaration::from(constructor.identity),
            "constructorKind": QueryJson(&constructor.kind),
            "parameters": self.parameters(&constructor.parameters)?,
            "result": self.type_use(constructor.result)?, "access": QueryJson(&constructor.access),
            "annotations": self.annotations(constructor.annotations), "origin": QueryJson(&constructor.origin),
        }))
    }
}
