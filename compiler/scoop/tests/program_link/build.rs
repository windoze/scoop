use super::*;

impl Environment {
    pub fn build(
        &self,
        directory: &Path,
        name: &str,
        kind: &str,
        source: &str,
        dependencies: &[(&str, &Path)],
    ) -> PathBuf {
        self.build_with_support(directory, name, kind, source, dependencies, &[])
    }

    #[allow(clippy::too_many_arguments)]
    pub fn build_with_support(
        &self,
        directory: &Path,
        name: &str,
        kind: &str,
        source: &str,
        dependencies: &[(&str, &Path)],
        support: &[&Path],
    ) -> PathBuf {
        let cone = directory.join("sources").join(name);
        std::fs::create_dir_all(cone.join("src")).unwrap();
        let mut manifest = format!(
            "schema = 1\n[cone]\ngroup = \"dev.programlink\"\nname = \"{name}\"\nversion = \"0.1.0\"\nkind = \"{kind}\"\n"
        );
        if !dependencies.is_empty() {
            manifest.push_str("\n[dependencies]\n");
            for (name, _) in dependencies {
                manifest.push_str(&format!("\"dev.programlink:{name}\" = \"0.1.0\"\n"));
            }
        }
        std::fs::write(cone.join("Cone.toml"), manifest).unwrap();
        std::fs::write(cone.join("src/main.scoop"), source).unwrap();
        let artifact = directory.join(format!("{name}.slib"));
        let mut command = Command::new(&self.compiler);
        command
            .arg("build")
            .arg(cone)
            .arg("--direct-slib")
            .arg(&self.core);
        for (_, dependency) in dependencies {
            command.arg("--direct-slib").arg(dependency);
        }
        for artifact in support {
            command.arg("--support-slib").arg(artifact);
        }
        checked(command.arg("--out-slib").arg(&artifact));
        artifact
    }
}
