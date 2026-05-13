//! Shared helpers for stable dump renderers.

use std::fmt;
use std::path::{Component, Path, PathBuf};

use crate::span::Span;
use crate::stable_id::{StableCanonicalKey, stable_local_label};
use crate::ty::{EffectRow, TypeId, TypeStore};

/// Stable local dump key derived from an owner identity plus lexical source anchor.
pub(crate) struct LocalEntityKey {
    owner: String,
    source_path: String,
    span: Span,
    entity_kind: String,
    readable_name: String,
    ordinal: usize,
}

impl LocalEntityKey {
    pub(crate) fn new(
        owner: impl Into<String>,
        source_path: &Path,
        span: Span,
        entity_kind: impl Into<String>,
        readable_name: impl Into<String>,
        ordinal: usize,
    ) -> Self {
        Self::new_with_workspace_root(
            owner,
            source_path,
            span,
            entity_kind,
            readable_name,
            ordinal,
            dump_workspace_root(),
        )
    }

    fn new_with_workspace_root(
        owner: impl Into<String>,
        source_path: &Path,
        span: Span,
        entity_kind: impl Into<String>,
        readable_name: impl Into<String>,
        ordinal: usize,
        workspace_root: &Path,
    ) -> Self {
        Self {
            owner: owner.into(),
            source_path: normalize_dump_path_with_workspace_root(source_path, workspace_root),
            span,
            entity_kind: entity_kind.into(),
            readable_name: readable_name.into(),
            ordinal,
        }
    }

    pub(crate) fn label(&self, role: &str) -> String {
        stable_local_label(role, self)
    }
}

impl StableCanonicalKey for LocalEntityKey {
    fn canonical_text(&self) -> String {
        format!(
            "local_entity|owner={}|path={}|start={}|end={}|kind={}|name={}|ordinal={}",
            self.owner,
            self.source_path,
            self.span.start,
            self.span.end,
            self.entity_kind,
            self.readable_name,
            self.ordinal,
        )
    }
}

/// Small writer for deterministic, indented dump text.
pub(crate) struct IndentWriter {
    out: String,
    indent: usize,
}

impl IndentWriter {
    pub(crate) fn new() -> Self {
        Self {
            out: String::new(),
            indent: 0,
        }
    }

    pub(crate) fn line(&mut self, text: impl AsRef<str>) {
        for _ in 0..self.indent {
            self.out.push_str("    ");
        }
        self.out.push_str(text.as_ref());
        self.out.push('\n');
    }

    pub(crate) fn push_indent(&mut self) {
        self.indent += 1;
    }

    pub(crate) fn pop_indent(&mut self) {
        self.indent -= 1;
    }

    pub(crate) fn finish(self) -> String {
        self.out
    }
}

pub(crate) fn format_debug<T>(value: &T) -> String
where
    T: fmt::Debug + ?Sized,
{
    format!("{value:?}")
}

pub(crate) fn format_effect_row(types: &TypeStore, row: &EffectRow) -> String {
    if row.terms.is_empty() {
        return "Pure".to_string();
    }
    row.terms
        .iter()
        .map(|&ty| types.display(ty).to_string())
        .collect::<Vec<_>>()
        .join(" + ")
}

pub(crate) fn format_type(types: &TypeStore, ty: TypeId) -> String {
    if (ty.as_u32() as usize) < types.len() {
        return types.display(ty).to_string();
    }
    "<invalid-type>".to_string()
}

fn dump_workspace_root() -> &'static Path {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .unwrap_or(manifest_dir)
}

/// Normalizes a path for dump output so fixtures stay workspace-relative and slash-stable.
pub(crate) fn normalize_dump_path(path: &Path) -> String {
    normalize_dump_path_with_workspace_root(path, dump_workspace_root())
}

fn normalize_dump_path_with_workspace_root(path: &Path, workspace_root: &Path) -> String {
    let relative = if path.is_absolute() {
        path.strip_prefix(workspace_root)
            .unwrap_or(path)
            .to_path_buf()
    } else {
        path.to_path_buf()
    };

    lexical_normalize(&relative)
        .to_string_lossy()
        .replace('\\', "/")
}

fn lexical_normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    let mut rooted = false;

    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => {
                normalized.push(component.as_os_str());
                rooted = true;
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if normalized.file_name().is_some() {
                    normalized.pop();
                } else if !rooted {
                    normalized.push("..");
                }
            }
            Component::Normal(part) => normalized.push(part),
        }
    }

    normalized
}

#[cfg(test)]
mod tests {
    use super::{LocalEntityKey, normalize_dump_path, normalize_dump_path_with_workspace_root};
    use crate::span::Span;
    use std::path::Path;

    #[test]
    fn normalize_dump_path_keeps_workspace_relative_fixture_paths() {
        let input = std::path::Path::new("crates/scoop/../../tests/fixtures/mir/example.scoop");
        assert_eq!(
            normalize_dump_path(input),
            "tests/fixtures/mir/example.scoop"
        );
    }

    #[test]
    fn normalize_dump_path_strips_checkout_root_prefixes_consistently() {
        let root_a = Path::new("/tmp/scoop-checkout-a");
        let root_b = Path::new("/tmp/scoop-checkout-b");
        let path_a = root_a.join("tests/fixtures/mir/example.scoop");
        let path_b = root_b.join("tests/fixtures/mir/example.scoop");

        assert_eq!(
            normalize_dump_path_with_workspace_root(&path_a, root_a),
            "tests/fixtures/mir/example.scoop"
        );
        assert_eq!(
            normalize_dump_path_with_workspace_root(&path_b, root_b),
            "tests/fixtures/mir/example.scoop"
        );
    }

    #[test]
    fn local_entity_key_labels_change_with_ordinal_but_hide_dense_ids() {
        let span = Span::new(10, 20);
        let first = LocalEntityKey::new(
            "owner:sample.main",
            std::path::Path::new("tests/fixtures/sample.scoop"),
            span,
            "local",
            "tmp",
            0,
        )
        .label("local");
        let second = LocalEntityKey::new(
            "owner:sample.main",
            std::path::Path::new("tests/fixtures/sample.scoop"),
            span,
            "local",
            "tmp",
            1,
        )
        .label("local");

        assert_ne!(first, second);
        assert!(first.starts_with("local#h"));
        assert!(second.starts_with("local#h"));
    }

    #[test]
    fn local_entity_key_labels_stay_stable_across_checkout_roots() {
        let span = Span::new(10, 20);
        let root_a = Path::new("/tmp/scoop-checkout-a");
        let root_b = Path::new("/tmp/scoop-checkout-b");
        let path_a = root_a.join("tests/fixtures/sample.scoop");
        let path_b = root_b.join("tests/fixtures/sample.scoop");

        let first = LocalEntityKey::new_with_workspace_root(
            "owner:sample.main",
            &path_a,
            span,
            "local",
            "tmp",
            0,
            root_a,
        )
        .label("local");
        let second = LocalEntityKey::new_with_workspace_root(
            "owner:sample.main",
            &path_b,
            span,
            "local",
            "tmp",
            0,
            root_b,
        )
        .label("local");

        assert_eq!(first, second);
        assert!(first.starts_with("local#h"));
    }
}
