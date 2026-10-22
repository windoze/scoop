use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct CompilerQuery {
    pub context: MethodId,
    pub query: Query,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Query {
    DescribeTypes {
        types: Vec<TypeReference>,
    },
    DescribeMembers {
        types: Vec<TypeReference>,
    },
    DescribeAnnotations {
        targets: Vec<AnnotationTarget>,
        declarations: Vec<Reference<PersistentAnnotationId>>,
    },
    DescribeDefaults {
        defaults: Vec<DefaultReference>,
    },
    DescribeBodies {
        callables: Vec<CallableDeclaration>,
    },
    TypeRelation {
        relation: TypeRelation,
        left: TypeReference,
        right: TypeReference,
    },
    LookupExport {
        provider: Reference<ConeIdentity>,
        path: Vec<String>,
        namespace: ExportNamespace,
    },
    SourceLocation {
        targets: Vec<AnnotationTarget>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct DefaultReference {
    pub owner: CallableDeclaration,
    pub parameter: u32,
    pub arguments: Option<Vec<TypeReference>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TypeRelation {
    Equal,
    Subtype,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExportNamespace {
    Type,
    Value,
}
