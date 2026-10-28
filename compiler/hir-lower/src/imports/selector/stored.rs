//! Body imports query the same loaded namespaces after the borrowed world ends.

use super::*;
use scoop_identity::PackagePath;

impl CurrentUnitImports {
    pub(super) fn stored_package_cursor(
        &self,
        lowerer: &Lowerer,
        ast_segments: &[ast::Ident],
        canonical_segments: &[CanonicalIdentifier],
    ) -> (usize, NamespaceCursor<'static, 'static>) {
        let (current, current_len) = lowerer
            .top_level_namespaces
            .longest_package_prefix(lowerer.current_file, ast_segments);
        for length in (current_len..=canonical_segments.len()).rev() {
            let path = PackagePath::from_segments(canonical_segments[..length].to_vec());
            let Some(contributions) = self.direct_packages.get(&path) else {
                continue;
            };
            let visible = contributions
                .iter()
                .filter(|contribution| lowerer.dependency_in_source_scope(contribution.provider))
                .collect::<Vec<_>>();
            if visible.is_empty() {
                continue;
            }
            let groups = visible
                .into_iter()
                .flat_map(|contribution| contribution.groups.iter().cloned())
                .collect();
            let identity = if length == current_len {
                ResolvedImportNamespace::SplitPackage {
                    current,
                    direct: path,
                }
            } else {
                ResolvedImportNamespace::DirectPackage(path)
            };
            return (length, NamespaceCursor::Stored { identity, groups });
        }
        (
            current_len,
            NamespaceCursor::Current(ResolvedNamespace::Package(current)),
        )
    }
}
