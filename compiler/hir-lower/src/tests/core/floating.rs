use super::super::*;

/// Reuse the numeric declarations from the real core. Codec companions belong
/// to the complete sysroot fixtures, not this minimal compiler-core fixture.
pub(super) fn floating_declarations() -> Vec<Decl> {
    static DECLARATIONS: std::sync::OnceLock<Vec<Decl>> = std::sync::OnceLock::new();
    DECLARATIONS
        .get_or_init(|| {
            let mut source = scoop_parser::parse(include_str!(
                "../../../../../sysroot/lib/scoop.core/src/floating.scoop"
            ))
            .expect("the actual floating core parses");
            for declaration in &mut source.declarations {
                if let Decl::Struct(declaration) = declaration {
                    declaration
                        .members
                        .retain(|member| !matches!(member, ast::StructMember::Companion(_)));
                }
            }
            source.declarations
        })
        .clone()
}
