use super::*;
use crate::{ProtectedDefaultTemplateV1, TemplateLocalDefinitionV1};

pub(super) fn visit<V: SourceVisitor<E>, E>(
    inputs: TypeDefinitionSourceInputsV1<'_>,
    validator: &mut V,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), TypeDefinitionSourceClosureError<E>> {
    meter.check_table_entries(inputs.source_interfaces.records().len() as u64, path)?;
    for (index, source) in inputs.source_interfaces.records().iter().enumerate() {
        meter.charge_nodes(1, path)?;
        meter.charge_work(1, path)?;
        let at = path.clone().field(5).index(index as u64).field(2);
        meter.check_table_entries(source.parameters().parameters().len() as u64, &at)?;
        for (parameter_index, parameter) in source.parameters().parameters().iter().enumerate() {
            validator.observe(
                parameter.definition_origin(),
                TypeDefinitionSourceUseV1::SourceParameter {
                    source,
                    parameter_index,
                },
                meter,
                &at.clone().index(parameter_index as u64).field(4),
            )?;
        }
    }
    meter.check_table_entries(inputs.defaults.records().len() as u64, path)?;
    for (index, template) in inputs.defaults.records().iter().enumerate() {
        let at = path.clone().field(6).index(index as u64);
        visit_template(template, validator, meter, &at)?;
    }
    Ok(())
}

fn visit_template<V: SourceVisitor<E>, E>(
    template: &ProtectedDefaultTemplateV1,
    validator: &mut V,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), TypeDefinitionSourceClosureError<E>> {
    use TypeDefinitionDefaultOriginSiteV1 as Site;
    let use_at = |site| TypeDefinitionSourceUseV1::Default { template, site };
    validator.observe(
        template.definition_origin(),
        use_at(Site::Root),
        meter,
        &path.clone().field(12),
    )?;
    meter.check_table_entries(template.locals().records().len() as u64, path)?;
    for (index, local) in template.locals().records().iter().enumerate() {
        meter.charge_nodes(1, path)?;
        meter.charge_work(1, path)?;
        if let TemplateLocalDefinitionV1::Source(source) = local.definition() {
            validator.observe(
                source,
                use_at(Site::Local(local)),
                meter,
                &path.clone().field(4).index(index as u64).field(4).field(1),
            )?;
        }
    }
    template.body().visit_definition_sources_metered(
        &mut |origin, site, meter, path| {
            validator.observe(origin, use_at(Site::Body(site)), meter, path)
        },
        meter,
        &path.clone().field(5),
    )?;
    macro_rules! references {
        ($method:ident, $field:literal, $variant:ident) => {{
            let references = template.references().$method();
            meter.check_table_entries(references.len() as u64, path)?;
            for (index, reference) in references.iter().enumerate() {
                validator.observe(
                    reference.definition_origin(),
                    use_at(Site::$variant(reference)),
                    meter,
                    &path
                        .clone()
                        .field(11)
                        .field($field)
                        .index(index as u64)
                        .field(2),
                )?;
            }
        }};
    }
    references!(callables, 1, Callable);
    references!(constructors, 2, Constructor);
    references!(types, 3, Type);
    references!(globals, 4, Global);
    references!(singleton_values, 5, Singleton);
    references!(fields, 6, Field);
    Ok(())
}
