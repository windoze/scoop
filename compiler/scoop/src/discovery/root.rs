use super::*;

#[derive(Debug)]
pub struct LoadedBuildRoot {
    pub(super) root: LoadedRootInput,
    pub(super) context: BuildContext,
    pub(super) meter: SlibClosureDecodeMeterV1,
}

#[derive(Debug)]
pub(super) enum LoadedRootInput {
    Manifest(Box<LoadedConeManifest>),
    SingleFile(SingleFileLocator),
}

impl BuildGraphRequest {
    /// Loads only the selected root operand.
    /// No dependency locator or current-Cone source tree is traversed here.
    pub fn load_root(self) -> Result<LoadedBuildRoot, LoadBuildRootError> {
        let BuildGraphRequestParts {
            root,
            artifact_search_roots,
            cache_root,
            sysroot,
            target,
            compiler,
            diagnostics,
            limits,
        } = self.into_parts();
        let mut meter = SlibClosureDecodeMeterV1::new(limits.slib_closure_limits());
        let search_root_count = u64::try_from(artifact_search_roots.len()).map_err(|_| {
            LoadBuildRootError::Resource(SlibClosureResourceErrorV1::Overflow {
                resource: scoop_slib::SlibClosureResourceKindV1::ArtifactSearchRoots,
            })
        })?;
        meter
            .charge_search_roots(search_root_count)
            .map_err(LoadBuildRootError::Resource)?;

        let root = match root.into_kind() {
            BuildRootInputKind::ManifestCone(locator) => LoadedRootInput::Manifest(Box::new(
                load_cone_manifest(&locator).map_err(LoadBuildRootError::RootManifest)?,
            )),
            BuildRootInputKind::SingleFile(locator) => LoadedRootInput::SingleFile(locator),
        };
        Ok(LoadedBuildRoot {
            root,
            context: BuildContext {
                artifact_search_roots,
                cache_root,
                sysroot,
                target,
                compiler,
                diagnostics,
                limits,
            },
            meter,
        })
    }
}

impl LoadedBuildRoot {
    pub fn root_identity(&self) -> ConeIdentity {
        match &self.root {
            LoadedRootInput::Manifest(manifest) => manifest.identity(),
            LoadedRootInput::SingleFile(_) => ConeIdentity::SINGLE_FILE,
        }
    }

    /// Discovers exact source and prebuilt claims without constructing any
    /// Compile or Link artifact authority.
    pub fn discover(self) -> Result<DiscoveredBuildGraph, BuildGraphDiscoveryError> {
        DiscoveryBuilder::new(self)?.run()
    }
}
