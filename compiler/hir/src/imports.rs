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
use crate::{
    ClassId, EnumId, ExportTypeAliasId, FunctionId, InterfaceId, ObjectId, PropertyId, StructId,
};

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
    /// The target is declared in the current Cone; the witness is the
    /// import statement that granted the local binding.
    CurrentCone { witness: Span },
}

/// The namespace role of one import target, carrying the session-local
/// typed id of the resolved declaration. Cross-Cone wire forms carry
/// persistent ids instead and are produced at serialization time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportedTarget {
    Function {
        function: FunctionId,
    },
    Struct {
        declaration: StructId,
    },
    Enum {
        declaration: EnumId,
    },
    Class {
        declaration: ClassId,
    },
    Interface {
        declaration: InterfaceId,
    },
    Object {
        declaration: ObjectId,
    },
    TypeAlias {
        alias: ExportTypeAliasId,
    },
    Property {
        property: PropertyId,
    },
    /// One enum variant admitted by a star import of the enum's variant
    /// surface (`import pkg.E.*`, spec 4.2) or published by a public
    /// star expansion. Exact selectors never bind variants.
    Variant {
        variant: crate::declarations::EnumVariantRef,
    },
}

/// One target with the non-empty source set authorizing it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedTargetBinding {
    pub target: ImportedTarget,
    pub sources: Vec<ImportBindingSource>,
}

/// One resolved import binding: the local short name plus every target
/// it may denote (overload sets contribute one entry per function;
/// cross-namespace names contribute one entry per namespace).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedBinding {
    pub local_name: String,
    /// Non-empty exactly when the import resolved.
    pub targets: Vec<ImportedTargetBinding>,
}

impl ImportedBinding {
    /// Constructs a binding from at least one target; the argument is
    /// structurally non-empty by construction at every call site.
    pub fn of(local_name: String, targets: Vec<ImportedTargetBinding>) -> Self {
        debug_assert!(!targets.is_empty());
        ImportedBinding {
            local_name,
            targets,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }
}

/// One re-export binding created by `public import` (DESIGN 2.4): the
/// destination is the importing file's package and the alias-or-short
/// name. Targets keep their origin identities — a re-export creates no
/// wrapper, second declaration, or second typealias target. Within a
/// single-Cone compile every published target is authorized by the
/// implicit core edge; dependency witnesses arrive with upstream
/// artifact loading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReExport {
    /// Destination package (the importing file's package).
    pub package: PackageId,
    /// Destination short name (alias or target short name).
    pub name: String,
    pub span: Span,
    pub binding: ImportedBinding,
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
    /// The resolved binding; `None` until resolution runs (or when the
    /// import was diagnosed unresolved), never an unknown state.
    pub binding: Option<ImportedBinding>,
}

impl ExactImport {
    /// The short name this import binds locally.
    pub fn local_name(&self) -> &str {
        self.alias
            .as_deref()
            .unwrap_or_else(|| self.path.last().expect("import paths are non-empty"))
    }

    /// Diagnostic spelling of the selector.
    pub fn display_path(&self) -> String {
        self.path.join(".")
    }
}

/// One star selector's resolved surface (DESIGN 2.2): either a declared
/// package or the variant-name surface of one enum. Stars naming other
/// nominal owners are diagnosed unresolved in v1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StarSurface {
    /// Every top-level declaration of the package that is visible to
    /// the importing file.
    Package { package: PackageId },
    /// The variant short names of one enum (spec 4.2).
    EnumOwner { declaration: EnumId },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StarImport {
    pub public: bool,
    pub path: Vec<String>,
    pub span: Span,
    /// The resolved surface; `None` until resolution runs (or when the
    /// selector was diagnosed unresolved), never an unknown state.
    pub binding: Option<StarSurface>,
}

/// The Cone-wide package and import index.
#[derive(Debug, Default, Clone)]
pub struct SemanticSurface {
    pub packages: Arena<PackageDecl>,
    /// Per source file: its package id and import surface.
    pub files: Vec<FileSurface>,
    /// Re-export bindings published by `public import`, in deterministic
    /// file and declaration order (DESIGN 2.4). Same-origin entries in
    /// one destination merge at artifact-serialization time.
    pub reexports: Vec<ReExport>,
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
