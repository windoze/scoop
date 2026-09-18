use super::*;

/// Export-side identity and transparent target of one source `typealias`.
///
/// The alias never appears in [`Type`]: every use is expanded to `target`
/// during HIR lowering. `source_target` separately preserves whether the
/// source's outermost type reference selected another alias, which is needed
/// to serialize the M23 alias graph without introducing a second type tree.
#[derive(Debug, Clone)]
pub struct TypeAliasDecl {
    pub name: String,
    pub access: DeclarationAccess,
    /// The recursively expanded, alias-free semantic type.
    pub target: TypeId,
    pub source_target: TypeAliasSourceTarget,
    pub origin: DefinitionOrigin,
}

/// The source-level outer edge of a transparent alias declaration.
///
/// `Expanded` means the outer type form was not itself another typealias, so
/// [`TypeAliasDecl::target`] is serialized as a signature. `Alias` retains
/// the selected declaration while the same `target` field still carries its
/// complete expansion for ordinary type checking and code generation.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum TypeAliasSourceTarget {
    Expanded,
    Alias(ExportTypeAliasId),
}
