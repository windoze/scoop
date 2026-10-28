//! Source observations carried by IR transformations, independent of host paths.

use std::collections::HashSet;
use std::sync::Arc;

use scoop_identity::SourceIdentity;

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub enum DebugOrigin {
    #[default]
    Generated,
    Source(Arc<DebugLocation>),
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct DebugLocation {
    pub source: Arc<SourceIdentity>,
    pub line: u32,
    pub column: u32,
    pub inlined_at: Option<Arc<DebugInlineSite>>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct DebugInlineSite {
    pub function: Arc<str>,
    pub definition: DebugOrigin,
    pub call_site: DebugOrigin,
}

/// Interned source facts. IR nodes retain shared records after the builder is
/// dropped; transformations never need the complete earlier-stage IR.
#[derive(Debug, Default)]
pub struct DebugRecords {
    files: HashSet<Arc<SourceIdentity>>,
    names: HashSet<Arc<str>>,
    locations: HashSet<Arc<DebugLocation>>,
    inline_sites: HashSet<Arc<DebugInlineSite>>,
}

impl DebugRecords {
    pub fn source(&mut self, source: Arc<SourceIdentity>, line: u32, column: u32) -> DebugOrigin {
        let source = match self.files.get(source.as_ref()) {
            Some(source) => source.clone(),
            None => {
                self.files.insert(source.clone());
                source
            }
        };
        self.location(DebugLocation {
            source,
            line,
            column,
            inlined_at: None,
        })
    }

    pub fn inline_site(
        &mut self,
        function: &str,
        definition: DebugOrigin,
        call_site: DebugOrigin,
    ) -> Arc<DebugInlineSite> {
        let function = match self.names.get(function) {
            Some(name) => name.clone(),
            None => {
                let name: Arc<str> = function.into();
                self.names.insert(name.clone());
                name
            }
        };
        self.intern_inline_site(DebugInlineSite {
            function,
            definition,
            call_site,
        })
    }

    pub fn inlined(&mut self, origin: &DebugOrigin, site: &Arc<DebugInlineSite>) -> DebugOrigin {
        match origin {
            DebugOrigin::Generated => DebugOrigin::Generated,
            DebugOrigin::Source(location) => {
                let mut location = (**location).clone();
                location.inlined_at = Some(match &location.inlined_at {
                    Some(inner) => {
                        let call_site = self.inlined(&inner.call_site, site);
                        self.intern_inline_site(DebugInlineSite {
                            function: inner.function.clone(),
                            definition: inner.definition.clone(),
                            call_site,
                        })
                    }
                    None => site.clone(),
                });
                self.location(location)
            }
        }
    }

    fn location(&mut self, value: DebugLocation) -> DebugOrigin {
        let location = match self.locations.get(&value) {
            Some(location) => location.clone(),
            None => {
                let location = Arc::new(value);
                self.locations.insert(location.clone());
                location
            }
        };
        DebugOrigin::Source(location)
    }

    fn intern_inline_site(&mut self, value: DebugInlineSite) -> Arc<DebugInlineSite> {
        match self.inline_sites.get(&value) {
            Some(site) => site.clone(),
            None => {
                let site = Arc::new(value);
                self.inline_sites.insert(site.clone());
                site
            }
        }
    }
}
