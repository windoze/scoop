//! ScoopIR v0 的可序列化 schema（JSON）。
//!
//! 设计目标：
//! - **稳定**：字段名、结构与排序策略固定，便于作为 `.cone` 的长期兼容载体；
//! - **最小**：只覆盖 public API 的“头信息”（type + fun header），不包含实现细节；
//! - **可演进**：通过显式 `schema.version` 做版本协商（见 TODO T1106）。

use serde::{Deserialize, Serialize};

/// schema 名称：用于读写时进行 sanity check。
pub const SCOOPIR_SCHEMA_NAME: &str = "scoopir";

/// ScoopIR schema 版本（v0）。
pub const SCOOPIR_SCHEMA_VERSION: u32 = 0;

/// 一个 `.scoopir`（或 `api.scoopir`）文件的顶层结构。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScoopIrFile {
    pub schema: SchemaHeader,
    pub types: Vec<IrTypeDecl>,
    pub funs: Vec<IrFunDecl>,
}

impl ScoopIrFile {
    pub fn new_v0(types: Vec<IrTypeDecl>, funs: Vec<IrFunDecl>) -> Self {
        Self {
            schema: SchemaHeader::v0(),
            types,
            funs,
        }
    }
}

/// schema 头信息（强制包含版本号）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SchemaHeader {
    pub name: String,
    pub version: u32,
}

impl SchemaHeader {
    pub fn v0() -> Self {
        Self {
            name: SCOOPIR_SCHEMA_NAME.to_string(),
            version: SCOOPIR_SCHEMA_VERSION,
        }
    }
}

/// 声明处变型（declaration-site variance）。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IrVariance {
    In,
    Out,
}

/// 类型参数声明（类型头信息）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IrTypeParam {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variance: Option<IrVariance>,
}

/// 类型声明的分类（只覆盖 public API 的“声明头”）。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IrTypeDeclKind {
    Class,
    Interface,
    Struct,
    Enum,
    Effect,
    TypeAlias,
}

/// 一个 `public` 类型的声明头（不包含字段/方法/布局等实现细节）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IrTypeDecl {
    pub fqn: String,
    pub kind: IrTypeDeclKind,
    pub type_params: Vec<IrTypeParam>,
    /// Whether the nominal type carries compiler-recognized `@InteriorMutable` metadata.
    #[serde(default, skip_serializing_if = "is_false")]
    pub is_interior_mutable: bool,
    /// 若为 `typealias`，额外携带其 RHS（已在导出侧解析到 FQN，且按策略可能已展开）。
    ///
    /// 说明：
    /// - v0 的 `api.scoopir` 仅导出 public API 的“声明头”；但 typealias 若不携带 RHS，
    ///   下游无法在 typecheck lowering 阶段展开别名；
    /// - 该字段是可选的：老版本 `.cone`（或测试构造的最小文件）缺省为 None。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alias_of: Option<IrType>,
}

const fn is_false(value: &bool) -> bool {
    !*value
}

/// effect row（v0：仅保留 term 集合，空表示 `Pure`）。
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IrEffectRow {
    pub terms: Vec<IrType>,
}

/// ScoopIR v0 的类型表达式（用于函数签名与 public type 的参数/返回等）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum IrType {
    /// 名义类型（含 builtin 与用户类型），使用 FQN 表示。
    Named {
        fqn: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        args: Vec<IrType>,
        #[serde(skip_serializing_if = "Option::is_none")]
        eff: Option<IrEffectRow>,
    },
    /// 类型参数（例如 `T`）。
    Param { name: String },
    /// Tuple：`(A, B, ...)`。
    Tuple { elements: Vec<IrType> },
    /// 函数类型：`(A, B) -> C / R` 或 `T.(...) -> ... / R`。
    Function {
        #[serde(skip_serializing_if = "Option::is_none")]
        receiver: Option<Box<IrType>>,
        params: Vec<IrType>,
        return_ty: Box<IrType>,
        effects: IrEffectRow,
    },
    /// 受限 union：`A | B | ...`（用于分支合并等场景）。
    Union { variants: Vec<IrType> },
}

/// 函数声明的语义分类（与 `ast::FunDeclKind` 对齐）。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IrFunDeclKind {
    Regular,
    EffectOp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IrFunParam {
    pub name: String,
    pub ty: IrType,
}

/// 一个 `public` 函数的声明头（不包含函数体）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IrFunDecl {
    pub fqn: String,
    pub kind: IrFunDeclKind,
    /// v0：从签名中提取“被引用到的 type param 名字”（未引用到的声明处 type params 不会出现在这里）。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub type_params: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receiver: Option<IrType>,
    pub params: Vec<IrFunParam>,
    pub return_ty: IrType,
    pub effects: IrEffectRow,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_json_surface_stays_semantic_and_path_free(json: &str) {
        for forbidden in [
            "TypeId(",
            "ClosureId(",
            "SourceId(",
            "ConeId(",
            "SymbolId(",
            "BasicBlockId(",
            "LocalId(",
            "SiteId(",
            "StepSchemaId(",
            "ContinuationSchemaId(",
            "CaseTag(",
            "source_path",
            "decl_span",
            "/Users/",
            env!("CARGO_MANIFEST_DIR"),
        ] {
            assert!(
                !json.contains(forbidden),
                "healthy ScoopIR schema surface 不应泄漏 dense id/path 文本: {forbidden}\n{json}"
            );
        }
    }

    #[test]
    fn scoopir_json_baseline_stays_semantic_and_path_free() {
        let file = ScoopIrFile::new_v0(
            vec![IrTypeDecl {
                fqn: "a.Token".to_string(),
                kind: IrTypeDeclKind::Struct,
                type_params: vec![IrTypeParam {
                    name: "T".to_string(),
                    variance: Some(IrVariance::Out),
                }],
                is_interior_mutable: false,
                alias_of: None,
            }],
            vec![IrFunDecl {
                fqn: "a.make".to_string(),
                kind: IrFunDeclKind::Regular,
                type_params: vec!["T".to_string()],
                receiver: None,
                params: vec![IrFunParam {
                    name: "value".to_string(),
                    ty: IrType::Param {
                        name: "T".to_string(),
                    },
                }],
                return_ty: IrType::Named {
                    fqn: "a.Token".to_string(),
                    args: vec![IrType::Param {
                        name: "T".to_string(),
                    }],
                    eff: None,
                },
                effects: IrEffectRow::default(),
            }],
        );

        let json = serde_json::to_string_pretty(&file).unwrap();
        assert!(json.contains("\"fqn\": \"a.Token\""));
        assert!(json.contains("\"fqn\": \"a.make\""));
        assert_json_surface_stays_semantic_and_path_free(&json);
    }
}
