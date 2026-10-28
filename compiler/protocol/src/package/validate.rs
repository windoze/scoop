use std::collections::BTreeSet;

use super::*;

impl PrepareRequestV1 {
    pub fn validate(&self) -> Result<(), String> {
        version(self.version)?;
        absolute(&self.cwd)?;
        crate::TargetSelectionRequestV1::new(self.target.clone())
            .map_err(|error| error.to_string())?;
        if matches!(&self.members, PackageMemberSelectionV1::Packages { keys } if keys.is_empty()) {
            return Err("package member selection must not be empty".into());
        }
        Ok(())
    }
}

impl PreparedWorkspaceV1 {
    /// Check process-boundary references, without reinterpreting declarations.
    pub fn validate(&self) -> Result<(), String> {
        version(self.version)?;
        absolute(&self.workspace)?;
        absolute(&self.lockfile)?;
        if self.resolutions.is_empty() {
            return Err("prepared workspace has no selected roots".into());
        }
        let mut roots = BTreeSet::new();
        for resolution in &self.resolutions {
            let purpose = match resolution.purpose {
                CompilationPurpose::Production => 0,
                CompilationPurpose::Test => 1,
            };
            if !roots.insert((&resolution.root, purpose, &resolution.target)) {
                return Err("duplicate prepared resolution".into());
            }
            resolution.validate()?;
        }
        Ok(())
    }
}

impl PreparedResolutionV1 {
    fn validate(&self) -> Result<(), String> {
        scoop_manifest::ConeRelativePath::new(&self.root).map_err(str::to_owned)?;
        crate::TargetSelectionRequestV1::new(self.target.clone())
            .map_err(|error| error.to_string())?;
        let mut nodes = BTreeMap::new();
        for node in &self.nodes {
            let coordinate = node.cone.coordinate();
            if nodes
                .insert(DependencyCoordinateKey::from_coordinate(coordinate), node)
                .is_some()
            {
                return Err(format!("duplicate package in prepared graph: {coordinate}"));
            }
        }
        if !nodes
            .values()
            .any(|node| node.cone.coordinate() == &self.coordinate)
        {
            return Err("prepared root coordinate is absent from graph".into());
        }
        for node in &self.nodes {
            let coordinate = node.cone.coordinate();
            let is_test =
                coordinate == &self.coordinate && self.purpose == CompilationPurpose::Test;
            if !is_test && !node.test_dependencies.is_empty() {
                return Err(format!("unexpected test edges for {coordinate}"));
            }
            let mut edges = BTreeSet::new();
            for edge in node.dependencies.iter().chain(&node.test_dependencies) {
                let key = DependencyCoordinateKey::from_coordinate(edge);
                if !edges.insert(key.clone()) {
                    return Err(format!(
                        "duplicate prepared edge from {coordinate} to {edge}"
                    ));
                }
                let target = nodes
                    .get(&key)
                    .ok_or_else(|| format!("missing prepared node {edge}"))?;
                if target.cone.coordinate() != edge
                    || target.cone.kind() != RequestedConeKind::Library
                {
                    return Err(format!(
                        "invalid prepared dependency from {coordinate} to {edge}"
                    ));
                }
            }
            match &node.cone {
                PreparedConeV1::Source { config } => {
                    config.validate()?;
                    let mut expected: Vec<_> = config
                        .manifest
                        .semantic()
                        .dependency_iter()
                        .map(|(_, coordinate)| coordinate.clone())
                        .collect();
                    if coordinate != &ConeCoordinate::reserved_core()
                        && !expected.contains(&ConeCoordinate::reserved_core())
                    {
                        expected.push(ConeCoordinate::reserved_core());
                    }
                    same_edges(&expected, &node.dependencies)?;
                    let test: Vec<_> = config
                        .manifest
                        .tests()
                        .dependency_iter()
                        .map(|(_, dependency)| dependency.coordinate().clone())
                        .collect();
                    same_edges(&test, &node.test_dependencies)?;
                }
                PreparedConeV1::Prebuilt { artifact, summary } => {
                    absolute(artifact)?;
                    if summary
                        .target_selection()
                        .target()
                        .contract()
                        .canonical_triple()
                        != self.target
                    {
                        return Err(format!(
                            "prepared artifact target mismatch for {coordinate}"
                        ));
                    }
                    let expected: Vec<_> = summary
                        .direct_dependencies()
                        .iter()
                        .map(|dependency| dependency.coordinate().clone())
                        .collect();
                    same_edges(&expected, &node.dependencies)?;
                }
            }
        }
        Ok(())
    }
}

impl ExpandedConeInputV1 {
    pub fn validate(&self) -> Result<(), String> {
        absolute(&self.root)?;
        absolute(&self.manifest_path)?;
        absolute(&self.input_root)?;
        if self
            .manifest
            .locators()
            .iter()
            .any(|(_, locator)| !locator.uses_search_roots())
            || self
                .manifest
                .tests()
                .dependency_iter()
                .any(|(_, dependency)| !dependency.locator().uses_search_roots())
        {
            return Err("expanded Cone input cannot contain unresolved source locators".into());
        }
        let spans = self.manifest.diagnostic_spans();
        let fields = spans.cone_fields();
        for span in [
            spans.schema(),
            spans.cone(),
            fields.group(),
            fields.name(),
            fields.version(),
            fields.kind(),
        ] {
            self.span(span)?;
        }
        for (_, span) in spans.dependency_iter().chain(
            self.manifest
                .tests()
                .dependency_iter()
                .map(|(key, dependency)| (key, dependency.spans())),
        ) {
            self.span(span.declaration())?;
            self.span(span.version())?;
        }
        Ok(())
    }

    fn span(&self, span: &scoop_manifest::ManifestSpan) -> Result<(), String> {
        if self.manifest_text.get(span.range()).is_none() {
            return Err("manifest diagnostic span is outside the original UTF-8 text".into());
        }
        Ok(())
    }
}

fn same_edges(expected: &[ConeCoordinate], actual: &[ConeCoordinate]) -> Result<(), String> {
    let keys = |values: &[ConeCoordinate]| {
        values
            .iter()
            .map(ToString::to_string)
            .collect::<BTreeSet<_>>()
    };
    if keys(expected) != keys(actual) {
        return Err("prepared edges do not match the bound Cone input".into());
    }
    Ok(())
}

pub(super) fn version(actual: u32) -> Result<(), String> {
    if actual != PACKAGE_PROTOCOL_VERSION {
        return Err(format!(
            "unsupported package protocol version {actual}; expected {PACKAGE_PROTOCOL_VERSION}"
        ));
    }
    Ok(())
}

pub(super) fn absolute(path: &HostPathCarrier) -> Result<(), String> {
    let path = path.to_path_buf().map_err(|error| error.to_string())?;
    if !path.is_absolute() {
        return Err(format!(
            "prepared host path must be absolute: {}",
            path.display()
        ));
    }
    Ok(())
}
