use super::*;

impl Catalog<'_> {
    pub(super) fn git(
        &mut self,
        requirement: &Requirement,
        source: &GitSource,
        base: &Path,
    ) -> PmResult<Candidate> {
        let repository = git::repository_url(&source.repository, base)?;
        let reference = reference_key(&source.reference);
        let pinned = self
            .policy
            .locked
            .get(&requirement.key)
            .and_then(|node| match &node.source {
                LockedSource::Git {
                    git,
                    commit,
                    subdir,
                    references,
                } if *git == repository
                    && subdir == source.subdir.as_str()
                    && references.contains(&reference) =>
                {
                    Some(commit.clone())
                }
                _ => None,
            });
        if self.policy.strict && pinned.is_none() {
            return Err(PmError::new(format!(
                "Cone.lock does not record {reference} for {}; resolve without --locked",
                requirement.key
            )));
        }
        let precise = self
            .policy
            .precise
            .as_ref()
            .filter(|(key, _)| *key == requirement.key)
            .map(|(_, value)| value.clone());
        if let Some(commit) = &precise
            && (!matches!(commit.len(), 40 | 64)
                || !commit
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
        {
            return Err(PmError::new(
                "Git --precise requires a full lowercase commit ID",
            ));
        }
        let pinned = precise.or_else(|| {
            if self.policy.updating(&requirement.key) {
                None
            } else {
                pinned
            }
        });
        let ref_key = (
            repository.clone(),
            pinned
                .as_ref()
                .map(|commit| format!("commit:{commit}"))
                .unwrap_or(reference),
        );
        let checkout = match self.git_refs.get(&ref_key) {
            Some(checkout) => checkout.clone(),
            None => {
                let checkout = git::checkout(
                    self.cache,
                    source,
                    base,
                    pinned.as_deref(),
                    self.options.offline,
                )?;
                self.git_refs.insert(ref_key, checkout.clone());
                checkout
            }
        };
        self.local(
            &checkout.root.join(source.subdir.as_path()),
            &Context::Git(checkout.clone()),
        )
    }

    pub(super) fn registry(
        &mut self,
        requirement: &Requirement,
        registry: &RegistryUrl,
    ) -> PmResult<Vec<Candidate>> {
        let locked_source = self.policy.locked.get(&requirement.key).is_some_and(|node| {
            matches!(&node.source, LockedSource::Registry { registry: locked, .. } if locked == registry)
        });
        if self.policy.strict && !locked_source {
            return Err(PmError::new(format!(
                "Cone.lock does not record registry {registry} for {}; resolve without --locked",
                requirement.key
            )));
        }
        let key = (registry.to_string(), requirement.key.clone());
        if !self.indexes.contains_key(&key) {
            let cache_path = self
                .cache
                .registry(registry)
                .join("index")
                .join(requirement.key.group())
                .join(format!("{}.json", requirement.key.name()));
            let was_cached = cache_path
                .try_exists()
                .map_err(|error| PmError::io(&cache_path, error))?;
            let index = registry::load_index(
                &self.http,
                registry,
                &requirement.key,
                self.options.offline,
                self.policy.updating(&requirement.key) || !locked_source,
            )?;
            if was_cached
                && locked_source
                && !self.options.offline
                && !self.policy.updating(&requirement.key)
            {
                self.refreshable_indexes.insert(requirement.key.clone());
            }
            self.indexes.insert(key.clone(), index);
        }
        let mut result = Vec::new();
        for version in &self.indexes[&key].versions {
            let coordinate = ConeCoordinate::new(
                requirement.key.group(),
                requirement.key.name(),
                &version.version,
            )
            .map_err(|error| PmError::new(error.to_string()))?;
            let candidate = Candidate {
                coordinate,
                kind: version.kind,
                source: LockedSource::Registry {
                    registry: registry.clone(),
                    archive: version.archive.clone(),
                    sha256: version.sha256.clone(),
                },
                dependencies: self.index_requirements(&version.dependencies)?,
                yanked: version.yanked,
            };
            self.versions
                .insert((candidate.key(), candidate.choice()), version.clone());
            result.push(candidate);
        }
        Ok(result)
    }
}
