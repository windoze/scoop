use std::fs;

use scoop_pm::sources::registry::{RegistryIndex, RegistryVersion, published_manifest};

use super::{server::Server, support::*};

fn publish(server: &Server, name: &str, version: &str, dependencies: &str) -> RegistryVersion {
    let text = format!("{}\n[dependencies]\n{dependencies}\n", cone(name, version));
    let (coordinate, kind, dependencies, _) = published_manifest(&text).unwrap();
    let encoder = zstd::stream::Encoder::new(Vec::new(), 1).unwrap();
    let mut archive = tar::Builder::new(encoder);
    let mut header = tar::Header::new_gnu();
    header.set_mode(0o644);
    header.set_size(text.len() as u64);
    header.set_cksum();
    archive
        .append_data(&mut header, "Cone.toml", text.as_bytes())
        .unwrap();
    let bytes = archive.into_inner().unwrap().finish().unwrap();
    let location = format!("archives/{name}-{version}.tar.zst");
    let version = RegistryVersion {
        version: coordinate.version().into(),
        kind,
        dependencies,
        archive: location.clone(),
        sha256: scoop_wire::sha256(&bytes).to_string(),
        yanked: false,
    };
    server.set(&format!("/{location}"), bytes);
    version
}

fn index(server: &Server, name: &str, versions: Vec<RegistryVersion>) {
    server.set("/config.json", br#"{"format":1}"#.to_vec());
    server.set(
        &format!("/packages/org.example/{name}.json"),
        serde_json::to_vec(&RegistryIndex {
            format: 1,
            group: "org.example".into(),
            name: name.into(),
            versions,
        })
        .unwrap(),
    );
}

#[test]
fn real_registry_resolution_downloads_only_selected_versions_and_shares_across_projects() {
    let server = Server::new();
    let url = &server.url;
    let old = publish(
        &server,
        "a",
        "1.1.0",
        &format!("'org.example:json'={{version='1.0.0',registry='{url}'}}"),
    );
    let latest = publish(
        &server,
        "a",
        "1.2.0",
        &format!("'org.example:json'={{version='2.0.0',registry='{url}'}}"),
    );
    index(&server, "a", vec![old.clone(), latest.clone()]);
    let one = publish(&server, "json", "1.0.0", "");
    let two = publish(&server, "json", "2.0.0", "");
    index(&server, "json", vec![one, two]);
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    core(root);
    let dependencies = format!(
        "'org.example:a'={{version='1.0.0',registry='{url}'}}\n'org.example:json'={{version='1.0.0',registry='{url}'}}"
    );
    for name in ["first", "second"] {
        app(root, name, &dependencies);
        success(pm(root, &root.join(name), &["resolve"]));
    }
    assert_eq!(server.count("/archives/a-1.2.0.tar.zst"), 0);
    assert_eq!(server.count("/archives/json-2.0.0.tar.zst"), 0);
    assert_eq!(server.count("/archives/a-1.1.0.tar.zst"), 1);
    assert_eq!(server.count("/archives/json-1.0.0.tar.zst"), 1);
    let requests = server.total();
    let first = root.join("first");
    success(pm(root, &first, &["fetch", "--offline"]));
    success(pm(root, &first, &["resolve", "--locked"]));
    assert_eq!(server.total(), requests);
    let new = publish(
        &server,
        "a",
        "1.3.0",
        &format!("'org.example:json'={{version='1.0.0',registry='{url}'}}"),
    );
    index(&server, "a", vec![old.clone(), latest.clone(), new.clone()]);
    success(pm(root, &first, &["resolve"]));
    assert_eq!(server.total(), requests);
    let updated = success(pm(root, &first, &["update", "--package", "org.example:a"]));
    assert!(
        updated.contains("1.1.0")
            && updated.contains("1.3.0")
            && updated.contains("required by org.example:first"),
        "{updated}"
    );
    assert_eq!(
        lock(&first).resolutions[0]
            .nodes
            .iter()
            .find(|node| node.coordinate.name() == "a")
            .unwrap()
            .coordinate
            .version(),
        "1.3.0"
    );
    assert_eq!(server.count("/packages/org.example/json.json"), 2);
    let bytes = fs::read(first.join("Cone.lock")).unwrap();
    failure(
        pm(
            root,
            &first,
            &["update", "--package", "org.example:a", "--precise", "9.0.0"],
        ),
        "dependency resolution failed",
    );
    assert_eq!(fs::read(first.join("Cone.lock")).unwrap(), bytes);
    assert!(
        !String::from_utf8(bytes)
            .unwrap()
            .contains(root.to_str().unwrap())
    );
    let next = publish(
        &server,
        "a",
        "1.4.0",
        &format!("'org.example:json'={{version='1.0.0',registry='{url}'}}"),
    );
    index(&server, "a", vec![old, latest, new, next]);
    let path = first.join("Cone.toml");
    write(
        &path,
        &fs::read_to_string(&path).unwrap().replace(
            "'org.example:a'={version='1.0.0'",
            "'org.example:a'={version='1.4.0'",
        ),
    );
    success(pm(root, &first, &["resolve"]));
    assert_eq!(
        lock(&first).resolutions[0]
            .nodes
            .iter()
            .find(|node| node.coordinate.name() == "a")
            .unwrap()
            .coordinate
            .version(),
        "1.4.0"
    );
}
