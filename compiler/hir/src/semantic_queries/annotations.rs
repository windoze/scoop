//! Annotation views preserve the frontend's ordinary target and value model.

use crate::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SemanticAnnotationTarget {
    Current(SourceAnnotationTarget),
    Imported(AnnotationTargetV1),
}

#[derive(Clone, Copy, Debug)]
pub enum SemanticAnnotationDeclaration<'a> {
    Current(&'a SourceAnnotationDeclaration),
    Imported(&'a ImportedAnnotationDeclaration),
}

#[derive(Clone, Copy, Debug)]
pub enum SemanticAnnotations<'a> {
    Current(&'a [SourceAnnotationApplication]),
    Imported(&'a [AnnotationApplicationV1]),
}
