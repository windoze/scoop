use super::*;

impl ConeLirFoundation {
    pub fn native_cxx(&self) -> bool {
        self.canonical.native_cxx
    }

    pub fn with_native_cxx(mut self, cxx: bool) -> Self {
        Rc::make_mut(&mut self.canonical).native_cxx = cxx;
        self
    }

    pub fn with_native_library_requirements(
        mut self,
        requirements: Vec<crate::CanonicalNativeLibraryRequirementV1>,
    ) -> Self {
        if !requirements.is_empty() {
            let canonical = Rc::make_mut(&mut self.canonical);
            let mut merged = std::mem::take(&mut canonical.native_link_requirements)
                .into_iter()
                .map(|record| (record.id(), record))
                .collect::<std::collections::BTreeMap<_, _>>();
            merged.extend(requirements.into_iter().map(|record| (record.id(), record)));
            canonical.native_link_requirements = merged.into_values().collect();
        }
        self
    }
}
