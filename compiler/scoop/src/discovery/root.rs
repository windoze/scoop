use super::*;

#[derive(Debug)]
pub struct LoadedBuildRoot {
    pub(super) root: LoadedRootInput,
    pub(super) context: BuildContext,
}

#[derive(Debug)]
pub(super) enum LoadedRootInput {
    Prepared {
        identity: ConeIdentity,
        resolution: Box<scoop_protocol::package::PreparedResolutionV1>,
    },
    SingleFile {
        source: SingleFileLocator,
        dependencies: Vec<scoop_manifest::SingleFileDependency>,
    },
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
            optimization,
            debug_info,
            purpose,
            jobs,
            generators,
        } = self.into_parts();

        LoadedBuildRoot::from_input(
            root,
            BuildContext {
                artifact_search_roots,
                cache_root,
                sysroot,
                target: GraphTarget::Artifact(Box::new(target)),
                compiler,
                diagnostics,
                optimization,
                debug_info,
                purpose,
                jobs,
                generators,
            },
        )
    }
}

impl LoadedBuildRoot {
    pub(crate) fn from_input(
        root: crate::BuildRootInput,
        context: BuildContext,
    ) -> Result<Self, LoadBuildRootError> {
        let root = match root.into_kind() {
            BuildRootInputKind::Prepared {
                identity,
                resolution,
            } => LoadedRootInput::Prepared {
                identity,
                resolution,
            },
            BuildRootInputKind::SingleFile(source) => {
                let dependencies = context
                    .cache_root
                    .load_single_file_header(&source)
                    .map_err(LoadBuildRootError::SingleFile)?;
                LoadedRootInput::SingleFile {
                    source,
                    dependencies,
                }
            }
        };
        Ok(Self { root, context })
    }

    pub fn root_identity(&self) -> ConeIdentity {
        match &self.root {
            LoadedRootInput::Prepared { identity, .. } => *identity,
            LoadedRootInput::SingleFile { .. } => ConeIdentity::SINGLE_FILE,
        }
    }

    /// Discovers exact source and prebuilt claims without constructing any
    /// Compile or Link artifact authority.
    pub fn discover(self) -> Result<DiscoveredBuildGraph, BuildGraphDiscoveryError> {
        match self.root {
            LoadedRootInput::Prepared {
                identity,
                resolution,
            } => DiscoveredBuildGraph::from_package(identity, *resolution, self.context),
            LoadedRootInput::SingleFile {
                source,
                dependencies,
            } => DiscoveryBuilder::new(source, dependencies, self.context)?.run(),
        }
    }
}
