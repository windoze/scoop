use super::*;

impl ProjectBuilder<'_> {
    pub(super) fn source_dependencies(
        &mut self,
        source: &SourceConeInput,
    ) -> Result<Vec<ConeIdentity>, LoadError> {
        let manifest = &source.manifest;
        let mut dependencies = Vec::new();
        for (key, coordinate) in manifest.parsed().semantic().dependency_iter() {
            let locator = manifest
                .parsed()
                .locators()
                .get(key)
                .expect("parsed declarations retain their locator");
            let span = manifest
                .parsed()
                .diagnostic_spans()
                .dependency(key)
                .expect("parsed declarations retain their span")
                .declaration()
                .range();
            self.declare(manifest, coordinate, locator, span, &mut dependencies)?;
        }
        if manifest.identity() == self.project.root
            && self.project.configuration.purpose == scoop_protocol::CompilationPurpose::Test
        {
            for (_, declaration) in manifest.parsed().tests().dependency_iter() {
                self.declare(
                    manifest,
                    declaration.coordinate(),
                    declaration.locator(),
                    declaration.spans().declaration().range(),
                    &mut dependencies,
                )?;
            }
        }
        if manifest.identity() != ConeIdentity::CORE && !dependencies.contains(&ConeIdentity::CORE)
        {
            dependencies.push(ConeIdentity::CORE);
        }
        dependencies.sort();
        dependencies.dedup();
        Ok(dependencies)
    }

    fn declare(
        &mut self,
        manifest: &Arc<LoadedConeManifest>,
        coordinate: &ConeCoordinate,
        locator: &DependencyLocator,
        span: std::ops::Range<usize>,
        dependencies: &mut Vec<ConeIdentity>,
    ) -> Result<(), LoadError> {
        let identity = coordinate.identity().map_err(|error| {
            LoadError::new(manifest.manifest_path(), error).with_span(span.clone())
        })?;
        dependencies.push(identity);
        if locator.uses_search_roots() {
            self.unlocated.insert(identity, coordinate.clone());
        } else {
            self.explicit.push(PendingDependency {
                coordinate: coordinate.clone(),
                locator: locator.clone(),
                manifest: manifest.clone(),
                span,
            });
        }
        Ok(())
    }
}
