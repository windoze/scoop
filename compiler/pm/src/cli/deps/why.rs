use std::collections::{BTreeMap, VecDeque};

use scoop_identity::ConeCoordinate;
use scoop_manifest::package::format;
use scoop_pm::{
    lockfile::LockedSource,
    resolve::{ResolveRequest, ResolvedRoot},
};
use serde::Serialize;

use super::*;

#[derive(Serialize)]
struct Explanation {
    #[serde(with = "format::coordinate")]
    root: ConeCoordinate,
    purpose: Purpose,
    target: String,
    #[serde(with = "format::coordinate")]
    selected: ConeCoordinate,
    source: LockedSource,
    requirements: Vec<Requirement>,
}

#[derive(Serialize)]
struct Requirement {
    #[serde(with = "format::coordinate")]
    from: ConeCoordinate,
    #[serde(with = "format::coordinates")]
    path: Vec<ConeCoordinate>,
    versions: Vec<String>,
    source: String,
    test: bool,
    origins: Vec<Origin>,
}

#[derive(Serialize)]
struct Origin {
    path: HostPathCarrier,
    span: std::ops::Range<usize>,
}

pub(super) fn execute(args: DepsArgs, key: DependencyCoordinateKey) -> PmResult<()> {
    let cwd = std::env::current_dir().map_err(|error| PmError::new(error.to_string()))?;
    let request = ResolveRequest {
        cwd,
        input: args.members.input,
        members: if args.members.workspace {
            MemberSelection::All
        } else if args.members.package.is_empty() {
            MemberSelection::Default
        } else {
            MemberSelection::Packages(args.members.package)
        },
        purpose: args.purpose,
        target: args
            .target
            .map(Ok)
            .unwrap_or_else(|| scoop_toolchain::host_target_triple().map(str::to_owned))
            .map_err(|error| PmError::new(error.to_string()))?,
        sysroot: scoop_toolchain::configured_sysroot_root(args.sysroot)
            .map_err(|error| PmError::new(error.to_string()))?,
        artifact_roots: args.cone_path,
        package_cache: args.package_cache_dir,
        lockfile: args.lockfile,
        locked: true,
        offline: true,
        update: None,
        precise: None,
    };
    let result = scoop_pm::resolve::inspect(request)?;
    let resolutions = result
        .roots
        .iter()
        .filter(|root| root.solution.packages.contains_key(&key))
        .map(|root| explain(root, &key))
        .collect::<PmResult<Vec<_>>>()?;
    if resolutions.is_empty() {
        return Err(PmError::new(format!(
            "{key} is not in any selected locked graph"
        )));
    }
    if args.members.json {
        #[derive(Serialize)]
        struct Output {
            format: u32,
            resolutions: Vec<Explanation>,
        }
        println!(
            "{}",
            serde_json::to_string_pretty(&Output {
                format: 1,
                resolutions
            })
            .map_err(|error| PmError::new(error.to_string()))?
        );
    } else {
        for explanation in resolutions {
            print_explanation(&explanation)?;
        }
    }
    Ok(())
}

fn explain(root: &ResolvedRoot, key: &DependencyCoordinateKey) -> PmResult<Explanation> {
    let solution = &root.solution;
    let start = solution.packages[&solution.root].coordinate.clone();
    let mut paths = BTreeMap::from([(solution.root.clone(), vec![start])]);
    let mut queue = VecDeque::from([solution.root.clone()]);
    while let Some(parent) = queue.pop_front() {
        for edge in &solution.packages[&parent].dependencies {
            if !paths.contains_key(&edge.key) {
                let mut path = paths[&parent].clone();
                path.push(solution.packages[&edge.key].coordinate.clone());
                paths.insert(edge.key.clone(), path);
                queue.push_back(edge.key.clone());
            }
        }
    }
    let selected = &solution.packages[key];
    let mut requirements = Vec::new();
    for (parent, candidate) in &solution.packages {
        for edge in candidate
            .dependencies
            .iter()
            .filter(|edge| &edge.key == key)
        {
            let mut path = paths[parent].clone();
            path.push(selected.coordinate.clone());
            let origins = edge
                .origins
                .iter()
                .map(|origin| {
                    Ok(Origin {
                        path: HostPathCarrier::from_path(&origin.manifest)
                            .map_err(|error| PmError::new(error.to_string()))?,
                        span: origin.span.clone(),
                    })
                })
                .collect::<PmResult<_>>()?;
            requirements.push(Requirement {
                from: candidate.coordinate.clone(),
                path,
                versions: edge.versions.iter().map(ToString::to_string).collect(),
                source: edge.source.to_string(),
                test: edge.test,
                origins,
            });
        }
    }
    Ok(Explanation {
        root: root.resolution.root_coordinate.clone(),
        purpose: root.resolution.purpose,
        target: root.resolution.target.clone(),
        selected: selected.coordinate.clone(),
        source: root
            .resolution
            .nodes
            .iter()
            .find(|node| node.coordinate == selected.coordinate)
            .expect("selected package has a locked node")
            .source
            .clone(),
        requirements,
    })
}

fn print_explanation(value: &Explanation) -> PmResult<()> {
    println!("{} [{} / {}]", value.root, value.purpose, value.target);
    println!("  {} ({}): locked selection", value.selected, value.source);
    if value.root == value.selected {
        println!("    selected workspace root");
    }
    for requirement in &value.requirements {
        let path = requirement
            .path
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(" -> ");
        let versions = if requirement.versions.is_empty() {
            "source version".into()
        } else {
            requirement.versions.join(" and ")
        };
        let test = if requirement.test { " [test]" } else { "" };
        println!(
            "    {path}{test}: requires {versions} from {}",
            requirement.source
        );
        for origin in &requirement.origins {
            let path = origin
                .path
                .to_path_buf()
                .map_err(|error| PmError::new(error.to_string()))?;
            println!(
                "      {} (bytes {}..{})",
                path.display(),
                origin.span.start,
                origin.span.end
            );
        }
    }
    Ok(())
}
