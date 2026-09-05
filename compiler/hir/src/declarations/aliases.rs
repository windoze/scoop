use super::*;

/// Export-side identity and transparent target of one source `typealias`.
///
/// The alias never appears in [`Type`]: every use is expanded to `target`
/// during HIR lowering. Keeping the declaration in Export HIR preserves the
/// source API and access proof that M23 will serialize without creating a
/// second nominal/runtime identity.
#[derive(Debug, Clone)]
pub struct TypeAliasDecl {
    pub name: String,
    pub access: DeclarationAccess,
    pub target: TypeId,
    pub origin: DefinitionOrigin,
}
