//! ScoopIR（Scoop Stable IR）v0：用于 `.cone` 分发的公共 API 描述。
//!
//! 当前阶段（TODO T1103）只导出：
//! - `public` 类型声明头（type kind + type params）
//! - `public` 函数声明头（签名：receiver/params/return/effects）
//!
//! 不导出：
//! - 函数体、字段/方法列表、布局信息等实现细节

mod export;
mod schema;

pub use export::{
    ScoopIrExportError, export_public_api_for_cone_sources, export_public_api_for_source,
};
pub use schema::{
    IrEffectRow, IrFunDecl, IrFunDeclKind, IrFunParam, IrType, IrTypeDecl, IrTypeDeclKind,
    IrTypeParam, IrVariance, SCOOPIR_SCHEMA_NAME, SCOOPIR_SCHEMA_VERSION, SchemaHeader,
    ScoopIrFile,
};

#[cfg(test)]
mod tests;
