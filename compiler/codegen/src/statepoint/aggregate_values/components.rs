use super::*;

pub(super) fn contains_managed(ty: BasicTypeEnum<'_>, space: ManagedAddressSpace) -> bool {
    match ty {
        BasicTypeEnum::PointerType(pointer) => pointer.get_address_space() == space.inkwell(),
        BasicTypeEnum::StructType(structure) => structure
            .get_field_types()
            .into_iter()
            .any(|field| contains_managed(field, space)),
        BasicTypeEnum::ArrayType(array) => contains_managed(array.get_element_type(), space),
        _ => false,
    }
}

pub(super) fn shape(
    ty: BasicTypeEnum<'_>,
    space: ManagedAddressSpace,
) -> Vec<(Vec<u32>, BasicTypeEnum<'_>)> {
    fn visit<'ctx>(
        ty: BasicTypeEnum<'ctx>,
        space: ManagedAddressSpace,
        path: &mut Vec<u32>,
        out: &mut Vec<(Vec<u32>, BasicTypeEnum<'ctx>)>,
    ) {
        if contains_managed(ty, space) {
            match ty {
                BasicTypeEnum::StructType(structure) => {
                    for (index, field) in structure.get_field_types().into_iter().enumerate() {
                        path.push(index as u32);
                        visit(field, space, path, out);
                        path.pop();
                    }
                    return;
                }
                BasicTypeEnum::ArrayType(array) => {
                    for index in 0..array.len() {
                        path.push(index);
                        visit(array.get_element_type(), space, path, out);
                        path.pop();
                    }
                    return;
                }
                _ => {}
            }
        }
        out.push((path.clone(), ty));
    }
    let mut result = Vec::new();
    visit(ty, space, &mut Vec::new(), &mut result);
    result
}

pub(super) fn extract<'ctx>(
    builder: &Builder<'ctx>,
    mut value: BasicValueEnum<'ctx>,
    path: &[u32],
) -> Result<BasicValueEnum<'ctx>, CodegenError> {
    for index in path {
        value = builder
            .build_extract_value(aggregate(value), *index, "gc.part")
            .map_err(|error| CodegenError(format!("extract aggregate component: {error}")))?;
    }
    Ok(value)
}

pub(super) fn insert<'ctx>(
    builder: &Builder<'ctx>,
    value: BasicValueEnum<'ctx>,
    path: &[u32],
    inserted: BasicValueEnum<'ctx>,
) -> Result<BasicValueEnum<'ctx>, CodegenError> {
    let Some((index, remaining)) = path.split_first() else {
        return Ok(inserted);
    };
    let inserted = if remaining.is_empty() {
        inserted
    } else {
        let child = extract(builder, value, &[*index])?;
        insert(builder, child, remaining, inserted)?
    };
    builder
        .build_insert_value(aggregate(value), inserted, *index, "gc.aggregate")
        .map(|value| match value {
            AggregateValueEnum::StructValue(value) => value.into(),
            AggregateValueEnum::ArrayValue(value) => value.into(),
        })
        .map_err(|error| CodegenError(format!("rebuild aggregate component: {error}")))
}
