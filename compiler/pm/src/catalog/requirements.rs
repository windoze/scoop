use scoop_manifest::package::PackageSource;

use super::*;

impl Catalog<'_> {
    pub(super) fn source_requirements(
        &self,
        member: &WorkspaceMember,
        context: &Context,
        purpose: Purpose,
    ) -> PmResult<Vec<Requirement>> {
        let mut result = member
            .cone
            .dependencies
            .iter()
            .map(|(key, dependency)| self.requirement(key, dependency, context, false))
            .collect::<PmResult<Vec<_>>>()?;
        if purpose == Purpose::Test {
            for (key, dependency) in &member.cone.test_dependencies {
                result.push(self.requirement(key, dependency, context, true)?);
            }
        }
        let core = ConeCoordinate::reserved_core();
        let key = Key::from_coordinate(&core);
        if member.cone.coordinate != core {
            if let Some(edge) = result.iter_mut().find(|edge| edge.key == key) {
                // Core remains a production edge when a Test root selects its source.
                edge.test = false;
                edge.versions
                    .push("=0.1.0".parse().expect("core requirement is valid"));
            } else {
                let mut edge = Requirement {
                    key,
                    versions: vec!["=0.1.0".parse().expect("core requirement is valid")],
                    source: SourceRequest::ImplicitCore,
                    origins: Vec::new(),
                    test: false,
                    artifact: None,
                };
                self.apply_override(&mut edge)?;
                result.push(edge);
            }
        }
        Ok(result)
    }

    fn requirement(
        &self,
        key: &Key,
        dependency: &ExpandedDependency,
        context: &Context,
        test: bool,
    ) -> PmResult<Requirement> {
        let mut requirement = Requirement {
            key: key.clone(),
            versions: dependency.request.requirement.iter().cloned().collect(),
            source: SourceRequest::System,
            origins: vec![dependency.origin.clone()],
            test,
            artifact: None,
        };
        requirement.origins.extend(dependency.inherited_at.clone());
        // Apply replacements before touching the original locator or inferring its version.
        if self.overrides.contains_key(key) {
            self.apply_override(&mut requirement)?;
        } else {
            requirement.source = self.request_source(key, dependency, context)?;
        }
        Ok(requirement)
    }

    pub(super) fn apply_override(&self, requirement: &mut Requirement) -> PmResult<()> {
        if let Some(replacement) = self.overrides.get(&requirement.key) {
            requirement.source =
                self.request_source(&requirement.key, replacement, &Context::Local)?;
            requirement
                .versions
                .extend(replacement.request.requirement.clone());
            requirement.origins.push(replacement.origin.clone());
        }
        Ok(())
    }

    fn request_source(
        &self,
        key: &Key,
        declaration: &ExpandedDependency,
        context: &Context,
    ) -> PmResult<SourceRequest> {
        Ok(match &declaration.request.source {
            PackageSource::Path(path) => {
                let path = lexical(&declaration.base.join(path));
                match context {
                    Context::Git(checkout) => {
                        if !path.starts_with(&checkout.root) { return Err(PmError::new(format!("Git dependency {key} has a path outside its checkout")).at(declaration.origin.clone())); }
                        SourceRequest::GitPath { checkout: checkout.clone(), subdir: relative(&path, &checkout.root)? }
                    },
                    Context::Local => SourceRequest::Path(path),
                }
            },
            PackageSource::Git(source) => SourceRequest::Git { source: source.clone(), base: declaration.base.clone() },
            PackageSource::Registry(registry) => SourceRequest::Registry(registry.clone()),
            PackageSource::Artifact(path) => {
                if !matches!(context, Context::Local) { return Err(PmError::new("remote packages cannot contain local artifact locators").at(declaration.origin.clone())); }
                SourceRequest::Artifact(lexical(&declaration.base.join(path)))
            },
            PackageSource::Default if key.group() == "scoop" => SourceRequest::System,
            PackageSource::Default => SourceRequest::Registry(declaration.registry.clone().or_else(|| {
                if matches!(context, Context::Local) { self.options.default_registry.clone() } else { None }
            }).ok_or_else(|| PmError::new(format!("no default registry for {key}; declare registry.default or an explicit registry URL")).at(declaration.origin.clone()))?),
        })
    }

    pub(super) fn index_requirements(
        &self,
        entries: &[registry::RegistryDependency],
    ) -> PmResult<Vec<Requirement>> {
        entries
            .iter()
            .map(|entry| {
                let source = match &entry.source {
                    registry::RegistrySource::Registry { registry } => {
                        SourceRequest::Registry(registry.clone())
                    }
                    registry::RegistrySource::Git {
                        git,
                        commit,
                        subdir,
                    } => SourceRequest::Git {
                        source: GitSource {
                            repository: git.clone(),
                            reference: GitReference::Revision(commit.clone()),
                            subdir: scoop_manifest::ConeRelativePath::new(subdir)
                                .map_err(PmError::new)?,
                        },
                        base: self.cache.root().to_owned(),
                    },
                    registry::RegistrySource::Sysroot
                        if entry.key == Key::from_coordinate(&ConeCoordinate::reserved_core()) =>
                    {
                        SourceRequest::ImplicitCore
                    }
                    registry::RegistrySource::Sysroot => SourceRequest::System,
                };
                let mut requirement = Requirement {
                    key: entry.key.clone(),
                    versions: vec![entry.version.clone()],
                    source,
                    origins: Vec::new(),
                    test: false,
                    artifact: None,
                };
                self.apply_override(&mut requirement)?;
                Ok(requirement)
            })
            .collect()
    }
}
