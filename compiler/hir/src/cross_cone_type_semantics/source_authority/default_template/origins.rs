//! Complete source-origin occurrences, without deduplication or semantic authority.
use super::*;
use crate::{
    DefaultBodyOriginSiteV1, DefaultConstructorRefV1, DefaultFieldRefV1, DefaultSourceReferenceV1,
    ExportDefaultCallableTargetV1, TemplateLocalDefinitionV1, TemplateLocalRecordV1,
};
use scoop_identity::{PersistentObjectValueId, PersistentPropertyId};
use scoop_wire::{BudgetMeter, WireError, WirePath};

#[derive(Clone, Copy, Debug)]
pub enum DefaultSourceOriginSiteV1<'a> {
    Root,
    Local(&'a TemplateLocalRecordV1),
    Body(DefaultBodyOriginSiteV1),
    Callable(&'a DefaultSourceReferenceV1<ExportDefaultCallableTargetV1>),
    Constructor(&'a DefaultSourceReferenceV1<DefaultConstructorRefV1>),
    Type(&'a DefaultSourceReferenceV1<SignatureTypeKey>),
    Global(&'a DefaultSourceReferenceV1<PersistentPropertyId>),
    Singleton(&'a DefaultSourceReferenceV1<PersistentObjectValueId>),
    Field(&'a DefaultSourceReferenceV1<DefaultFieldRefV1>),
}

impl DefaultSourceTemplateV1 {
    /// Visits each source occurrence with its typed site and wire path. The body
    /// uses the complete provider-envelope walk, including nested metadata.
    pub fn visit_definition_sources_metered<V, E>(
        &self,
        visitor: &mut V,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), E>
    where
        V: FnMut(
            &ExportDefinitionSourceV1,
            DefaultSourceOriginSiteV1<'_>,
            &mut BudgetMeter,
            &WirePath,
        ) -> Result<(), E>,
        E: From<WireError>,
    {
        use DefaultSourceOriginSiteV1 as Site;
        meter.check_semantic_depth(1, path)?;
        meter.charge_nodes(1, path)?;
        meter.charge_work(1, path)?;
        visitor(
            self.definition_origin(),
            Site::Root,
            meter,
            &path.clone().field(12),
        )?;
        meter.check_table_entries(self.locals().records().len() as u64, path)?;
        for (index, local) in self.locals().records().iter().enumerate() {
            meter.charge_nodes(1, path)?;
            meter.charge_work(1, path)?;
            if let TemplateLocalDefinitionV1::Source(source) = local.definition() {
                visitor(
                    source,
                    Site::Local(local),
                    meter,
                    &path.clone().field(4).index(index as u64).field(4).field(1),
                )?;
            }
        }
        self.body().visit_definition_sources_metered(
            &mut |source, site, meter, path| visitor(source, Site::Body(site), meter, path),
            meter,
            &path.clone().field(5),
        )?;
        macro_rules! references {
            ($method:ident, $field:literal, $variant:ident) => {{
                let references = self.references().$method();
                meter.check_table_entries(references.len() as u64, path)?;
                for (index, reference) in references.iter().enumerate() {
                    meter.charge_nodes(1, path)?;
                    meter.charge_work(1, path)?;
                    visitor(
                        reference.definition_origin(),
                        Site::$variant(reference),
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
}
