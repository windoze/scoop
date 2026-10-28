use super::*;

/// Unsaved editor manifests supplied to local PM inspection, never to a build.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestOverlaysV1 {
    pub version: u32,
    pub manifests: Vec<ManifestOverlayV1>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_inputs_require_versioned_absolute_unique_manifest_paths() {
        let path = std::env::current_dir().unwrap().join("Cone.toml");
        let mut input = ManifestOverlaysV1 {
            version: PACKAGE_PROTOCOL_VERSION,
            manifests: vec![ManifestOverlayV1 {
                path: HostPathCarrier::from_path(&path).unwrap(),
                text: "schema=1".into(),
            }],
        };
        input.validate().unwrap();
        assert_eq!(
            serde_json::from_slice::<ManifestOverlaysV1>(&serde_json::to_vec(&input).unwrap())
                .unwrap(),
            input
        );
        input.manifests.push(input.manifests[0].clone());
        assert!(input.validate().unwrap_err().contains("duplicate"));
        input.manifests.pop();
        input.version += 1;
        assert!(input.validate().unwrap_err().contains("unsupported"));
        input.version = PACKAGE_PROTOCOL_VERSION;
        input.manifests[0].path =
            HostPathCarrier::from_path(std::path::Path::new("Cone.toml")).unwrap();
        assert!(input.validate().unwrap_err().contains("absolute"));
        input.manifests[0].path =
            HostPathCarrier::from_path(&path.with_file_name("main.scoop")).unwrap();
        assert!(input.validate().unwrap_err().contains("Cone.toml"));
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestOverlayV1 {
    pub path: HostPathCarrier,
    pub text: String,
}

impl ManifestOverlaysV1 {
    pub fn validate(&self) -> Result<(), String> {
        validate::version(self.version)?;
        let mut paths = std::collections::BTreeSet::new();
        for manifest in &self.manifests {
            validate::absolute(&manifest.path)?;
            let path = manifest
                .path
                .to_path_buf()
                .map_err(|error| error.to_string())?;
            if path.file_name().is_none_or(|name| name != "Cone.toml") {
                return Err("manifest overlay path must name Cone.toml".into());
            }
            if !paths.insert(path) {
                return Err("duplicate manifest overlay path".into());
            }
        }
        Ok(())
    }
}
