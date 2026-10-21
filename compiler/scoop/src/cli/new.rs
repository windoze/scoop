//! Fixed templates for new manifest Cones.
use std::path::{Path, PathBuf};

use clap::{Args, ValueEnum};
use scoop::{BuildFailure, BuildFailurePhase, BuildResult};
use scoop_identity::ConeCoordinate;

mod package;
#[cfg(test)]
mod tests;

#[derive(Args)]
pub(super) struct NewArgs {
    /// Directory to create; it must not exist.
    pub path: PathBuf,
    #[arg(long)]
    pub group: String,
    /// Cone name; defaults to the destination directory name.
    #[arg(long)]
    pub name: Option<String>,
    #[arg(long, value_enum, default_value = "executable")]
    pub kind: Kind,
    #[arg(long, default_value = "0.1.0")]
    pub version: String,
    /// Source package; defaults to the group, with identifier escaping.
    #[arg(long)]
    pub package: Option<String>,
    /// Omit the sample test and its test-only dependency.
    #[arg(long)]
    pub no_tests: bool,
}

#[derive(Clone, Copy, ValueEnum)]
pub(super) enum Kind {
    Executable,
    Library,
}

pub(super) fn execute(args: NewArgs) -> BuildResult<()> {
    let files = template(&args)?;
    if let Some(parent) = args
        .path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent).map_err(|error| io_error(parent, error))?;
    }
    // create_dir rejects every existing destination, including an empty directory.
    std::fs::create_dir(&args.path).map_err(|error| io_error(&args.path, error))?;
    for (relative, contents) in files {
        let path = args.path.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| io_error(parent, error))?;
        }
        std::fs::write(&path, contents).map_err(|error| io_error(&path, error))?;
    }
    eprintln!("created {}", args.path.display());
    Ok(())
}

fn template(args: &NewArgs) -> BuildResult<Vec<(&'static str, String)>> {
    let name = args
        .name
        .as_deref()
        .or_else(|| args.path.file_name()?.to_str())
        .ok_or_else(|| invalid("destination has no UTF-8 directory name; use --name"))?;
    let coordinate = ConeCoordinate::new(&args.group, name, &args.version).map_err(invalid)?;
    let package = package::source(&args.group, args.package.as_deref())?;
    let (kind, source_path, body) = match args.kind {
        Kind::Executable => (
            "executable",
            "src/main.scoop",
            "fun main() {\n    println(greeting())\n}\n\nfun greeting(): String = \"Hello, Scoop!\"\n",
        ),
        Kind::Library => (
            "library",
            "src/lib.scoop",
            "fun greeting(): String = \"Hello, Scoop!\"\n",
        ),
    };
    let mut manifest = format!(
        "schema = 1\n\n[cone]\ngroup = {:?}\nname = {:?}\nversion = {:?}\nkind = {kind:?}\n",
        coordinate.group(),
        coordinate.name(),
        coordinate.version(),
    );
    if !args.no_tests {
        manifest.push_str("\n[test-dependencies]\n\"scoop:scoop.test\" = \"0.1.0\"\n");
    }
    // Use the manifest parser for reserved coordinates and kind constraints too.
    scoop_manifest::parse_cone_manifest(&manifest, &[]).map_err(invalid)?;
    let mut files = vec![
        ("Cone.toml", manifest),
        (source_path, format!("package {package}\n\n{body}")),
    ];
    if !args.no_tests {
        files.push(("tests/basic.scoop", format!(
            "package {package}\n\nimport scoop.test.*\n\n@Test\nprivate class BasicTests {{\n    @Test\n    fun `greeting is ready`() {{\n        assertEquals(\"Hello, Scoop!\", greeting())\n    }}\n}}\n"
        )));
    }
    Ok(files)
}

fn invalid(error: impl std::fmt::Display) -> Box<BuildFailure> {
    BuildFailure::tool("SCOOP_NEW_INPUT", BuildFailurePhase::Request, error)
}

fn io_error(path: &Path, error: std::io::Error) -> Box<BuildFailure> {
    BuildFailure::tool("SCOOP_NEW_IO", BuildFailurePhase::Request, error).at_host(path)
}
