//! The `scoop` umbrella binary. Argument parsing grows with the build
//! and link subcommands; graph resolution is already wired through the
//! library.

use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("graph") if args.len() == 2 => print_graph(&args[1]),
        _ => {
            eprintln!("usage: scoop graph <Cone-root>");
            std::process::exit(2);
        }
    }
}

fn print_graph(cone_root: &str) {
    let manifest_path = Path::new(cone_root).join("Cone.toml");
    let Ok(text) = std::fs::read_to_string(&manifest_path) else {
        eprintln!("cannot read {}", manifest_path.display());
        std::process::exit(1);
    };
    let manifest = match scoop_manifest::parse_manifest(&text) {
        Ok(manifest) => manifest,
        Err(errors) => {
            for error in &errors {
                eprintln!("{}", error.message);
            }
            std::process::exit(1);
        }
    };
    let mut inputs = FsBuildInputs {
        cone_roots: vec![Path::new(cone_root).to_path_buf()],
    };
    match scoop_build::resolve_graph(manifest, cone_root, &mut inputs) {
        Ok(graph) => {
            for identity in graph.order() {
                let node = graph.node(identity).expect("order covers nodes");
                println!("{} {}", node.coordinate.display(), origin_suffix(node));
            }
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}

fn origin_suffix(node: &scoop_build::ResolvedNode) -> &'static str {
    match node.origin {
        scoop_build::NodeOrigin::Source { .. } => "source",
        scoop_build::NodeOrigin::Prebuilt { .. } => "prebuilt",
        scoop_build::NodeOrigin::Core { .. } => "core",
    }
}

/// Minimal filesystem inputs: relative `path` locators resolve against
/// the root Cone's directory, artifacts against the same tree, and the
/// core slot against the workspace sysroot.
struct FsBuildInputs {
    cone_roots: Vec<std::path::PathBuf>,
}

impl scoop_build::BuildInputs for FsBuildInputs {
    fn read_manifest(
        &mut self,
        path: &str,
    ) -> Result<scoop_manifest::ConeManifest, scoop_build::GraphError> {
        let text = std::fs::read_to_string(path)
            .map_err(|error| scoop_build::GraphError::Io(format!("{path}: {error}")))?;
        scoop_manifest::parse_manifest(&text)
            .map_err(|errors| scoop_build::GraphError::Manifest(format!("{errors:#?}")))
    }

    fn read_artifact(&mut self, path: &str) -> Result<Vec<u8>, scoop_build::GraphError> {
        std::fs::read(path).map_err(|error| scoop_build::GraphError::Io(format!("{path}: {error}")))
    }

    fn search_candidates(
        &mut self,
        coordinate: &scoop_identity::ConeCoordinate,
    ) -> Result<Vec<String>, scoop_build::GraphError> {
        let mut candidates = Vec::new();
        for root in &self.cone_roots {
            let candidate = root
                .join(coordinate.group())
                .join(coordinate.name())
                .join(coordinate.version().as_str())
                .join("cone.slib");
            if candidate.is_file() {
                candidates.push(candidate.display().to_string());
            }
        }
        Ok(candidates)
    }

    fn core_slot(&mut self) -> Result<Vec<u8>, scoop_build::GraphError> {
        let sysroot = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../sysroot");
        let slot = sysroot.join("lib/scoop.core/artifact/cone.slib");
        std::fs::read(&slot).map_err(|error| {
            scoop_build::GraphError::CoreSlot(format!("{}: {error}", slot.display()))
        })
    }
}
