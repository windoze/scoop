//! File packages and import collections (DESIGN sections 2.1-2.3).
//!
//! One file's package and its import surface are recorded structurally
//! in the Export HIR. Within a single-Cone compile every resolved import
//! target is a `CurrentCone` binding; direct-dependency sources arrive
//! with upstream artifact loading, and the same closed sums extend
//! without schema changes.

use la_arena::Arena;
use scoop_ast::Span;

use crate::ids::PackageId;

/// One package name in this Cone, shared by every file declaring it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageDecl {
    /// Dot-separated segments; empty for the root package.
    pub segments: Vec<String>,
}

impl PackageDecl {
    pub fn root() -> Self {
        PackageDecl {
            segments: Vec::new(),
        }
    }

    pub fn is_root(&self) -> bool {
        self.segments.is_empty()
    }

    /// Diagnostic spelling (`a.b.c`, `<root>`).
    pub fn display(&self) -> String {
        if self.segments.is_empty() {
            "<root>".to_owned()
        } else {
            self.segments.join(".")
        }
    }
}

/// The provenance of one import binding: why this Cone may see the
/// target. The closed sum keeps re-export witnesses and current-Cone
/// lookups distinct; dependency witnesses arrive with `.slib` loading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportBindingSource {
    /// The target is declared in the current Cone.
    CurrentCone {
        /// Import witness span in the importing file.
        witness: Span,
    },
}

/// One resolved exact-import binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedTargetBinding {
    /// Local short name (alias or target short name).
    pub local_name: String,
    /// The typed target; within one Cone this names a current-Cone
    /// declaration namespace entry.
    pub target: ImportedTarget,
    /// Non-empty in declaration order; diamond merges keep every path.
    pub sources: Vec<ImportBindingSource>,
}

/// The namespace role of an import target. The sum is closed per the
/// language's declaration namespaces; adding a kind is a schema change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportedTarget {
    Function { qualified_name: String },
    Property { qualified_name: String },
    Type { qualified_name: String },
    TypeAlias { qualified_name: String },
}

/// One file's import surface, recorded before any name resolution runs.
#[derive(Debug, Clone, Default)]
pub struct FileImports {
    /// Exact imports (including `public import`) in declaration order.
    pub exact: Vec<ExactImport>,
    /// Star imports in declaration order.
    pub star: Vec<StarImport>,
}

impl FileImports {
    pub fn is_empty(&self) -> bool {
        self.exact.is_empty() && self.star.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactImport {
    pub public: bool,
    /// Dot-separated target path segments.
    pub path: Vec<String>,
    pub alias: Option<String>,
    pub span: Span,
    /// Resolved binding (alias or final segment short name). Resolution
    /// fills this after declaration collection; `None` means unresolved
    /// (diagnosed) rather than unknown.
    pub binding: Option<ImportedTargetBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StarImport {
    pub public: bool,
    pub path: Vec<String>,
    pub span: Span,
}

/// The Cone-wide package and import index.
#[derive(Debug, Default, Clone)]
pub struct SemanticSurface {
    pub packages: Arena<PackageDecl>,
    /// Per source file: its package id and import surface.
    pub files: Vec<FileSurface>,
}

impl SemanticSurface {
    pub fn package_of_file(&self, file: usize) -> &PackageDecl {
        &self.packages[self.files[file].package]
    }
}

#[derive(Debug, Clone)]
pub struct FileSurface {
    pub package: PackageId,
    pub imports: FileImports,
}
