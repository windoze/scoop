use std::{fs, path::Path};

use serde_json::Value;

use super::support::*;

fn why(root: &Path, purpose: &str) -> Value {
    let output = success(pm(
        root,
        root,
        &[
            "deps",
            "--workspace",
            "--purpose",
            purpose,
            "--why",
            "org.example:json",
            "--json",
        ],
    ));
    serde_json::from_str(&output).unwrap()
}

#[test]
fn why_reports_each_root_binding_all_constraints_inherited_locations_and_test_edges() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    core(root);
    write(
        &root.join("Cone.toml"),
        "schema=1\n[workspace]\nmembers=['first','second']\n[workspace.cone]\ngroup='org.example'\nversion='0.1.0'\n[workspace.dependencies]\n'org.example:json'={path='json1',version='1.0.0'}\n[overrides]\n'scoop:scoop.core'={path='core'}\n",
    );
    write(
        &root.join("first/Cone.toml"),
        "schema=1\n[cone]\ngroup={workspace=true}\nname='first'\nversion={workspace=true}\nkind='library'\n[dependencies]\n'org.example:left'={path='../left'}\n'org.example:right'={path='../right'}\n[test-dependencies]\n'org.example:json'={workspace=true}\n",
    );
    write(
        &root.join("second/Cone.toml"),
        &format!(
            "{}\n[dependencies]\n'org.example:json'={{path='../json2', version='2.0.0'}}\n",
            cone("second", "0.1.0")
        ),
    );
    for (name, requirement) in [("left", "~1.4.0"), ("right", ">=1.0.0, <2.0.0")] {
        write(
            &root.join(name).join("Cone.toml"),
            &format!(
                "{}\n[dependencies]\n'org.example:json'={{path='../json1',version='{requirement}'}}\n",
                cone(name, "1.0.0")
            ),
        );
    }
    write(&root.join("json1/Cone.toml"), &cone("json", "1.4.0"));
    write(&root.join("json2/Cone.toml"), &cone("json", "2.2.0"));
    failure(
        pm(
            root,
            root,
            &["deps", "--workspace", "--why", "org.example:json"],
        ),
        "no locked resolution",
    );
    assert!(!root.join("Cone.lock").exists());
    success(pm(root, root, &["resolve", "--workspace"]));
    let lock = fs::read(root.join("Cone.lock")).unwrap();
    let stamp = fs::metadata(root.join("Cone.lock"))
        .unwrap()
        .modified()
        .unwrap();
    let production = why(root, "production");
    assert_eq!(production["format"], 1);
    let roots = production["resolutions"].as_array().unwrap();
    assert_eq!(roots.len(), 2);
    assert_eq!(roots[0]["selected"], "org.example:json:1.4.0");
    assert_eq!(roots[1]["selected"], "org.example:json:2.2.0");
    assert_eq!(roots[0]["requirements"].as_array().unwrap().len(), 2);
    let left = &roots[0]["requirements"][0];
    assert_eq!(left["from"], "org.example:left:1.0.0");
    assert_eq!(left["versions"][0], "~1.4.0");
    assert_eq!(
        left["path"],
        serde_json::json!([
            "org.example:first:0.1.0",
            "org.example:left:1.0.0",
            "org.example:json:1.4.0"
        ])
    );
    assert_eq!(left["test"], false);
    assert_eq!(fs::read(root.join("Cone.lock")).unwrap(), lock);
    assert_eq!(
        fs::metadata(root.join("Cone.lock"))
            .unwrap()
            .modified()
            .unwrap(),
        stamp
    );
    let human = success(pm(
        root,
        root,
        &[
            "deps",
            "-p",
            "org.example:first",
            "--why",
            "org.example:json",
        ],
    ));
    assert!(human.contains("locked selection") && human.contains("requires ~1.4.0"));
    assert!(!human.contains("org.example:json:2.2.0"));
    failure(
        pm(
            root,
            root,
            &["deps", "--workspace", "--why", "org.example:absent"],
        ),
        "not in any selected locked graph",
    );
    failure(
        pm(
            root,
            root,
            &["deps", "--purpose", "test", "--why", "org.example:json"],
        ),
        "no locked resolution",
    );
    success(pm(
        root,
        root,
        &["resolve", "--workspace", "--purpose", "test"],
    ));
    let test = why(root, "test");
    let requirements = test["resolutions"][0]["requirements"].as_array().unwrap();
    assert_eq!(requirements.len(), 3);
    let direct = requirements
        .iter()
        .find(|edge| edge["test"] == true)
        .unwrap();
    assert_eq!(direct["from"], "org.example:first:0.1.0");
    assert_eq!(direct["origins"].as_array().unwrap().len(), 2);
    assert_eq!(
        direct["path"],
        serde_json::json!(["org.example:first:0.1.0", "org.example:json:1.4.0"])
    );
    let root_only = success(pm(
        root,
        root,
        &[
            "deps",
            "-p",
            "org.example:first",
            "--why",
            "org.example:first",
        ],
    ));
    assert!(root_only.contains("selected workspace root"));
    let lock = fs::read(root.join("Cone.lock")).unwrap();
    write(&root.join("json1/Cone.toml"), &cone("json", "1.4.1"));
    failure(
        pm(
            root,
            root,
            &["deps", "--workspace", "--why", "org.example:json"],
        ),
        "run scoop pm resolve",
    );
    assert_eq!(fs::read(root.join("Cone.lock")).unwrap(), lock);
}
