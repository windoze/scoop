use std::{collections::BTreeSet, fs};

use scoop_slib::{ArtifactFile, ArtifactManifestSummaryV1, ConeKind};

use super::*;

impl Catalog<'_> {
    pub(super) fn artifact(&mut self, path: &Path, source: LockedSource) -> PmResult<Candidate> {
        let path = fs::canonicalize(path).map_err(|error| PmError::io(path, error))?;
        let source = match source {
            LockedSource::Artifact { .. } => LockedSource::Artifact {
                path: self.lock_path(&path)?,
            },
            source => source,
        };
        let stamp = FileStamp::read(&path)?;
        let target = self.options.target;
        let target_key = target
            .target()
            .fingerprint()
            .map_err(|error| PmError::new(error.to_string()))?;
        let backend_key = target
            .backend()
            .fingerprint()
            .map_err(|error| PmError::new(error.to_string()))?;
        let extra = [
            target_key.as_array().as_slice(),
            backend_key.as_array().as_slice(),
        ]
        .concat();
        let record = self
            .cache
            .input_record("artifacts-v1", &path, &stamp, &extra)?;
        let summary: ArtifactManifestSummaryV1 = match fs::read(&record) {
            Ok(bytes) => scoop_wire::decode_canonical(&bytes)
                .map_err(|error| PmError::new(error.to_string()))?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let file =
                    ArtifactFile::open(&path, target).map_err(|error| PmError::io(&path, error))?;
                let summary = file
                    .summary()
                    .map_err(|error| PmError::new(format!("{}: {error}", path.display())))?
                    .clone();
                stamp.unchanged(&path)?;
                let bytes = scoop_wire::encode(&summary)
                    .map_err(|error| PmError::new(error.to_string()))?;
                crate::cache::atomic_write(&record, &bytes)?;
                summary
            }
            Err(error) => return Err(PmError::io(&record, error)),
        };
        let dependencies = summary
            .direct_dependencies()
            .iter()
            .map(|edge| {
                let mut requirement = Requirement {
                    key: Key::from_coordinate(edge.coordinate()),
                    versions: vec![
                        format!("={}", edge.coordinate().version())
                            .parse()
                            .expect("artifact coordinate is valid"),
                    ],
                    source: SourceRequest::ExactArtifact(edge.coordinate().clone()),
                    origins: Vec::new(),
                    test: false,
                    artifact: Some(edge.clone()),
                };
                self.apply_override(&mut requirement)?;
                Ok(requirement)
            })
            .collect::<PmResult<Vec<_>>>()?;
        let candidate = Candidate {
            coordinate: summary.cone().coordinate().clone(),
            kind: match summary.cone().kind() {
                ConeKind::Library => RequestedConeKind::Library,
                ConeKind::Executable => RequestedConeKind::Executable,
            },
            source,
            dependencies,
            yanked: false,
        };
        self.inputs.insert(path.clone(), stamp);
        self.packages.insert(
            (candidate.key(), candidate.choice()),
            PackageInput::Prebuilt {
                path,
                summary: Box::new(summary),
            },
        );
        Ok(candidate)
    }

    pub(super) fn search_artifacts(
        &mut self,
        requirement: &Requirement,
    ) -> PmResult<Vec<Candidate>> {
        let exact = match &requirement.source {
            SourceRequest::ExactArtifact(coordinate) => Some(coordinate),
            _ => None,
        };
        let mut paths = BTreeSet::new();
        for root in self.options.artifact_roots.clone() {
            if let Some(coordinate) = exact {
                let path = scoop_toolchain::artifact_candidate_path(&root, coordinate);
                if self.observe_presence(&path)? {
                    paths.insert(path);
                }
            } else {
                let directory = root
                    .join(requirement.key.group())
                    .join(requirement.key.name());
                if !self.observe_presence(&directory)? {
                    continue;
                }
                let entries = match fs::read_dir(&directory) {
                    Ok(entries) => entries,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                    Err(error) => return Err(PmError::io(&directory, error)),
                };
                for entry in entries {
                    let entry = entry.map_err(|error| PmError::io(&directory, error))?;
                    if entry
                        .file_name()
                        .to_str()
                        .and_then(|name| semver::Version::parse(name).ok())
                        .is_some_and(|version| {
                            requirement
                                .versions
                                .iter()
                                .all(|range| range.matches(&version))
                        })
                    {
                        let path = entry.path().join("cone.slib");
                        if self.observe_presence(&path)? {
                            paths.insert(path);
                        }
                    }
                }
            }
        }
        let mut result = Vec::new();
        let mut fingerprints = BTreeMap::new();
        for path in paths {
            let candidate = self.artifact(
                &path,
                LockedSource::Artifact {
                    path: self.lock_path(&path)?,
                },
            )?;
            if candidate.key() != requirement.key
                || exact.is_some_and(|exact| *exact != candidate.coordinate)
            {
                return Err(PmError::new(format!(
                    "artifact at {} declares {}, expected {}",
                    path.display(),
                    candidate.coordinate,
                    requirement.key
                )));
            }
            let PackageInput::Prebuilt { summary, .. } = self.input(&candidate) else {
                unreachable!("artifact loader stores a prebuilt input")
            };
            let fingerprint = summary.artifact_fingerprint();
            match fingerprints.insert(candidate.coordinate.to_string(), fingerprint) {
                Some(existing) if existing != fingerprint => {
                    return Err(PmError::new(format!(
                        "ambiguous artifacts for {} in search roots",
                        candidate.coordinate
                    )));
                }
                Some(_) => {}
                None => result.push(candidate),
            }
        }
        if result.is_empty() && requirement.key.group() == "scoop" {
            let path = scoop_toolchain::target_library_root(
                &self.options.sysroot,
                self.options.target.target(),
            )
            .join(format!("{}.slib", requirement.key.name()));
            result.push(self.artifact(
                &path,
                LockedSource::Sysroot {
                    name: requirement.key.name().into(),
                },
            )?);
        }
        if result.is_empty() {
            return Err(PmError::new(format!(
                "no artifact found for {} in the selected artifact search roots",
                requirement.key
            )));
        }
        Ok(result)
    }
}
