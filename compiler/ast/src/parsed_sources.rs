use std::fmt;

use crate::{NonEmptyVec, SourceFile};

/// Identifies one M23-1 parse request. This value is request-local and is not
/// a persistent source or Cone identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Stage1RequestId(u64);

impl Stage1RequestId {
    pub const fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn into_raw(self) -> u64 {
        self.0
    }
}

/// Identifies one source only within a specific M23-1 parse request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Stage1SourceHandle {
    request: Stage1RequestId,
    local_index: u32,
}

impl Stage1SourceHandle {
    pub const fn new(request: Stage1RequestId, local_index: u32) -> Self {
        Self {
            request,
            local_index,
        }
    }

    pub const fn request(self) -> Stage1RequestId {
        self.request
    }

    pub const fn local_index(self) -> u32 {
        self.local_index
    }
}

/// A parsed AST paired with its request-local source handle.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedSource {
    source_handle: Stage1SourceHandle,
    ast: SourceFile,
}

impl ParsedSource {
    pub fn new(source_handle: Stage1SourceHandle, ast: SourceFile) -> Self {
        Self { source_handle, ast }
    }

    pub const fn source_handle(&self) -> Stage1SourceHandle {
        self.source_handle
    }

    pub const fn ast(&self) -> &SourceFile {
        &self.ast
    }

    pub fn into_ast(self) -> SourceFile {
        self.ast
    }
}

/// A non-empty, fully parsed source set belonging to one request.
///
/// Fields stay private so downstream stages can only receive a set whose
/// handles have been checked for request membership and uniqueness.
#[derive(Debug, Clone, PartialEq)]
pub struct AllParsedSources {
    request: Stage1RequestId,
    sources: NonEmptyVec<ParsedSource>,
}

impl AllParsedSources {
    pub fn try_new(
        request: Stage1RequestId,
        sources: NonEmptyVec<ParsedSource>,
    ) -> Result<Self, AllParsedSourcesError> {
        let mut seen = Vec::with_capacity(sources.len());
        for (source_index, source) in sources.iter().enumerate() {
            let handle = source.source_handle();
            if handle.request() != request {
                return Err(AllParsedSourcesError::DifferentRequest {
                    source_index,
                    expected: request,
                    actual: handle.request(),
                });
            }
            if let Some(first_index) = seen.iter().position(|seen| *seen == handle) {
                return Err(AllParsedSourcesError::DuplicateSourceHandle {
                    first_index,
                    duplicate_index: source_index,
                    handle,
                });
            }
            seen.push(handle);
        }
        Ok(Self { request, sources })
    }

    pub const fn request(&self) -> Stage1RequestId {
        self.request
    }

    pub const fn sources(&self) -> &NonEmptyVec<ParsedSource> {
        &self.sources
    }

    pub fn into_sources(self) -> NonEmptyVec<ParsedSource> {
        self.sources
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AllParsedSourcesError {
    DifferentRequest {
        source_index: usize,
        expected: Stage1RequestId,
        actual: Stage1RequestId,
    },
    DuplicateSourceHandle {
        first_index: usize,
        duplicate_index: usize,
        handle: Stage1SourceHandle,
    },
}

impl fmt::Display for AllParsedSourcesError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DifferentRequest {
                source_index,
                expected,
                actual,
            } => write!(
                formatter,
                "source {source_index} belongs to stage-1 request {}, expected {}",
                actual.into_raw(),
                expected.into_raw()
            ),
            Self::DuplicateSourceHandle {
                first_index,
                duplicate_index,
                handle,
            } => write!(
                formatter,
                "source {duplicate_index} duplicates source {first_index} handle {} in stage-1 request {}",
                handle.local_index(),
                handle.request().into_raw()
            ),
        }
    }
}

impl std::error::Error for AllParsedSourcesError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PackageSyntax, Span};

    fn parsed(handle: Stage1SourceHandle) -> ParsedSource {
        ParsedSource::new(
            handle,
            SourceFile {
                package: PackageSyntax::RootPackage,
                imports: Vec::new(),
                declarations: Vec::new(),
                span: Span::new(0, 0),
            },
        )
    }

    #[test]
    fn validates_non_empty_unique_same_request_sources() {
        let request = Stage1RequestId::from_raw(7);
        let sources = NonEmptyVec::new(
            parsed(Stage1SourceHandle::new(request, 3)),
            vec![parsed(Stage1SourceHandle::new(request, 8))],
        );
        let parsed = AllParsedSources::try_new(request, sources).expect("valid source set");

        assert_eq!(parsed.request(), request);
        assert_eq!(parsed.sources().len(), 2);
        assert_eq!(parsed.sources().first().source_handle().local_index(), 3);
    }

    #[test]
    fn rejects_a_handle_from_another_request() {
        let request = Stage1RequestId::from_raw(7);
        let other = Stage1RequestId::from_raw(9);
        let error = AllParsedSources::try_new(
            request,
            NonEmptyVec::new(
                parsed(Stage1SourceHandle::new(request, 0)),
                vec![parsed(Stage1SourceHandle::new(other, 1))],
            ),
        )
        .expect_err("mixed requests must fail");

        assert_eq!(
            error,
            AllParsedSourcesError::DifferentRequest {
                source_index: 1,
                expected: request,
                actual: other,
            }
        );
    }

    #[test]
    fn rejects_a_duplicate_source_handle() {
        let request = Stage1RequestId::from_raw(7);
        let handle = Stage1SourceHandle::new(request, 3);
        let error = AllParsedSources::try_new(
            request,
            NonEmptyVec::new(parsed(handle), vec![parsed(handle)]),
        )
        .expect_err("duplicate handles must fail");

        assert_eq!(
            error,
            AllParsedSourcesError::DuplicateSourceHandle {
                first_index: 0,
                duplicate_index: 1,
                handle,
            }
        );
    }
}
