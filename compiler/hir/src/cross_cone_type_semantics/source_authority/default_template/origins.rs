//! Complete source-origin occurrences, without deduplication or semantic authority.
use super::*;
use crate::{
    DefaultBodyOriginSiteV1, DefaultConstructorRefV1, DefaultFieldRefV1, DefaultSourceReferenceV1,
    ExportDefaultCallableTargetV1, TemplateLocalDefinitionV1, TemplateLocalRecordV1,
};
use scoop_identity::{PersistentObjectValueId, PersistentPropertyId};
use scoop_wire::{WireError, WirePath};

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
    pub fn visit_definition_sources<V, E>(&self, visitor: &mut V, path: &WirePath) -> Result<(), E>
    where
        V: FnMut(
            &ExportDefinitionSourceV1,
            DefaultSourceOriginSiteV1<'_>,

            &WirePath,
        ) -> Result<(), E>,
        E: From<WireError>,
    {
        use DefaultSourceOriginSiteV1 as Site;

        visitor(
            self.definition_origin(),
            Site::Root,
            &path.clone().field(12),
        )?;

        for (index, local) in self.locals().records().iter().enumerate() {
            if let TemplateLocalDefinitionV1::Source(source) = local.definition() {
                visitor(
                    source,
                    Site::Local(local),
                    &path.clone().field(4).index(index as u64).field(4).field(1),
                )?;
            }
        }
        self.body().visit_definition_sources(
            &mut |source, site, path| visitor(source, Site::Body(site), path),
            &path.clone().field(5),
        )?;
        macro_rules! references {
            ($method:ident, $field:literal, $variant:ident) => {{
                let references = self.references().$method();

                for (index, reference) in references.iter().enumerate() {
                    visitor(
                        reference.definition_origin(),
                        Site::$variant(reference),
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
