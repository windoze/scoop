use scoop_example::catalog::Catalog;
use scoop_example::search::search;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::TempDir;

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn catalog_text() -> String {
    fs::read_to_string(repository().join("sysroot/share/scoop/examples/catalog.toml")).unwrap()
}

fn copy(source: &Path, destination: &Path) {
    fs::create_dir_all(destination.parent().unwrap()).unwrap();
    fs::copy(source, destination).unwrap();
}

struct Installation(TempDir);

impl Installation {
    fn new() -> Self {
        let install = Self(tempfile::tempdir().unwrap());
        let source = repository().join("sysroot");
        let directory = Path::new("share/scoop/examples");
        copy(
            &source.join(directory).join("catalog.toml"),
            &install.root().join(directory).join("catalog.toml"),
        );
        let catalog = Catalog::parse(&catalog_text()).unwrap();
        for topic in catalog.topics {
            copy(
                &source.join(directory).join(&topic.document),
                &install.root().join(directory).join(&topic.document),
            );
            for file in topic.files {
                copy(
                    &source.join(&file.source),
                    &install.root().join(&file.source),
                );
            }
        }
        copy(
            Path::new(env!("CARGO_BIN_EXE_scoop-example")),
            &install.root().join("commands/scoop-example"),
        );
        install
    }

    fn root(&self) -> &Path {
        self.0.path()
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(self.root().join("commands/scoop-example"))
            .args(args)
            .current_dir(repository())
            .env("SCOOP_SYSROOT", repository().join("sysroot"))
            .output()
            .unwrap()
    }

    fn json(&self, args: &[&str]) -> Value {
        let mut arguments = args.to_vec();
        arguments.extend(["--format", "json"]);
        let output = self.run(&arguments);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        serde_json::from_slice(&output.stdout).unwrap()
    }
}

#[test]
fn published_catalog_has_complete_documents_and_real_files() {
    let catalog = Catalog::parse(&catalog_text()).unwrap();
    assert_eq!(catalog.topics.len(), 24);
    let root = repository().join("sysroot");
    for topic in catalog.topics {
        assert!(!topic.title.is_empty() && !topic.summary.is_empty());
        assert!(!topic.requirements.is_empty() && !topic.related.is_empty());
        let body =
            fs::read_to_string(root.join("share/scoop/examples").join(&topic.document)).unwrap();
        for section in ["## 运行", "## 预期结果", "## 关键说明", "## 常见误用"] {
            assert!(body.contains(section), "{} lacks {section}", topic.id);
        }
        assert!(!body.contains("```scoop"), "Source must stay in real files");
        for file in topic.files {
            assert!(
                !fs::read_to_string(root.join(file.source))
                    .unwrap()
                    .is_empty()
            );
        }
    }
}

#[test]
fn index_validation_catches_broken_editorial_references() {
    for (old, new) in [
        ("schema = 1", "schema = 2"),
        ("aliases = [\"option\",", "aliases = [\"language/enum\","),
        (
            "related = [\"language/enum\",",
            "related = [\"language/missing\",",
        ),
        ("path = \"main.scoop\"", "path = \"../main.scoop\""),
        ("id = \"language/option\"", "id = \"absent/option\""),
    ] {
        let text = catalog_text();
        assert!(text.contains(old));
        assert!(
            Catalog::parse(&text.replacen(old, new, 1)).is_err(),
            "{new}"
        );
    }
}

#[test]
fn search_matches_all_words_and_keeps_editorial_order() {
    let catalog = Catalog::parse(&catalog_text()).unwrap();
    let hits = search(&catalog, " JSON   序列化 ");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].topic.id, "json/typed");
    assert!(hits[0].matched_fields.contains(&"keywords"));
    assert_eq!(search(&catalog, "OPTION")[0].topic.id, "language/option");
    assert_eq!(
        search(&catalog, "language/o")
            .iter()
            .map(|h| h.topic.id.as_str())
            .collect::<Vec<_>>(),
        ["language/overview", "language/option"]
    );
    assert!(search(&catalog, "json nowhere-present").is_empty());
    assert!(search(&catalog, " ").is_empty());
    assert!(
        search(&catalog, "saved before failure").is_empty(),
        "Do not search program bodies"
    );
}

#[test]
fn directories_aliases_search_and_version_use_the_executable_installation() {
    let install = Installation::new();
    fs::write(
        install.root().join("VERSION"),
        "test-install\nrevision: abc\n",
    )
    .unwrap();
    assert_eq!(install.json(&[])["version"], "test-install\nrevision: abc");
    assert_eq!(install.json(&[])["kind"], "index");
    let all = install.json(&["--list"]);
    assert_eq!(all["topics"].as_array().unwrap().len(), 24);
    let group = install.json(&["scoop.json"]);
    assert_eq!(group["kind"], "group");
    assert_eq!(group["topics"][0]["id"], "json/typed");
    assert_eq!(install.json(&["可选值"])["id"], "language/option");
    let found = install.json(&["--search", "JSON 序列化"]);
    assert_eq!(found["kind"], "search");
    assert_eq!(found["topics"][0]["id"], "json/typed");
    assert!(
        !found["topics"][0]["matched_fields"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn every_topic_uses_the_same_complete_files_in_all_formats() {
    let install = Installation::new();
    let catalog = Catalog::parse(&catalog_text()).unwrap();
    for topic in catalog.topics {
        let json = install.json(&[&topic.id]);
        assert_eq!(json["version"], "开发内容，未提供发行版本");
        let markdown = install.run(&[&topic.id]);
        assert!(markdown.status.success());
        let markdown = String::from_utf8(markdown.stdout).unwrap();
        assert!(!markdown.contains('\u{1b}'));
        assert!(markdown.contains(json["body_markdown"].as_str().unwrap()));
        assert_eq!(json["files"].as_array().unwrap().len(), topic.files.len());
        for (file, data) in topic.files.iter().zip(json["files"].as_array().unwrap()) {
            let original = fs::read(install.root().join(&file.source)).unwrap();
            let source = install.run(&[&topic.id, "--format", "source", "--file", &file.path]);
            assert!(source.status.success());
            assert_eq!(source.stdout, original);
            assert_eq!(data["text"].as_str().unwrap().as_bytes(), original);
            assert!(markdown.contains(data["text"].as_str().unwrap()));
        }
    }
}

#[test]
fn single_file_source_is_raw_and_multi_file_source_requires_selection() {
    let install = Installation::new();
    let output = install.run(&["start/hello", "--format", "source"]);
    assert!(output.status.success());
    assert_eq!(
        output.stdout,
        b"fun main() {\n    println(\"Hello, Scoop!\")\n}\n"
    );
    let output = install.run(&["http/get", "--format", "source"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(error.contains("client.scoop") && error.contains("server.scoop"));
}

#[test]
fn usage_and_lookup_failures_are_atomic_and_json_errors_stay_on_stderr() {
    let install = Installation::new();
    for (args, code) in [
        (vec!["--list", "language/option"], 2),
        (vec!["--list", "--search", "option"], 2),
        (vec!["--search", " "], 2),
        (vec!["--file", "main.scoop"], 2),
        (vec!["--unknown-option"], 2),
        (vec!["no-such-topic"], 1),
        (vec!["--search", "no-such-topic"], 1),
    ] {
        let mut args = args;
        args.push("--format=json");
        let output = install.run(&args);
        assert_eq!(output.status.code(), Some(code), "{args:?}");
        assert!(output.stdout.is_empty());
        let error: Value = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(error["kind"], "error");
        assert_eq!(error["code"], code);
    }
    for args in [
        vec!["--format", "source"],
        vec!["language", "--format", "source"],
        vec![
            "start/hello",
            "--format",
            "source",
            "--file",
            "absent.scoop",
        ],
    ] {
        let output = install.run(&args);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
    let output = install.run(&["option-typo"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
}

#[test]
fn unreadable_content_does_not_emit_a_partial_topic() {
    let install = Installation::new();
    let index = install.root().join("share/scoop/examples/catalog.toml");
    let text = fs::read_to_string(&index).unwrap();
    // A newly referenced, missing input is an ordinary I/O error. Existing sources stay intact.
    fs::write(
        &index,
        text.replace(
            "src/scoop.http.server/examples/hello.scoop",
            "src/missing.scoop",
        ),
    )
    .unwrap();
    let output = install.run(&["http/get", "--format", "json"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let error: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert!(error["message"].as_str().unwrap().contains("missing.scoop"));
    // Catalog navigation does not read all programs or prepare libraries.
    assert_eq!(install.json(&["--list"])["kind"], "list");
}

#[test]
fn help_works_before_installation() {
    let output = Command::new(env!("CARGO_BIN_EXE_scoop-example"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("scoop --sysroot")
    );
}
