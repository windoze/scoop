use super::QueryJson;
use crate::Reference;
use scoop_identity::{DefinitionOrigin, EvaluationOrigin, SourceIdentity, SourceSpan};
use serde::{Serialize, Serializer};

impl Serialize for QueryJson<'_, SourceIdentity> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        object!(serializer, {
            "cone": Reference::from(self.0.cone()),
            "path": self.0.logical_path().as_str(),
        })
    }
}

impl Serialize for QueryJson<'_, SourceSpan> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        object!(serializer, {
            "startByte": self.0.start_byte(), "endByte": self.0.end_byte(),
        })
    }
}

impl Serialize for QueryJson<'_, scoop_hir::SemanticSourceLocation> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        object!(serializer, {
            "source": QueryJson(&self.0.source), "span": QueryJson(&self.0.span),
        })
    }
}

impl Serialize for QueryJson<'_, scoop_hir::Span> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        object!(serializer, { "startByte": self.0.start, "endByte": self.0.end })
    }
}

pub(crate) struct Location<'a> {
    pub source: &'a SourceIdentity,
    pub span: SourceSpan,
}

impl Serialize for Location<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        object!(serializer, { "source": QueryJson(self.source), "span": QueryJson(&self.span) })
    }
}

impl Serialize for QueryJson<'_, DefinitionOrigin> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        object!(serializer, {
            "source": QueryJson(self.0.source()), "span": QueryJson(&self.0.span()),
            "context": Reference::from(self.0.context()),
        })
    }
}

impl Serialize for QueryJson<'_, EvaluationOrigin> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        object!(serializer, {
            "source": QueryJson(self.0.source()), "span": QueryJson(&self.0.span()),
            "context": Reference::from(self.0.context()),
        })
    }
}

impl Serialize for QueryJson<'_, scoop_hir::ExportDefinitionSourceV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        QueryJson(self.0.origin()).serialize(serializer)
    }
}
