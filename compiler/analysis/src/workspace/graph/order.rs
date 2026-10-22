use super::*;

impl ProjectBuilder<'_> {
    pub(super) fn check_records(&self) -> Result<(), LoadError> {
        for (owner, record) in &self.records {
            if let Some(ConeInput::Artifact(provider)) = self.project.cones.get(&record.identity())
            {
                let summary = &provider.summary;
                let fingerprints = summary.semantic_fingerprints();
                if record.coordinate() != summary.cone().coordinate()
                    || record.hir_fingerprint() != fingerprints.hir()
                    || record.mir_fingerprint() != fingerprints.mir()
                    || record.lir_fingerprint() != fingerprints.lir()
                {
                    return Err(LoadError::new(
                        &self.manifest,
                        format!(
                            "dependency metadata for {:?} does not match {:?}",
                            self.project.cones[owner].coordinate(),
                            record.coordinate()
                        ),
                    ));
                }
            }
        }
        Ok(())
    }

    pub(super) fn order(&mut self) -> Result<(), LoadError> {
        let mut completed = BTreeSet::new();
        let mut order = Vec::new();
        for start in self.project.cones.keys().copied() {
            if completed.contains(&start) {
                continue;
            }
            let mut visiting = BTreeSet::from([start]);
            let mut stack = vec![(start, 0)];
            while let Some((identity, position)) = stack.last_mut() {
                let dependencies = &self.project.dependencies[identity];
                if let Some(next) = dependencies.get(*position).copied() {
                    *position += 1;
                    if !self.project.cones.contains_key(&next) {
                        return Err(LoadError::new(
                            &self.manifest,
                            format!("dependency input {next} is unavailable"),
                        ));
                    }
                    if completed.contains(&next) {
                        continue;
                    }
                    if !visiting.insert(next) {
                        return Err(LoadError::new(
                            &self.manifest,
                            format!(
                                "dependency cycle reaches {:?}",
                                self.project.cones[&next].coordinate()
                            ),
                        ));
                    }
                    stack.push((next, 0));
                } else {
                    let (identity, _) = stack.pop().expect("nonempty traversal stack");
                    visiting.remove(&identity);
                    completed.insert(identity);
                    order.push(identity);
                }
            }
        }
        self.project.dependency_order = order;
        Ok(())
    }
}
