use std::fmt;

use scoop_identity::SourceIdentity;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PackageSyntax, Span};
    use scoop_identity::{ConeCoordinate, NormalizedSourcePath};

    fn identity(path: &str) -> SourceIdentity {
        let cone = ConeCoordinate::new("test", "ast", "0.0.0")
            .unwrap()
            .identity()
            .unwrap();
        SourceIdentity::new(cone, NormalizedSourcePath::new(path).unwrap()).unwrap()
    }

    fn parsed(path: &str) -> IdentifiedParsedSource {
        IdentifiedParsedSource::new(
            identity(path),
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
}
