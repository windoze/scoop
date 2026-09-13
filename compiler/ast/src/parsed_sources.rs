use scoop_identity::SourceIdentity;
use std::fmt;

use crate::{NonEmptyVec, SourceFile};

/// A parsed AST paired with its persistent semantic source identity.
#[derive(Debug, Clone, PartialEq)]
pub struct IdentifiedParsedSource {
    identity: SourceIdentity,
    ast: SourceFile,
}

impl IdentifiedParsedSource {
    pub fn new(identity: SourceIdentity, ast: SourceFile) -> Self {
        Self { identity, ast }
    }

    pub const fn identity(&self) -> &SourceIdentity {
        &self.identity
    }

    pub const fn ast(&self) -> &SourceFile {
        &self.ast
    }

    pub fn into_ast(self) -> SourceFile {
        self.ast
    }
}

/// A non-empty parsed source set with globally meaningful, unique identities.
///
/// Fields stay private so downstream stages cannot receive a source set whose
/// identities have not been checked for uniqueness.
#[derive(Debug, Clone, PartialEq)]
pub struct AllParsedSources {
    sources: NonEmptyVec<IdentifiedParsedSource>,
}

impl AllParsedSources {
    pub fn try_new(
        sources: NonEmptyVec<IdentifiedParsedSource>,
    ) -> Result<Self, AllParsedSourcesError> {
        let mut seen = Vec::with_capacity(sources.len());
        for (source_index, source) in sources.iter().enumerate() {
            if let Some(first_index) = seen
                .iter()
                .position(|seen: &&SourceIdentity| *seen == source.identity())
            {
                return Err(AllParsedSourcesError::DuplicateSourceIdentity {
                    first_index,
                    duplicate_index: source_index,
                    identity: source.identity().clone(),
                });
            }
            seen.push(source.identity());
        }
        Ok(Self { sources })
    }

    pub const fn sources(&self) -> &NonEmptyVec<IdentifiedParsedSource> {
        &self.sources
    }

    pub fn into_sources(self) -> NonEmptyVec<IdentifiedParsedSource> {
        self.sources
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AllParsedSourcesError {
    DuplicateSourceIdentity {
        first_index: usize,
        duplicate_index: usize,
        identity: SourceIdentity,
    },
}

impl fmt::Display for AllParsedSourcesError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateSourceIdentity {
                first_index,
                duplicate_index,
                identity,
            } => write!(
                formatter,
                "source {duplicate_index} duplicates source {first_index} identity {}/{}",
                identity.cone(),
                identity.logical_path()
            ),
        }
    }
}

impl std::error::Error for AllParsedSourcesError {}

mod current_cone;
pub use current_cone::*;

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::{PackageSyntax, Span};
    use scoop_identity::{ConeCoordinate, NormalizedSourcePath, SourceContentDigest};

    fn identity(path: &str) -> SourceIdentity {
        identity_in("ast", path)
    }

    fn identity_in(cone_name: &str, path: &str) -> SourceIdentity {
        let cone = ConeCoordinate::new("test", cone_name, "0.0.0")
            .unwrap()
            .identity()
            .unwrap();
        SourceIdentity::new(cone, NormalizedSourcePath::new(path).unwrap()).unwrap()
    }

    fn parsed(path: &str) -> IdentifiedParsedSource {
        parsed_with_identity(identity(path))
    }

    fn parsed_with_identity(identity: SourceIdentity) -> IdentifiedParsedSource {
        IdentifiedParsedSource::new(
            identity,
            SourceFile {
                package: PackageSyntax::RootPackage,
                imports: Vec::new(),
                declarations: Vec::new(),
                span: Span::new(0, 0),
            },
        )
    }

    #[test]
    fn validates_non_empty_unique_source_identities() {
        let sources = NonEmptyVec::new(parsed("src/first.scoop"), vec![parsed("src/second.scoop")]);
        let parsed = AllParsedSources::try_new(sources).expect("valid source set");

        assert_eq!(parsed.sources().len(), 2);
        assert_eq!(
            parsed.sources().first().identity().logical_path().as_str(),
            "src/first.scoop"
        );
    }

    #[test]
    fn rejects_a_duplicate_source_identity() {
        let duplicate = identity("src/duplicate.scoop");
        let error = AllParsedSources::try_new(NonEmptyVec::new(
            IdentifiedParsedSource::new(duplicate.clone(), parsed("src/first.scoop").into_ast()),
            vec![IdentifiedParsedSource::new(
                duplicate.clone(),
                parsed("src/second.scoop").into_ast(),
            )],
        ))
        .expect_err("duplicate identities must fail");

        assert_eq!(
            error,
            AllParsedSourcesError::DuplicateSourceIdentity {
                first_index: 0,
                duplicate_index: 1,
                identity: duplicate,
            }
        );
    }

    #[test]
    fn current_cone_sources_seal_canonical_ast_and_complete_sidecars() {
        let first = identity("src/first.scoop");
        let second = identity("src/second.scoop");
        let sources = AllParsedSources::try_new(NonEmptyVec::new(
            parsed_with_identity(first.clone()),
            vec![parsed_with_identity(second.clone())],
        ))
        .unwrap();
        let parsed = CurrentConeParsedSources::try_new(
            sources,
            NonEmptyVec::new(
                CurrentSourceText::new(first.clone(), "package first".to_owned()),
                vec![CurrentSourceText::new(
                    second.clone(),
                    "package second".to_owned(),
                )],
            ),
            NonEmptyVec::new(
                CurrentSourceDiagnosticContext::new(first.clone(), "/tmp/first.scoop".into()),
                vec![CurrentSourceDiagnosticContext::new(
                    second.clone(),
                    "/tmp/second.scoop".into(),
                )],
            ),
        )
        .unwrap();

        assert_eq!(parsed.cone(), first.cone());
        let text = parsed.source_texts().get(&second).unwrap();
        assert_eq!(text.text(), "package second");
        assert_eq!(
            text.content_digest(),
            SourceContentDigest::from_utf8("package second")
        );
        assert_eq!(
            parsed.diagnostic_context().display_locator(&first),
            Some(Path::new("/tmp/first.scoop"))
        );
    }

    #[test]
    fn current_cone_sources_reject_mixed_and_noncanonical_identities() {
        let first = identity("src/first.scoop");
        let foreign = identity_in("foreign", "src/second.scoop");
        let mixed = AllParsedSources::try_new(NonEmptyVec::new(
            parsed_with_identity(first.clone()),
            vec![parsed_with_identity(foreign.clone())],
        ))
        .unwrap();
        assert!(matches!(
            CurrentConeParsedSources::try_new(
                mixed,
                NonEmptyVec::new(
                    CurrentSourceText::new(first.clone(), String::new()),
                    vec![CurrentSourceText::new(foreign.clone(), String::new())],
                ),
                NonEmptyVec::new(
                    CurrentSourceDiagnosticContext::new(first.clone(), "first".into()),
                    vec![CurrentSourceDiagnosticContext::new(
                        foreign,
                        "foreign".into()
                    )],
                ),
            ),
            Err(CurrentConeParsedSourcesError::MixedCone { index: 1, .. })
        ));

        let later = identity("src/z.scoop");
        let earlier = identity("src/a.scoop");
        let noncanonical = AllParsedSources::try_new(NonEmptyVec::new(
            parsed_with_identity(later.clone()),
            vec![parsed_with_identity(earlier.clone())],
        ))
        .unwrap();
        assert!(matches!(
            CurrentConeParsedSources::try_new(
                noncanonical,
                NonEmptyVec::new(
                    CurrentSourceText::new(later.clone(), String::new()),
                    vec![CurrentSourceText::new(earlier.clone(), String::new())],
                ),
                NonEmptyVec::new(
                    CurrentSourceDiagnosticContext::new(later, "later".into()),
                    vec![CurrentSourceDiagnosticContext::new(
                        earlier,
                        "earlier".into()
                    )],
                ),
            ),
            Err(CurrentConeParsedSourcesError::NonIncreasingSourceIdentity {
                first_index: 0,
                second_index: 1,
                ..
            })
        ));
    }

    #[test]
    fn current_cone_sources_reject_sidecar_coverage_mismatch() {
        let first = identity("src/first.scoop");
        let second = identity("src/second.scoop");
        let sources = AllParsedSources::try_new(NonEmptyVec::new(
            parsed_with_identity(first.clone()),
            vec![parsed_with_identity(second.clone())],
        ))
        .unwrap();
        let error = CurrentConeParsedSources::try_new(
            sources,
            NonEmptyVec::new(
                CurrentSourceText::new(first.clone(), String::new()),
                Vec::new(),
            ),
            NonEmptyVec::new(
                CurrentSourceDiagnosticContext::new(first, "first".into()),
                vec![CurrentSourceDiagnosticContext::new(second, "second".into())],
            ),
        )
        .unwrap_err();
        assert_eq!(
            error,
            CurrentConeParsedSourcesError::SourceTextCount {
                expected: 2,
                actual: 1,
            }
        );

        let sources = AllParsedSources::try_new(NonEmptyVec::new(
            parsed_with_identity(identity("src/first.scoop")),
            vec![parsed_with_identity(identity("src/second.scoop"))],
        ))
        .unwrap();
        let wrong = identity("src/wrong.scoop");
        assert!(matches!(
            CurrentConeParsedSources::try_new(
                sources,
                NonEmptyVec::new(
                    CurrentSourceText::new(identity("src/first.scoop"), String::new()),
                    vec![CurrentSourceText::new(wrong, String::new())],
                ),
                NonEmptyVec::new(
                    CurrentSourceDiagnosticContext::new(
                        identity("src/first.scoop"),
                        "first".into(),
                    ),
                    vec![CurrentSourceDiagnosticContext::new(
                        identity("src/second.scoop"),
                        "second".into(),
                    )],
                ),
            ),
            Err(CurrentConeParsedSourcesError::SourceTextIdentity { index: 1, .. })
        ));
    }
}
