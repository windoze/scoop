use super::*;

pub(super) fn validate<E>(
    new: &CheckedProtectedDefaultTemplatesV1<'_>,
    public: &CrossConeHirInterfaceSectionV1,

    path: &WirePath,
) -> Result<(), TypeSectionExportValidationError<E>> {
    let old = public.default_templates().records();
    for (index, checked) in new.records().iter().enumerate() {
        let new = checked.template();
        let key = (new.key().owner(), new.key().parameter_position());

        if let Ok(old_index) = old.binary_search_by_key(&key, |record| {
            (record.key().owner(), record.key().parameter_position())
        }) {
            require(equal(
                new,
                &old[old_index],
                &path.clone().index(index as u64),
            )?)?;
        }
    }
    Ok(())
}

pub(super) fn equal(
    new: &ProtectedDefaultTemplateV1,
    old: &ExportDefaultTemplateV1,

    path: &WirePath,
) -> Result<bool, WireError> {
    if new.key().owner() != old.key().owner()
        || new.key().parameter_position() != old.key().parameter_position()
        || new.definition_root() != old.definition_root()
        || new.allows_suspend() != old.allows_suspend()
    {
        return Ok(false);
    }

    if new.definition_path() != old.definition_path()
        || !signature(new.result(), old.result(), &path.clone().field(6))?
        || !signatures(
            new.type_parameters().arguments(),
            old.type_parameters().arguments(),
            &path.clone().field(8),
        )?
    {
        return Ok(false);
    }

    if new.definition_origin() != old.definition_origin() {
        return Ok(false);
    }

    if new.locals() != old.locals() {
        return Ok(false);
    }
    if !optional_signature(
        new.receiver()
            .receiver()
            .map(TemplateReceiverV1::value_type),
        old.receiver()
            .receiver()
            .map(TemplateReceiverV1::value_type),
        &path.clone().field(9),
    )? {
        return Ok(false);
    }

    if new.value_parameters() != old.value_parameters() {
        return Ok(false);
    }

    // Reference records have different authorities and are validated separately.
    Ok(new.body() == old.body())
}

impl ProtectedDefaultTemplateV1 {
    pub(in crate::cross_cone_type_semantics) fn matches_shared_template(
        &self,
        shared: &ExportDefaultTemplateV1,

        path: &WirePath,
    ) -> Result<bool, WireError> {
        equal(self, shared, path)
    }
}
