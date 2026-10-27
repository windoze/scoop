use std::collections::{BTreeMap, BTreeSet};

use scoop_manifest::{ConeRelativePath, DependencyCoordinateKey};

use super::*;
use crate::{PmError, PmResult};

impl Lockfile {
    pub fn validate(&self) -> PmResult<()> {
        if self.format != 1 {
            return Err(PmError::new(format!(
                "unsupported Cone.lock format {}",
                self.format
            )));
        }
        let mut keys = BTreeSet::new();
        for resolution in &self.resolutions {
            if !keys.insert((&resolution.root, resolution.purpose, &resolution.target)) {
                return Err(PmError::new(format!(
                    "duplicate lock resolution for {} / {} / {}",
                    resolution.root, resolution.purpose, resolution.target
                )));
            }
            resolution.validate()?;
        }
        Ok(())
    }
}

impl LockedResolution {
    pub fn validate(&self) -> PmResult<()> {
        if ConeRelativePath::new(&self.root)
            .map_err(PmError::new)?
            .as_str()
            != self.root
        {
            return Err(PmError::new(
                "locked root must be a normalized workspace-relative path",
            ));
        }
        if !scoop_toolchain::SOURCE_TARGETS
            .iter()
            .any(|target| target.triple == self.target)
        {
            return Err(PmError::new(format!(
                "unknown lock target {:?}",
                self.target
            )));
        }
        let mut nodes = BTreeMap::new();
        let mut keys = BTreeSet::new();
        for node in &self.nodes {
            let key = DependencyCoordinateKey::from_coordinate(&node.coordinate);
            if !keys.insert(key.clone()) {
                return Err(PmError::new(format!(
                    "lock resolution has more than one node for {key}"
                )));
            }
            if node.coordinate != self.root_coordinate && node.kind != RequestedConeKind::Library {
                return Err(PmError::new(format!(
                    "locked dependency {} must be a library",
                    node.coordinate
                )));
            }
            if !node.test_dependencies.is_empty()
                && (node.coordinate != self.root_coordinate || self.purpose != Purpose::Test)
            {
                return Err(PmError::new(format!(
                    "test dependencies are only allowed on a Test root: {}",
                    node.coordinate
                )));
            }
            node.source.validate(&node.coordinate)?;
            nodes.insert(node.coordinate.to_string(), node);
        }
        let root = self.root_coordinate.to_string();
        if !nodes.contains_key(&root) {
            return Err(PmError::new(format!(
                "lock resolution is missing root {root}"
            )));
        }
        for node in &self.nodes {
            let mut edges = BTreeSet::new();
            for edge in node.dependencies.iter().chain(&node.test_dependencies) {
                let edge = edge.to_string();
                if !nodes.contains_key(&edge) || !edges.insert(edge.clone()) {
                    return Err(PmError::new(format!(
                        "{} has missing or duplicate locked dependency {edge}",
                        node.coordinate
                    )));
                }
            }
            let core = ConeCoordinate::reserved_core();
            if node.coordinate != core && !node.dependencies.contains(&core) {
                return Err(PmError::new(format!(
                    "{} is missing the implicit core edge",
                    node.coordinate
                )));
            }
        }
        // An explicit stack keeps ordinary long dependency chains off the call stack.
        let mut active = BTreeSet::new();
        let mut done = BTreeSet::new();
        let mut stack = vec![(root, false)];
        while let Some((coordinate, leaving)) = stack.pop() {
            if leaving {
                active.remove(&coordinate);
                done.insert(coordinate);
            } else if !done.contains(&coordinate) {
                if !active.insert(coordinate.clone()) {
                    return Err(PmError::new(format!(
                        "cycle in locked dependency graph at {coordinate}"
                    )));
                }
                stack.push((coordinate.clone(), true));
                for edge in nodes[&coordinate]
                    .dependencies
                    .iter()
                    .chain(&nodes[&coordinate].test_dependencies)
                    .rev()
                {
                    stack.push((edge.to_string(), false));
                }
            }
        }
        if done.len() != nodes.len() {
            return Err(PmError::new("lock resolution contains unreachable nodes"));
        }
        Ok(())
    }

    pub(crate) fn canonicalize(&mut self) {
        self.nodes
            .sort_by_cached_key(|node| node.coordinate.to_string());
        for node in &mut self.nodes {
            node.dependencies.sort_by_cached_key(ToString::to_string);
            node.test_dependencies
                .sort_by_cached_key(ToString::to_string);
        }
    }
}

impl LockedSource {
    fn validate(&self, coordinate: &ConeCoordinate) -> PmResult<()> {
        let invalid = |message: &str| {
            PmError::new(format!("invalid locked source for {coordinate}: {message}"))
        };
        match self {
            Self::Path { path } | Self::Artifact { path } => {
                if path.is_empty() || path.contains('\0') {
                    return Err(invalid("empty path or NUL in path"));
                }
            }
            Self::Git {
                git,
                commit,
                subdir,
            } => {
                if git.is_empty()
                    || git.contains('\0')
                    || commit.is_empty()
                    || !commit
                        .bytes()
                        .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
                {
                    return Err(invalid(
                        "Git repository and full hexadecimal commit are required",
                    ));
                }
                if ConeRelativePath::new(subdir)
                    .map_err(PmError::new)?
                    .as_str()
                    != subdir
                {
                    return Err(invalid("Git subdir must be normalized"));
                }
            }
            Self::Registry {
                registry,
                archive,
                sha256,
            } => {
                if archive.is_empty()
                    || sha256.len() != 64
                    || !sha256
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                {
                    return Err(invalid(
                        "registry archive and lowercase SHA-256 are required",
                    ));
                }
                let base = url::Url::parse(registry.as_str())
                    .map_err(|error| invalid(&error.to_string()))?;
                let archive = base
                    .join(archive)
                    .map_err(|error| invalid(&error.to_string()))?;
                if !matches!(archive.scheme(), "http" | "https")
                    || !archive.username().is_empty()
                    || archive.password().is_some()
                    || archive.fragment().is_some()
                {
                    return Err(invalid(
                        "archive must be an HTTP(S) URL without embedded credentials or fragment",
                    ));
                }
            }
            Self::Sysroot { name } => {
                if coordinate.group() != "scoop" || coordinate.name() != name {
                    return Err(invalid("sysroot name must match a scoop group coordinate"));
                }
            }
        }
        Ok(())
    }
}
