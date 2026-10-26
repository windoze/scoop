use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::Arc,
};

use scoop_identity::{ConeCoordinate, ConeIdentity, RequestedConeKind};
use scoop_manifest::{DependencyLocator, LoadedConeManifest};

use super::{
    ConeInput, LoadError, ProjectConfiguration, ProjectInput, SourceConeInput, WorkspaceLoader,
};
use crate::AnalysisHost;

mod declarations;
mod order;

struct PendingDependency {
    coordinate: ConeCoordinate,
    locator: DependencyLocator,
    manifest: Arc<LoadedConeManifest>,
    span: std::ops::Range<usize>,
}

pub(super) struct ProjectBuilder<'a> {
    loader: &'a mut WorkspaceLoader,
    host: &'a mut AnalysisHost,
    overlays: &'a BTreeSet<PathBuf>,
    project: ProjectInput,
    manifest: PathBuf,
    explicit: Vec<PendingDependency>,
    unlocated: BTreeMap<ConeIdentity, ConeCoordinate>,
    records: Vec<(ConeIdentity, scoop_slib::DependencyRecord)>,
    errors: Vec<LoadError>,
}

impl<'a> ProjectBuilder<'a> {
    pub(super) fn load(
        loader: &'a mut WorkspaceLoader,
        host: &'a mut AnalysisHost,
        overlays: &'a BTreeSet<PathBuf>,
        configuration: ProjectConfiguration,
    ) -> Result<(ProjectInput, Vec<LoadError>), LoadError> {
        let manifest = loader.files.manifest(host, overlays, &configuration.root)?;
        let root = manifest.identity();
        let path = manifest.manifest_path().to_owned();
        let mut builder = Self {
            loader,
            host,
            overlays,
            project: ProjectInput {
                configuration,
                root,
                cones: BTreeMap::new(),
                artifact_sources: BTreeMap::new(),
                dependencies: BTreeMap::new(),
                dependency_order: Vec::new(),
                complete: false,
            },
            manifest: path,
            explicit: Vec::new(),
            unlocated: BTreeMap::new(),
            records: Vec::new(),
            errors: Vec::new(),
        };
        builder.insert_source(manifest)?;
        if let Err(error) = builder.discover() {
            builder.errors.push(error);
        } else if let Err(error) = builder.check_records().and_then(|()| builder.order()) {
            builder.errors.push(error);
        }
        builder.project.complete = builder.errors.is_empty();
        builder.errors.extend(builder.loader.attach_sources(
            builder.host,
            builder.overlays,
            &mut builder.project,
        ));
        Ok((builder.project, builder.errors))
    }

    fn discover(&mut self) -> Result<(), LoadError> {
        loop {
            while let Some(pending) = self.explicit.pop() {
                let result = self.locate_explicit(&pending);
                if let Err(error) = result {
                    return Err(LoadError::new(
                        pending.manifest.manifest_path(),
                        format!("{}: {}", error.path.display(), error.message),
                    )
                    .with_span(pending.span));
                }
            }
            if let Some((identity, coordinate)) = self.unlocated.pop_first() {
                if self.project.cones.contains_key(&identity) {
                    continue;
                }
                if let Some(artifact) =
                    self.loader
                        .artifacts
                        .find(&coordinate, &self.project.configuration, false)?
                {
                    self.insert(ConeInput::Artifact(artifact))?;
                }
                continue;
            }
            if !self.project.cones.contains_key(&ConeIdentity::CORE) {
                let artifact = self
                    .loader
                    .artifacts
                    .find(
                        &ConeCoordinate::reserved_core(),
                        &self.project.configuration,
                        true,
                    )?
                    .ok_or_else(|| LoadError::new(&self.manifest, "core artifact was not found"))?;
                self.insert(ConeInput::Artifact(artifact))?;
                continue;
            }
            return Ok(());
        }
    }

    fn locate_explicit(&mut self, pending: &PendingDependency) -> Result<(), LoadError> {
        match &pending.locator {
            DependencyLocator::SourcePath(path) => {
                let path = pending.manifest.real_root().join(path.as_path());
                let manifest = self
                    .loader
                    .files
                    .manifest(self.host, self.overlays, &path)?;
                if manifest.coordinate() != &pending.coordinate {
                    return Err(LoadError::new(
                        manifest.manifest_path(),
                        "source dependency coordinate does not match its declaration",
                    ));
                }
                if manifest.parsed().semantic().requested_kind() != RequestedConeKind::Library {
                    return Err(LoadError::new(
                        manifest.manifest_path(),
                        "source dependency must be a library",
                    ));
                }
                self.insert_source(manifest)
            }
            DependencyLocator::ArtifactPath(path) => {
                let path = pending.manifest.real_root().join(path.as_path());
                let artifact = self.loader.artifacts.read(
                    &path,
                    &pending.coordinate,
                    self.project.configuration.target,
                )?;
                self.insert(ConeInput::Artifact(artifact))
            }
            DependencyLocator::SearchRoots => {
                unreachable!("search dependencies use the unlocated queue")
            }
        }
    }

    fn insert_source(&mut self, manifest: Arc<LoadedConeManifest>) -> Result<(), LoadError> {
        if let Some(existing) = self.project.cones.get(&manifest.identity()) {
            if let ConeInput::Source(existing) = existing
                && existing.manifest.real_root() == manifest.real_root()
            {
                return Ok(());
            }
            return Err(LoadError::new(
                manifest.manifest_path(),
                "conflicting source or artifact dependency location",
            ));
        }
        let include_tests = manifest.identity() == self.project.root
            && self.project.configuration.purpose == scoop_protocol::CompilationPurpose::Test;
        let source = match self.loader.files.sources(
            self.host,
            self.overlays,
            manifest.clone(),
            &self.project.configuration,
            include_tests,
        ) {
            Ok(source) => source,
            Err(error) => {
                self.errors.push(error);
                SourceConeInput {
                    manifest,
                    sources: Vec::new(),
                    test_dependencies: Vec::new(),
                }
            }
        };
        self.insert(ConeInput::Source(source))
    }

    fn insert(&mut self, input: ConeInput) -> Result<(), LoadError> {
        let coordinate = input.coordinate();
        let identity = coordinate
            .identity()
            .map_err(|error| LoadError::new(&self.manifest, error))?;
        for other in self.project.cones.values() {
            let previous = other.coordinate();
            if previous.group() == coordinate.group()
                && previous.name() == coordinate.name()
                && previous.version() != coordinate.version()
            {
                return Err(LoadError::new(
                    &self.manifest,
                    format!(
                        "multiple versions of {}:{}: {} and {}",
                        coordinate.group(),
                        coordinate.name(),
                        previous.version(),
                        coordinate.version()
                    ),
                ));
            }
        }
        if let Some(existing) = self.project.cones.get(&identity) {
            if let (ConeInput::Artifact(previous), ConeInput::Artifact(next)) = (existing, &input)
                && previous.summary.artifact_fingerprint() == next.summary.artifact_fingerprint()
            {
                return Ok(());
            }
            return Err(LoadError::new(
                &self.manifest,
                format!("conflicting inputs for {coordinate:?}"),
            ));
        }
        let dependencies = match &input {
            ConeInput::Source(source) => self.source_dependencies(source)?,
            ConeInput::Artifact(artifact) => {
                for dependency in artifact.summary.direct_dependencies() {
                    self.unlocated
                        .insert(dependency.identity(), dependency.coordinate().clone());
                    self.records.push((identity, dependency.clone()));
                }
                artifact
                    .summary
                    .direct_dependencies()
                    .iter()
                    .map(|dependency| dependency.identity())
                    .collect()
            }
        };
        self.project.dependencies.insert(identity, dependencies);
        self.project.cones.insert(identity, input);
        Ok(())
    }
}
