//! 类型环境（type environment）。
//!
//! T0402：基于 sysroot AST 建立 type env（Any/Option/Raise），
//! 为后续 typecheck 提供“类型符号的声明头（kind + arity）”查询能力。
//!
//! T0425：扩展 type env 以收集 enum variants（tag + payload types），为 rich enum 的类型检查打底。

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::path::PathBuf;

use miette::Diagnostic;
use thiserror::Error;

use crate::ast;
use crate::resolve::{
    ConeInfo, ImportTable, Index, add_auto_prelude_star_imports, is_symbol_visible_from,
};
use crate::source::SourceFile;
use crate::span::Span;
use crate::sysroot::Sysroot;
use crate::target::TargetPlatform;

use super::builtin_annotations::{
    BuiltinAnnotationKind, DeprecatedAnnotationInfo, builtin_annotation_kind,
    parse_deprecated_annotation,
};

pub(crate) const ANY_REF_MARKER_FQN: &str = "scoop.core.AnyRef";
pub(crate) const ANY_VALUE_MARKER_FQN: &str = "scoop.core.AnyValue";

/// 类型符号的种类（type namespace）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeSymbolKind {
    /// 名义类型声明：class/interface/struct/enum/effect。
    Nominal(ast::TypeKind),
    /// 类型别名：`typealias Name = ...`。
    TypeAlias,
}

/// 注解可附着的目标（spec §15.5 `AnnotationTarget`）。
///
/// 说明：
/// - 该枚举用于 typecheck 阶段对 `@Target(...)` 的语义约束（T1016a）；
/// - 名字与 sysroot 的 `enum AnnotationTarget { ... }` 保持一致，便于诊断与 fixtures 编写；
/// - 当前实现只会在“注解类声明头”里缓存其 meta-annotations 的提取结果，
///   具体 enforcement 发生在 `typecheck::annotations`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnnotationTargetKind {
    Function,
    Property,
    Field,
    Param,
    Type,
    Constructor,
    LocalVariable,
    Expression,
    Module,
    TypeParam,
    EnumVariant,
}

impl AnnotationTargetKind {
    pub fn as_str(self) -> &'static str {
        match self {
            AnnotationTargetKind::Function => "Function",
            AnnotationTargetKind::Property => "Property",
            AnnotationTargetKind::Field => "Field",
            AnnotationTargetKind::Param => "Param",
            AnnotationTargetKind::Type => "Type",
            AnnotationTargetKind::Constructor => "Constructor",
            AnnotationTargetKind::LocalVariable => "LocalVariable",
            AnnotationTargetKind::Expression => "Expression",
            AnnotationTargetKind::Module => "Module",
            AnnotationTargetKind::TypeParam => "TypeParam",
            AnnotationTargetKind::EnumVariant => "EnumVariant",
        }
    }

    pub fn from_variant_name(name: &str) -> Option<Self> {
        match name {
            "Function" => Some(AnnotationTargetKind::Function),
            "Property" => Some(AnnotationTargetKind::Property),
            "Field" => Some(AnnotationTargetKind::Field),
            "Param" => Some(AnnotationTargetKind::Param),
            "Type" => Some(AnnotationTargetKind::Type),
            "Constructor" => Some(AnnotationTargetKind::Constructor),
            "LocalVariable" => Some(AnnotationTargetKind::LocalVariable),
            "Expression" => Some(AnnotationTargetKind::Expression),
            "Module" => Some(AnnotationTargetKind::Module),
            "TypeParam" => Some(AnnotationTargetKind::TypeParam),
            "EnumVariant" => Some(AnnotationTargetKind::EnumVariant),
            _ => None,
        }
    }
}

/// 注解保留策略（spec §15.5 `@Retention(policy)`，T1016a）。
///
/// 当前阶段只定义两档：
/// - `ComptimeOnly`：仅编译期可见；
/// - `ConePreserved`：会被导出到 `.cone`（导出行为由后续任务 T1016b 实现）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnotationRetentionPolicy {
    ComptimeOnly,
    ConePreserved,
}

impl AnnotationRetentionPolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            AnnotationRetentionPolicy::ComptimeOnly => "comptime",
            AnnotationRetentionPolicy::ConePreserved => "cone",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "comptime" | "comptime-only" | "comptime_only" => {
                Some(AnnotationRetentionPolicy::ComptimeOnly)
            }
            "cone" | "cone-preserved" | "cone_preserved" => {
                Some(AnnotationRetentionPolicy::ConePreserved)
            }
            _ => None,
        }
    }
}

/// typecheck 使用的“类型符号声明头”信息。
///
/// 当前阶段（T0402）只保留最小集合：
/// - `kind`：区分 nominal type 与 typealias
/// - `type_param_count`：泛型参数数量（arity）
/// - `span/decl_file`：用于 env 构建阶段的重复定义诊断与调试
#[derive(Debug, Clone)]
pub struct TypeSymbol {
    pub kind: TypeSymbolKind,
    /// `sealed interface` marker construct; distinct from a runtime interface.
    pub is_sealed_interface: bool,
    /// 是否为注解类（`annotation class ...`，spec §15.2）。
    ///
    /// 说明：
    /// - 该标记用于在 typecheck 阶段验证 `@Name(...)` 的 `Name` 必须引用一个注解类；
    /// - “target/retention/参数类型限制”等更完整规则留给后续任务实现（见 TODO T10xx）。
    pub is_annotation_class: bool,
    /// 若该类型是注解类，记录其 `@Target(...)` 约束（T1016a）。
    ///
    /// 说明：
    /// - `None`：未声明 `@Target`，默认允许出现在所有 annotatable elements 上（spec §15.3）；
    /// - `Some([])`：显式声明了空集合，表示该注解不可被使用（可用于“仅作为标记/保留”的占位，或在更高层报错）。
    pub annotation_targets: Option<Vec<AnnotationTargetKind>>,
    /// 若该类型是注解类，记录其 `@Retention(policy)`（T1016a）。
    ///
    /// 注意：导出到 `.cone` 的行为留给 T1016b；此处仅做“声明头收集”供后续阶段查询。
    pub annotation_retention: Option<AnnotationRetentionPolicy>,
    /// 若该类型是注解类，记录其主构造参数信息（T1019）。
    ///
    /// 说明：
    /// - 当前阶段仅用于在注解使用点做“参数类型 + 编译期常量”检查；
    /// - 更完整的默认值计算/可选参数规则会在后续任务中完善（spec §15.2 / §15.3）。
    pub annotation_params: Vec<AnnotationParamInfo>,
    pub type_param_count: usize,
    /// 是否包含 `eff` effect row 参数（spec §3.4 / §5.8）。
    ///
    /// 说明：
    /// - `eff` 参数不计入 `type_param_count`（arity），因为它不是 type argument；
    /// - use-site 的 `Type<eff Row>` lowering 需要知道声明处的默认值与声明文件上下文。
    pub eff_param: Option<EffParamInfo>,
    /// 类型参数名（按声明顺序）。
    ///
    /// 说明：
    /// - 早期阶段很多逻辑只需要 arity；但 `where` 约束满足性检查（T0458）需要把
    ///   `where T: Bound` 的左侧映射到“第几个 type arg”。
    pub type_param_names: Vec<String>,
    /// 声明处变型（declaration-site variance）：与 `type_param_count` 对齐的按位信息。
    ///
    /// 说明：
    /// - `None` 表示 invariant；
    /// - `Some(In|Out)` 对应 `in`/`out`。
    pub type_param_variances: Vec<Option<ast::TypeParamVariance>>,
    /// `where` 子句的约束信息（T0458）。
    ///
    /// 说明：
    /// - 这里保留 `TypeRef`（而不是提前 lowering 成 `TypeId`），以便在 use-site
    ///   结合具体 type args 做 substitution 后再进行检查。
    pub where_constraints: Vec<WhereConstraintInfo>,
    pub span: Span,
    pub decl_file: PathBuf,
}

/// Compile-time-only metadata for a `sealed interface` marker.
#[derive(Debug, Clone, Default)]
pub struct SealedMarkerInfo {
    pub direct_supers: Vec<String>,
    pub transitive_supers: Vec<String>,
}

/// 注解类主构造参数在 type env 中的最小表示（T1019）。
#[derive(Debug, Clone)]
pub struct AnnotationParamInfo {
    pub name: String,
    pub name_span: Span,
    pub ty: ast::TypeRef,
    pub has_default: bool,
}

/// `eff` effect row 参数在 type env 中的最小表示。
#[derive(Debug, Clone)]
pub struct EffParamInfo {
    pub span: Span,
    pub name: String,
    pub default: Option<ast::EffectRowExpr>,
}

/// `where` 子句的一条约束在 type env 中的最小表示。
#[derive(Debug, Clone)]
pub struct WhereConstraintInfo {
    pub span: Span,
    /// 约束目标在声明 type param 列表中的索引（0-based）。
    pub param_index: usize,
    /// 约束右侧的 bound TypeRef（在声明处文件上下文中解析/lower）。
    pub bound: ast::TypeRef,
}

/// nominal 类型声明头中的一条 direct supertype 定义。
///
/// 说明：
/// - `fqn` 供旧的 FQN-only 调用方继续复用；
/// - `ty` 保留原始 type args / `eff` use-site 语法，用于在具体实例化时做 substitution。
#[derive(Debug, Clone)]
pub struct DirectSupertypeInfo {
    pub fqn: String,
    pub ty: ast::TypeRef,
}

/// 单个源文件在 typecheck lowering 阶段需要的最小上下文信息。
///
/// 用途：
/// - typealias 展开（T0446）：需要在“声明处文件”的 package/import 规则下解析 RHS 类型引用；
/// - 其它跨文件 type position lowering（例如 sysroot enum variant 字段，T0426）也可复用。
#[derive(Debug, Clone)]
pub struct FileTypeContext {
    pub pkg_prefix: String,
    pub imports: ImportTable,
    pub cone: ConeInfo,
}

/// `typealias` 的声明信息（用于 typecheck 阶段展开别名）。
#[derive(Debug, Clone)]
pub struct TypeAliasInfo {
    pub decl_file: PathBuf,
    pub name_span: Span,
    pub ty: ast::TypeRef,
}

/// enum variant 的字段信息（payload field）。
#[derive(Debug, Clone)]
pub struct EnumVariantField {
    pub name: String,
    pub span: Span,
    pub ty: ast::TypeRef,
}

/// enum variant 信息（tag + payload types）。
#[derive(Debug, Clone)]
pub struct EnumVariantInfo {
    pub tag: u32,
    pub name: String,
    pub span: Span,
    pub fields: Vec<EnumVariantField>,
}

/// enum 声明的语义信息（当前阶段仅 variants）。
#[derive(Debug, Clone)]
pub struct EnumDecl {
    /// enum 声明所在的源文件（用于在 typecheck 阶段按 span 取回原始标识符文本）。
    pub decl_file: PathBuf,
    /// enum 的类型参数名（按声明顺序）。
    ///
    /// 说明：
    /// - 目前主要用于 enum variant 构造表达式的最小泛型实例化（T0426）；
    /// - 未来若引入更完整的泛型推断/约束系统，这里仍可作为声明信息来源。
    pub type_params: Vec<String>,
    pub variants: Vec<EnumVariantInfo>,
}

#[derive(Debug, Error, Diagnostic)]
pub enum TypeEnvError {
    #[error("重复的类型符号：{fqn}")]
    #[diagnostic(code(scoop::typecheck::duplicate_type_symbol))]
    DuplicateTypeSymbol {
        fqn: String,
        #[label("第一次定义在这里")]
        first: miette::SourceSpan,
        #[label("重复定义在这里")]
        second: miette::SourceSpan,
    },

    #[error("enum variant 重复定义：{enum_fqn}.{variant}")]
    #[diagnostic(code(scoop::typecheck::duplicate_enum_variant))]
    DuplicateEnumVariant {
        enum_fqn: String,
        variant: String,
        #[label("重复定义在这里")]
        second: miette::SourceSpan,
        #[label("第一次定义在这里")]
        first: miette::SourceSpan,
    },

    #[error("enum variant 字段重复定义：{enum_fqn}.{variant}.{field}")]
    #[diagnostic(code(scoop::typecheck::duplicate_enum_variant_field))]
    DuplicateEnumVariantField {
        enum_fqn: String,
        variant: String,
        field: String,
        #[label("重复定义在这里")]
        second: miette::SourceSpan,
        #[label("第一次定义在这里")]
        first: miette::SourceSpan,
    },

    #[error("`sealed interface` 只能在 trusted `syslib` cone 中定义：{fqn}")]
    #[diagnostic(code(scoop::typecheck::sealed_interface_user_definition_not_allowed))]
    SealedInterfaceUserDefinitionNotAllowed {
        fqn: String,
        #[label("这里需要 trusted `syslib` 身份")]
        span: miette::SourceSpan,
    },

    #[error("`sealed interface` body 必须为空：{fqn}")]
    #[diagnostic(code(scoop::typecheck::sealed_interface_must_be_empty))]
    SealedInterfaceMustBeEmpty {
        fqn: String,
        #[label("这里是不允许的 sealed interface body member")]
        span: miette::SourceSpan,
    },

    #[error("`sealed interface` 只能继承其它 sealed interface：{fqn} -> {super_fqn}")]
    #[diagnostic(code(scoop::typecheck::sealed_interface_supertype_must_be_sealed))]
    SealedInterfaceSupertypeMustBeSealed {
        fqn: String,
        super_fqn: String,
        #[label("这里的 supertype 不是 sealed interface")]
        span: miette::SourceSpan,
    },

    #[error("`sealed interface` 继承图存在循环：{cycle}")]
    #[diagnostic(code(scoop::typecheck::sealed_interface_inheritance_cycle))]
    SealedInterfaceInheritanceCycle {
        cycle: String,
        #[label("循环从这里回到自身")]
        span: miette::SourceSpan,
    },

    #[error("sealed marker bound 不能同时蕴涵 AnyRef 与 AnyValue：{fqn}")]
    #[diagnostic(code(scoop::typecheck::sealed_interface_mutually_exclusive_bound))]
    SealedInterfaceMutuallyExclusiveBound {
        fqn: String,
        #[label("这里同时包含互斥 marker")]
        span: miette::SourceSpan,
    },
}

/// 类型环境：通过 FQN 查询类型符号信息。
#[derive(Debug, Default, Clone)]
pub struct TypeEnv {
    by_fqn: HashMap<String, TypeSymbol>,
    enums: HashMap<String, EnumDecl>,
    sources: HashMap<PathBuf, SourceFile>,
    files: HashMap<PathBuf, ast::File>,
    supertypes: HashMap<String, Vec<String>>,
    supertype_defs: HashMap<String, Vec<DirectSupertypeInfo>>,
    sealed_markers: HashMap<String, SealedMarkerInfo>,
    file_ctx: HashMap<PathBuf, FileTypeContext>,
    type_aliases: HashMap<String, TypeAliasInfo>,
    deprecated_types: HashMap<String, DeprecatedAnnotationInfo>,
    deprecated_values: HashMap<String, DeprecatedAnnotationInfo>,
    deprecated_funs: HashMap<DeprecatedDeclKey, DeprecatedAnnotationInfo>,
    /// 编译目标平台（用于 capability gating；默认 host）。
    target_platform: TargetPlatform,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct DeprecatedDeclKey {
    decl_file: PathBuf,
    decl_span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SealedMarkerVisitState {
    Visiting,
    Done,
}

impl TypeEnv {
    /// 从 sysroot AST 构建类型环境。
    ///
    /// 说明：
    /// - sysroot 是编译器“内建 API 的声明源”，因此 typecheck 的起点应由 sysroot 决定；
    /// - 当前阶段仅收集声明头信息，不解析函数体/方法体。
    pub fn from_sysroot(sysroot: &Sysroot, index: &Index) -> Result<Self, TypeEnvError> {
        let mut env = Self::default();
        for f in sysroot.index_files() {
            env.collect_from_file(&f.source, &f.ast, index)?;
        }
        env.rebuild_sealed_marker_metadata()?;
        Ok(env)
    }

    /// 将一个普通源文件的类型声明头信息合并进当前环境。
    ///
    /// 说明：
    /// - 该方法用于把“当前编译单元的用户代码”纳入 type env，从而支持后续阶段：
    ///   - TypeRef lowering 的泛型 arity 检查（T0403）
    ///   - 顶层签名检查（T0404）
    /// - 目前依旧只收集声明头（kind + arity），不进入函数体/方法体。
    pub fn extend_from_file(
        &mut self,
        source: &SourceFile,
        file: &ast::File,
        index: &Index,
    ) -> Result<(), TypeEnvError> {
        self.collect_from_file(source, file, index)?;
        self.rebuild_sealed_marker_metadata()
    }

    /// 按 FQN 查询类型符号。
    pub fn type_symbol(&self, fqn: &str) -> Option<&TypeSymbol> {
        self.by_fqn.get(fqn)
    }

    /// 返回给定 FQN 的 type params 数量（arity）。
    pub fn type_param_count(&self, fqn: &str) -> Option<usize> {
        self.type_symbol(fqn).map(|s| s.type_param_count)
    }

    /// 返回给定 FQN 的声明处 type param variances（若不是 nominal/typealias 或未收集则为 None）。
    pub fn type_param_variances(&self, fqn: &str) -> Option<&[Option<ast::TypeParamVariance>]> {
        self.type_symbol(fqn)
            .map(|s| s.type_param_variances.as_slice())
    }

    /// 按 FQN 查询 enum 的 variant 信息（若该类型不是 enum 或未收集到则为 None）。
    pub fn enum_decl(&self, fqn: &str) -> Option<&EnumDecl> {
        self.enums.get(fqn)
    }

    /// 通过文件路径获取对应的 `SourceFile`（若该文件在构建 env 时被收集过）。
    pub fn source(&self, path: &Path) -> Option<&SourceFile> {
        self.sources.get(path)
    }

    /// 通过文件路径获取对应的 AST（若该文件在构建 env 时被收集过）。
    pub fn file_ast(&self, path: &Path) -> Option<&ast::File> {
        self.files.get(path)
    }

    /// 遍历当前 type env 中已登记的源文件与 AST。
    pub fn files(&self) -> impl Iterator<Item = (&PathBuf, &ast::File)> {
        self.files.iter()
    }

    /// 获取当前编译目标平台（用于 capability gating）。
    pub fn target_platform(&self) -> &TargetPlatform {
        &self.target_platform
    }

    /// 覆盖编译目标平台（主要用于 fixtures/测试或未来的 cross-compile driver）。
    pub fn set_target_platform(&mut self, target_platform: TargetPlatform) {
        self.target_platform = target_platform;
    }

    /// 注入一个“外部声明来源”的 `SourceFile`。
    ///
    /// 用途（T1105）：
    /// - `.cone` 依赖的 `api.scoopir` 会被反解为一组合成的 `ast::TypeRef`/签名信息；
    /// - 为了让后续 lowering 能通过 span 切片取回标识符文本，需要把该合成 source 挂到 env 上；
    /// - 同一路径重复注入时保持第一次写入（避免覆盖）。
    pub fn insert_external_source(&mut self, source: SourceFile) {
        self.sources
            .entry(source.path().to_path_buf())
            .or_insert(source);
    }

    /// 直接注入一个外部 type symbol（例如来自 `.cone` 的 public API）。
    pub fn insert_external_type_symbol(
        &mut self,
        fqn: String,
        symbol: TypeSymbol,
    ) -> Result<(), TypeEnvError> {
        self.insert_symbol(fqn, symbol)
    }

    /// 注入一个外部 `typealias` 的声明信息（用于别名展开）。
    ///
    /// 用途（T1302）：
    /// - `.cone` 依赖会在注入 public API 时，把 `typealias` 的 RHS 一并带入；
    /// - 下游在 typecheck lowering 阶段需要据此展开别名（包括泛型实例化）。
    ///
    /// 约定：
    /// - 若同名 alias 已存在（极少发生），保留第一次注入的版本（与 `insert_external_source` 一致）。
    pub(crate) fn insert_external_type_alias(
        &mut self,
        fqn: String,
        decl_file: PathBuf,
        name_span: Span,
        ty: ast::TypeRef,
    ) {
        self.type_aliases.entry(fqn).or_insert(TypeAliasInfo {
            decl_file,
            name_span,
            ty,
        });
    }

    /// 返回给定源文件的 type lowering 上下文（package/import）。
    pub fn file_type_context(&self, path: &Path) -> Option<&FileTypeContext> {
        self.file_ctx.get(path)
    }

    /// 返回给定源文件的 owner cone metadata。
    pub fn file_cone_info(&self, path: &Path) -> Option<ConeInfo> {
        self.file_type_context(path).map(|ctx| ctx.cone)
    }

    /// 返回某个 type symbol 的 owner cone metadata。
    pub fn type_symbol_owner_cone_info(&self, fqn: &str) -> Option<ConeInfo> {
        let sym = self.type_symbol(fqn)?;
        self.file_cone_info(&sym.decl_file)
    }

    /// 按 FQN 查询 typealias 的声明信息（用于别名展开与循环检测）。
    pub fn type_alias(&self, fqn: &str) -> Option<&TypeAliasInfo> {
        self.type_aliases.get(fqn)
    }

    pub(crate) fn deprecated_type(&self, fqn: &str) -> Option<&DeprecatedAnnotationInfo> {
        self.deprecated_types.get(fqn)
    }

    pub(crate) fn deprecated_value(&self, fqn: &str) -> Option<&DeprecatedAnnotationInfo> {
        self.deprecated_values.get(fqn)
    }

    pub(crate) fn deprecated_fun(
        &self,
        decl_file: &Path,
        decl_span: Span,
    ) -> Option<&DeprecatedAnnotationInfo> {
        self.deprecated_funs.get(&DeprecatedDeclKey {
            decl_file: decl_file.to_path_buf(),
            decl_span,
        })
    }

    /// 返回给定 nominal type 的 direct supertypes（仅 FQN 视图；不包含隐式 `Any`）。
    ///
    /// 说明：
    /// - 该接口保留给仍只关心“继承图形状”的调用方；
    /// - 若需要保留 type args / `eff` use-site 信息，请改用 `direct_supertype_infos`。
    pub fn direct_supertypes(&self, fqn: &str) -> Option<&[String]> {
        self.supertypes.get(fqn).map(|v| v.as_slice())
    }

    /// 返回给定 nominal type 的 direct supertypes（保留原始 type args / `eff` use-site 语法）。
    pub fn direct_supertype_infos(&self, fqn: &str) -> Option<&[DirectSupertypeInfo]> {
        self.supertype_defs.get(fqn).map(|v| v.as_slice())
    }

    /// 查询给定 FQN 是否为 compile-time-only sealed marker。
    pub fn is_sealed_interface(&self, fqn: &str) -> bool {
        self.by_fqn
            .get(fqn)
            .is_some_and(|sym| sym.is_sealed_interface)
    }

    /// 返回 sealed marker 的 direct sealed super markers。
    pub fn sealed_marker_direct_supers(&self, fqn: &str) -> Option<&[String]> {
        self.sealed_markers
            .get(fqn)
            .map(|info| info.direct_supers.as_slice())
    }

    /// 返回 sealed marker 的 transitive super marker closure。
    pub fn sealed_marker_transitive_supers(&self, fqn: &str) -> Option<&[String]> {
        self.sealed_markers
            .get(fqn)
            .map(|info| info.transitive_supers.as_slice())
    }

    /// `marker_fqn` 是否等于或传递蕴涵 `expected_fqn`。
    pub fn sealed_marker_implies(&self, marker_fqn: &str, expected_fqn: &str) -> bool {
        marker_fqn == expected_fqn
            || self
                .sealed_marker_transitive_supers(marker_fqn)
                .is_some_and(|supers| supers.iter().any(|st| st == expected_fqn))
    }

    /// 返回所有名字为 `variant_name` 的 enum variant 候选（跨所有已收集的 enum）。
    ///
    /// 说明（T0426）：
    /// - 早期阶段我们允许通过 `Some(1)` 这种“不带 enum 前缀”的写法构造 variant；
    /// - 为避免引入完整的作用域/导入规则，本方法仅提供候选集合，
    ///   由 typecheck 决定“同名唯一”约束与报错策略。
    pub fn find_enum_variants_named(&self, variant_name: &str) -> Vec<(String, EnumVariantInfo)> {
        let mut out = Vec::new();
        for (enum_fqn, decl) in &self.enums {
            for v in &decl.variants {
                if v.name == variant_name {
                    out.push((enum_fqn.clone(), v.clone()));
                }
            }
        }
        out
    }

    /// 与 `find_enum_variants_named` 类似，但会过滤“仅供实现内部使用”的 helper enum。
    ///
    /// 当前约定：
    /// - 名称最后一段以 `__` 开头的 enum 视为 internal helper enum；
    /// - 这类 enum 的 bare variant ctor 只在**同包**源码里可见；
    /// - 跨包源码即使 `import scoop.core.*`，也不应因 `__TaskState.Created` 之类内部实现细节
    ///   让普通 `Created(...)` / `Ready(...)` 变成歧义。
    pub fn find_visible_enum_variants_named(
        &self,
        variant_name: &str,
        use_source: &SourceFile,
    ) -> Vec<(String, EnumVariantInfo)> {
        let use_pkg_prefix = self
            .file_type_context(use_source.path())
            .map(|ctx| ctx.pkg_prefix.as_str())
            .unwrap_or("");
        let mut out = Vec::new();

        for (enum_fqn, decl) in &self.enums {
            let enum_name = enum_fqn.rsplit('.').next().unwrap_or(enum_fqn);
            let is_internal_helper = enum_name.starts_with("__");
            if is_internal_helper {
                let enum_pkg_prefix = self
                    .file_type_context(&decl.decl_file)
                    .map(|ctx| ctx.pkg_prefix.as_str())
                    .unwrap_or_else(|| enum_fqn.rsplit_once('.').map(|(pkg, _)| pkg).unwrap_or(""));
                if enum_pkg_prefix != use_pkg_prefix {
                    continue;
                }
            }

            for v in &decl.variants {
                if v.name == variant_name {
                    out.push((enum_fqn.clone(), v.clone()));
                }
            }
        }

        out
    }

    fn collect_from_file(
        &mut self,
        source: &SourceFile,
        file: &ast::File,
        index: &Index,
    ) -> Result<(), TypeEnvError> {
        if self.files.contains_key(source.path()) {
            return Ok(());
        }

        // 记录源文件内容，供后续 typecheck 在跨文件引用（例如 sysroot enum variants）时
        // 通过 span 反查标识符文本。
        self.sources
            .entry(source.path().to_path_buf())
            .or_insert_with(|| source.clone());
        self.files
            .entry(source.path().to_path_buf())
            .or_insert_with(|| file.clone());

        let pkg_prefix = package_prefix(source, file.package.as_ref());
        let cone = index.cone_info_of_source(source);
        self.file_ctx
            .entry(source.path().to_path_buf())
            .or_insert_with(|| FileTypeContext {
                pkg_prefix: pkg_prefix.clone(),
                imports: build_import_table_best_effort(source, file, index),
                cone,
            });

        for item in &file.items {
            match item {
                ast::Item::TypeAlias(ta) => {
                    let name = source.slice(ta.name.span).to_string();
                    let fqn = join_prefix(&pkg_prefix, &name);

                    let type_param_names = ta
                        .type_params
                        .iter()
                        .map(|p| source.slice(p.name.span).to_string())
                        .collect::<Vec<_>>();
                    let type_param_variances = ta
                        .type_params
                        .iter()
                        .map(|p| p.variance)
                        .collect::<Vec<_>>();
                    self.insert_symbol(
                        fqn.clone(),
                        TypeSymbol {
                            kind: TypeSymbolKind::TypeAlias,
                            is_sealed_interface: false,
                            is_annotation_class: false,
                            annotation_targets: None,
                            annotation_retention: None,
                            annotation_params: Vec::new(),
                            type_param_count: type_param_names.len(),
                            eff_param: None,
                            type_param_names,
                            type_param_variances,
                            where_constraints: Vec::new(),
                            span: ta.name.span,
                            decl_file: source.path().to_path_buf(),
                        },
                    )?;

                    // 记录别名声明：用于 TypeRef lowering 阶段展开 alias（T0446）。
                    self.type_aliases.insert(
                        fqn,
                        TypeAliasInfo {
                            decl_file: source.path().to_path_buf(),
                            name_span: ta.name.span,
                            ty: ta.ty.clone(),
                        },
                    );
                    self.record_type_deprecation(
                        source,
                        &join_prefix(&pkg_prefix, &name),
                        &ta.annotations,
                        AnnotationTargetKind::Type,
                    );
                }
                ast::Item::Type(ty) => {
                    self.collect_type_decl(source, file, &pkg_prefix, ty, index)?;
                }
                ast::Item::Object(obj) => {
                    self.collect_object_decl(source, file, &pkg_prefix, obj, index)?;
                }
                ast::Item::Fun(fun) => {
                    self.record_fun_deprecation(
                        source,
                        fun.name.span,
                        &fun.annotations,
                        AnnotationTargetKind::Function,
                    );
                }
                ast::Item::Val(v) => {
                    let Some(name) = v.name() else {
                        continue;
                    };
                    let value_fqn = join_prefix(&pkg_prefix, source.slice(name.span));
                    self.record_value_deprecation(
                        source,
                        &value_fqn,
                        &v.annotations,
                        AnnotationTargetKind::Property,
                    );
                }
                ast::Item::ExtensionProperty(prop) => {
                    let value_fqn = join_prefix(&pkg_prefix, source.slice(prop.name.span));
                    self.record_value_deprecation(
                        source,
                        &value_fqn,
                        &prop.annotations,
                        AnnotationTargetKind::Property,
                    );
                }
                ast::Item::ComptimeIf(_) => {}
            }
        }

        Ok(())
    }

    fn collect_type_decl(
        &mut self,
        source: &SourceFile,
        file: &ast::File,
        prefix: &str,
        decl: &ast::TypeDecl,
        index: &Index,
    ) -> Result<(), TypeEnvError> {
        let name = source.slice(decl.name.span).to_string();
        let fqn = join_prefix(prefix, &name);
        let type_params: Vec<String> = decl
            .type_params
            .iter()
            .map(|p| source.slice(p.name.span).to_string())
            .collect();
        let type_param_variances: Vec<Option<ast::TypeParamVariance>> =
            decl.type_params.iter().map(|p| p.variance).collect();

        let mut where_constraints: Vec<WhereConstraintInfo> = Vec::new();
        if let Some(w) = &decl.where_clause {
            // type param name -> index
            let mut idx_of: HashMap<&str, usize> = HashMap::new();
            for (idx, name) in type_params.iter().enumerate() {
                idx_of.insert(name.as_str(), idx);
            }

            for c in &w.constraints {
                let name = source.slice(c.ty_param.span);
                let Some(&param_index) = idx_of.get(name) else {
                    // resolver/typecheck 会给出更精确的诊断；这里保持健壮性。
                    continue;
                };
                where_constraints.push(WhereConstraintInfo {
                    span: c.span,
                    param_index,
                    bound: c.bound.clone(),
                });
            }
        }

        let is_sealed_interface = decl.kind == ast::TypeKind::Interface
            && decl.modifiers.contains(&ast::Modifier::Sealed);
        if is_sealed_interface {
            self.check_sealed_interface_decl_shape(source, &fqn, decl)?;
        }

        let is_annotation_class = decl.kind == ast::TypeKind::Class
            && decl.modifiers.contains(&ast::Modifier::Annotation);
        let (annotation_targets, annotation_retention) = if is_annotation_class {
            extract_annotation_class_meta(source, file, decl, index)
        } else {
            (None, None)
        };
        let annotation_params = if is_annotation_class {
            collect_annotation_params(source, decl)
        } else {
            Vec::new()
        };

        self.insert_symbol(
            fqn.clone(),
            TypeSymbol {
                kind: TypeSymbolKind::Nominal(decl.kind),
                is_sealed_interface,
                is_annotation_class,
                annotation_targets,
                annotation_retention,
                annotation_params,
                type_param_count: decl.type_params.len(),
                eff_param: decl.eff_param.as_ref().map(|p| EffParamInfo {
                    span: p.span,
                    name: source.slice(p.name.span).to_string(),
                    default: p.default.clone(),
                }),
                type_param_names: type_params.clone(),
                type_param_variances,
                where_constraints,
                span: decl.name.span,
                decl_file: source.path().to_path_buf(),
            },
        )?;
        self.record_type_deprecation(source, &fqn, &decl.annotations, AnnotationTargetKind::Type);

        // 记录 direct supertypes（用于后续最小子类型/boxing 判断）。
        //
        // 注意：
        // - `supers` 保留 “解析后的 FQN” 视图，供只关心继承图形状的调用方复用；
        // - `super_defs` 保留原始 `TypeRef`，供参数化超类型在 use-site 做 substitution；
        // - 不包含隐式 `Any`；`Any` 的顶类型语义由 typecheck 单独处理。
        let mut supers: Vec<String> = Vec::new();
        let mut super_defs: Vec<DirectSupertypeInfo> = Vec::new();
        let skip_first_super = matches!(decl.kind, ast::TypeKind::Enum)
            && !decl.supertypes.is_empty()
            && decl.body.as_ref().is_some_and(|body| {
                body.members.iter().any(
                    |m| matches!(m, ast::TypeMember::EnumVariant(v) if v.discriminant.is_some()),
                )
            });

        for st in decl
            .supertypes
            .iter()
            .skip(if skip_first_super { 1 } else { 0 })
        {
            if let Some(st_fqn) = index.type_ref_to_fqn_in_file(source, file, &st.ty) {
                supers.push(st_fqn.clone());
                super_defs.push(DirectSupertypeInfo {
                    fqn: st_fqn,
                    ty: st.ty.clone(),
                });
            }
        }
        supers.sort();
        supers.dedup();
        if !supers.is_empty() {
            self.supertypes.insert(fqn.clone(), supers);
        }
        if !super_defs.is_empty() {
            self.supertype_defs.insert(fqn.clone(), super_defs);
        }

        let Some(body) = &decl.body else {
            if matches!(decl.kind, ast::TypeKind::Enum) {
                self.enums.insert(
                    fqn.clone(),
                    EnumDecl {
                        decl_file: source.path().to_path_buf(),
                        type_params,
                        variants: Vec::new(),
                    },
                );
            }
            return Ok(());
        };

        if matches!(decl.kind, ast::TypeKind::Enum) {
            let mut variants: Vec<EnumVariantInfo> = Vec::new();
            let mut seen_variants: HashMap<String, Span> = HashMap::new();

            for member in &body.members {
                let ast::TypeMember::EnumVariant(v) = member else {
                    continue;
                };

                let variant_name = source.slice(v.name.span).to_string();
                if let Some(prev) = seen_variants.get(&variant_name).copied() {
                    return Err(TypeEnvError::DuplicateEnumVariant {
                        enum_fqn: fqn.clone(),
                        variant: variant_name,
                        first: prev.into(),
                        second: v.name.span.into(),
                    });
                }
                seen_variants.insert(variant_name.clone(), v.name.span);

                let mut fields: Vec<EnumVariantField> = Vec::new();
                let mut seen_fields: HashMap<String, Span> = HashMap::new();
                for p in &v.params {
                    let field_name = source.slice(p.name.span).to_string();
                    if let Some(prev) = seen_fields.get(&field_name).copied() {
                        return Err(TypeEnvError::DuplicateEnumVariantField {
                            enum_fqn: fqn.clone(),
                            variant: variant_name.clone(),
                            field: field_name,
                            first: prev.into(),
                            second: p.name.span.into(),
                        });
                    }
                    seen_fields.insert(field_name.clone(), p.name.span);

                    let Some(ty) = &p.ty else {
                        continue;
                    };
                    fields.push(EnumVariantField {
                        name: field_name,
                        span: p.name.span,
                        ty: ty.clone(),
                    });
                }

                variants.push(EnumVariantInfo {
                    tag: u32::try_from(variants.len()).unwrap_or(u32::MAX),
                    name: variant_name,
                    span: v.span,
                    fields,
                });
            }

            self.enums.insert(
                fqn.clone(),
                EnumDecl {
                    decl_file: source.path().to_path_buf(),
                    type_params,
                    variants,
                },
            );
        }

        if let Some(primary_ctor) = &decl.primary_ctor {
            for param in &primary_ctor.params {
                if param.kind.is_none() {
                    continue;
                }
                let property_fqn = join_prefix(&fqn, source.slice(param.name.span));
                self.record_value_deprecation(
                    source,
                    &property_fqn,
                    &param.annotations,
                    AnnotationTargetKind::Property,
                );
            }
        }

        for member in &body.members {
            match member {
                ast::TypeMember::Type(nested) => {
                    self.collect_type_decl(source, file, &fqn, nested, index)?;
                }
                ast::TypeMember::Object(obj) => {
                    self.collect_object_decl(source, file, &fqn, obj, index)?;
                }
                ast::TypeMember::Property(prop) => {
                    let property_fqn = join_prefix(&fqn, source.slice(prop.name.span));
                    self.record_value_deprecation(
                        source,
                        &property_fqn,
                        &prop.annotations,
                        AnnotationTargetKind::Property,
                    );
                }
                ast::TypeMember::Fun(fun) => {
                    self.record_fun_deprecation(
                        source,
                        fun.name.span,
                        &fun.annotations,
                        AnnotationTargetKind::Function,
                    );
                }
                ast::TypeMember::EnumVariant(_)
                | ast::TypeMember::InitBlock(_)
                | ast::TypeMember::SecondaryCtor(_) => {}
            }
        }

        Ok(())
    }

    fn collect_object_decl(
        &mut self,
        source: &SourceFile,
        file: &ast::File,
        prefix: &str,
        obj: &ast::ObjectDecl,
        index: &Index,
    ) -> Result<(), TypeEnvError> {
        let (name, name_span) = match &obj.name {
            Some(name) => (source.slice(name.span).to_string(), name.span),
            None => match obj.kind {
                ast::ObjectKind::Companion => ("Companion".to_string(), obj.span),
                ast::ObjectKind::Object => {
                    // parser 会拒绝 `object { ... }` 这类非法语法；这里作为防御性兜底忽略。
                    return Ok(());
                }
            },
        };

        // Kotlin-like：object 在 type 层面表现为一个“无构造器的 class-like nominal type”。
        // 说明：真实的初始化时机、存储与 codegen/runtime 语义留给后续阶段处理。
        let fqn = join_prefix(prefix, &name);
        self.insert_symbol(
            fqn.clone(),
            TypeSymbol {
                kind: TypeSymbolKind::Nominal(ast::TypeKind::Class),
                is_sealed_interface: false,
                is_annotation_class: false,
                annotation_targets: None,
                annotation_retention: None,
                annotation_params: Vec::new(),
                type_param_count: 0,
                eff_param: None,
                type_param_names: Vec::new(),
                type_param_variances: Vec::new(),
                where_constraints: Vec::new(),
                span: name_span,
                decl_file: source.path().to_path_buf(),
            },
        )?;
        self.record_type_deprecation(source, &fqn, &obj.annotations, AnnotationTargetKind::Type);
        self.record_value_deprecation(source, &fqn, &obj.annotations, AnnotationTargetKind::Type);

        // 记录 direct supertypes（与 nominal type 一致；不包含隐式 `Any`）。
        let mut supers: Vec<String> = Vec::new();
        let mut super_defs: Vec<DirectSupertypeInfo> = Vec::new();
        for st in &obj.supertypes {
            if let Some(st_fqn) = index.type_ref_to_fqn_in_file(source, file, &st.ty) {
                supers.push(st_fqn.clone());
                super_defs.push(DirectSupertypeInfo {
                    fqn: st_fqn,
                    ty: st.ty.clone(),
                });
            }
        }
        supers.sort();
        supers.dedup();
        if !supers.is_empty() {
            self.supertypes.insert(fqn.clone(), supers);
        }
        if !super_defs.is_empty() {
            self.supertype_defs.insert(fqn.clone(), super_defs);
        }

        let Some(body) = &obj.body else {
            return Ok(());
        };

        for member in &body.members {
            match member {
                ast::TypeMember::Type(nested) => {
                    self.collect_type_decl(source, file, &fqn, nested, index)?;
                }
                ast::TypeMember::Object(nested) => {
                    self.collect_object_decl(source, file, &fqn, nested, index)?;
                }
                ast::TypeMember::Property(prop) => {
                    let property_fqn = join_prefix(&fqn, source.slice(prop.name.span));
                    self.record_value_deprecation(
                        source,
                        &property_fqn,
                        &prop.annotations,
                        AnnotationTargetKind::Property,
                    );
                }
                ast::TypeMember::Fun(fun) => {
                    self.record_fun_deprecation(
                        source,
                        fun.name.span,
                        &fun.annotations,
                        AnnotationTargetKind::Function,
                    );
                }
                ast::TypeMember::EnumVariant(_)
                | ast::TypeMember::InitBlock(_)
                | ast::TypeMember::SecondaryCtor(_) => {}
            }
        }

        Ok(())
    }

    fn check_sealed_interface_decl_shape(
        &self,
        source: &SourceFile,
        fqn: &str,
        decl: &ast::TypeDecl,
    ) -> Result<(), TypeEnvError> {
        if !source.is_trusted_syslib() {
            return Err(TypeEnvError::SealedInterfaceUserDefinitionNotAllowed {
                fqn: fqn.to_string(),
                span: decl.name.span.into(),
            });
        }

        for st in &decl.supertypes {
            if st.ctor_args_span.is_some() {
                return Err(TypeEnvError::SealedInterfaceSupertypeMustBeSealed {
                    fqn: fqn.to_string(),
                    super_fqn: source.slice(st.ty.span()).to_string(),
                    span: st.ty.span().into(),
                });
            }
        }

        if let Some(body) = &decl.body
            && let Some(member) = body.members.first()
        {
            return Err(TypeEnvError::SealedInterfaceMustBeEmpty {
                fqn: fqn.to_string(),
                span: type_member_span(member).into(),
            });
        }

        Ok(())
    }

    fn rebuild_sealed_marker_metadata(&mut self) -> Result<(), TypeEnvError> {
        self.sealed_markers.clear();
        let mut markers = self
            .by_fqn
            .iter()
            .filter_map(|(fqn, sym)| sym.is_sealed_interface.then_some(fqn.clone()))
            .collect::<Vec<_>>();
        markers.sort();

        let marker_set = markers.iter().cloned().collect::<HashSet<_>>();
        let mut states: HashMap<String, SealedMarkerVisitState> = HashMap::new();
        let mut stack: Vec<String> = Vec::new();

        for marker in markers {
            self.compute_sealed_marker_closure(&marker, &marker_set, &mut states, &mut stack)?;
        }
        Ok(())
    }

    fn compute_sealed_marker_closure(
        &mut self,
        marker: &str,
        marker_set: &HashSet<String>,
        states: &mut HashMap<String, SealedMarkerVisitState>,
        stack: &mut Vec<String>,
    ) -> Result<Vec<String>, TypeEnvError> {
        match states.get(marker).copied() {
            Some(SealedMarkerVisitState::Done) => {
                return Ok(self
                    .sealed_markers
                    .get(marker)
                    .map(|info| info.transitive_supers.clone())
                    .unwrap_or_default());
            }
            Some(SealedMarkerVisitState::Visiting) => {
                let mut cycle_parts = stack.clone();
                cycle_parts.push(marker.to_string());
                let cycle = cycle_parts.join(" -> ");
                let span = self
                    .type_symbol(marker)
                    .map(|sym| sym.span)
                    .unwrap_or_else(|| Span::new(0, 0));
                return Err(TypeEnvError::SealedInterfaceInheritanceCycle {
                    cycle,
                    span: span.into(),
                });
            }
            None => {}
        }

        states.insert(marker.to_string(), SealedMarkerVisitState::Visiting);
        stack.push(marker.to_string());

        let direct_infos = self
            .direct_supertype_infos(marker)
            .map(|infos| infos.to_vec())
            .unwrap_or_default();
        let mut direct_supers = Vec::new();
        let mut transitive_supers = Vec::new();

        for info in direct_infos {
            if !marker_set.contains(&info.fqn) {
                return Err(TypeEnvError::SealedInterfaceSupertypeMustBeSealed {
                    fqn: marker.to_string(),
                    super_fqn: info.fqn,
                    span: info.ty.span().into(),
                });
            }

            if let Some(pos) = stack.iter().position(|fqn| fqn == &info.fqn) {
                let mut cycle = stack[pos..].to_vec();
                cycle.push(info.fqn.clone());
                return Err(TypeEnvError::SealedInterfaceInheritanceCycle {
                    cycle: cycle.join(" -> "),
                    span: info.ty.span().into(),
                });
            }

            direct_supers.push(info.fqn.clone());
            transitive_supers.push(info.fqn.clone());
            transitive_supers
                .extend(self.compute_sealed_marker_closure(&info.fqn, marker_set, states, stack)?);
        }

        direct_supers.sort();
        direct_supers.dedup();
        transitive_supers.sort();
        transitive_supers.dedup();

        self.check_sealed_marker_mutual_exclusion(marker, &transitive_supers)?;

        self.sealed_markers.insert(
            marker.to_string(),
            SealedMarkerInfo {
                direct_supers,
                transitive_supers: transitive_supers.clone(),
            },
        );
        states.insert(marker.to_string(), SealedMarkerVisitState::Done);
        let _ = stack.pop();
        Ok(transitive_supers)
    }

    fn check_sealed_marker_mutual_exclusion(
        &self,
        marker: &str,
        transitive_supers: &[String],
    ) -> Result<(), TypeEnvError> {
        let has_any_ref = marker == ANY_REF_MARKER_FQN
            || transitive_supers
                .iter()
                .any(|fqn| fqn == ANY_REF_MARKER_FQN);
        let has_any_value = marker == ANY_VALUE_MARKER_FQN
            || transitive_supers
                .iter()
                .any(|fqn| fqn == ANY_VALUE_MARKER_FQN);

        if has_any_ref && has_any_value {
            let span = self
                .type_symbol(marker)
                .map(|sym| sym.span)
                .unwrap_or_else(|| Span::new(0, 0));
            return Err(TypeEnvError::SealedInterfaceMutuallyExclusiveBound {
                fqn: marker.to_string(),
                span: span.into(),
            });
        }

        Ok(())
    }

    fn insert_symbol(&mut self, fqn: String, symbol: TypeSymbol) -> Result<(), TypeEnvError> {
        if let Some(prev) = self.by_fqn.get(&fqn) {
            return Err(TypeEnvError::DuplicateTypeSymbol {
                fqn,
                first: prev.span.into(),
                second: symbol.span.into(),
            });
        }
        self.by_fqn.insert(fqn, symbol);
        Ok(())
    }

    fn record_type_deprecation(
        &mut self,
        source: &SourceFile,
        fqn: &str,
        annotations: &[ast::AnnotationUse],
        primary_target: AnnotationTargetKind,
    ) {
        let Some(info) = extract_builtin_deprecated_info(source, annotations, primary_target)
        else {
            return;
        };
        self.deprecated_types.entry(fqn.to_string()).or_insert(info);
    }

    fn record_value_deprecation(
        &mut self,
        source: &SourceFile,
        fqn: &str,
        annotations: &[ast::AnnotationUse],
        primary_target: AnnotationTargetKind,
    ) {
        let Some(info) = extract_builtin_deprecated_info(source, annotations, primary_target)
        else {
            return;
        };
        self.deprecated_values
            .entry(fqn.to_string())
            .or_insert(info);
    }

    fn record_fun_deprecation(
        &mut self,
        source: &SourceFile,
        decl_span: Span,
        annotations: &[ast::AnnotationUse],
        primary_target: AnnotationTargetKind,
    ) {
        let Some(info) = extract_builtin_deprecated_info(source, annotations, primary_target)
        else {
            return;
        };
        self.deprecated_funs
            .entry(DeprecatedDeclKey {
                decl_file: source.path().to_path_buf(),
                decl_span,
            })
            .or_insert(info);
    }
}

fn extract_annotation_class_meta(
    source: &SourceFile,
    file: &ast::File,
    decl: &ast::TypeDecl,
    index: &Index,
) -> (
    Option<Vec<AnnotationTargetKind>>,
    Option<AnnotationRetentionPolicy>,
) {
    let mut targets: Option<Vec<AnnotationTargetKind>> = None;
    let mut retention: Option<AnnotationRetentionPolicy> = None;

    for ann in &decl.annotations {
        let Some(meta_fqn) = annotation_use_to_fqn(source, file, index, ann) else {
            continue;
        };
        match meta_fqn.as_str() {
            "scoop.core.Target" => {
                let mut out: Vec<AnnotationTargetKind> = Vec::new();
                for arg in &ann.args {
                    let Some((variant_name, _span)) =
                        extract_annotation_target_variant(source, &arg.value)
                    else {
                        continue;
                    };
                    let Some(kind) = AnnotationTargetKind::from_variant_name(&variant_name) else {
                        continue;
                    };
                    if out.contains(&kind) {
                        continue;
                    }
                    out.push(kind);
                }
                targets = Some(out);
            }
            "scoop.core.Retention" => {
                let Some(arg) = ann.args.first() else {
                    continue;
                };
                let Some(text) = extract_string_literal_text(source, &arg.value) else {
                    continue;
                };
                retention = AnnotationRetentionPolicy::parse(text.as_str());
            }
            _ => {}
        }
    }

    (targets, retention)
}

fn collect_annotation_params(
    source: &SourceFile,
    decl: &ast::TypeDecl,
) -> Vec<AnnotationParamInfo> {
    let Some(primary_ctor) = &decl.primary_ctor else {
        return Vec::new();
    };

    let mut out: Vec<AnnotationParamInfo> = Vec::new();
    for p in &primary_ctor.params {
        let Some(ty) = &p.ty else {
            // `typecheck::check_file_headers` 会给出更精确的诊断；这里保持健壮性。
            continue;
        };

        out.push(AnnotationParamInfo {
            name: source.slice(p.name.span).to_string(),
            name_span: p.name.span,
            ty: ty.clone(),
            has_default: p.default_value.is_some(),
        });
    }
    out
}

fn annotation_use_to_fqn(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    ann: &ast::AnnotationUse,
) -> Option<String> {
    let ty = ast::TypeRef::Path(ast::TypePath {
        span: ann.span,
        segments: ann.path.clone(),
        args: Vec::new(),
    });
    index.type_ref_to_fqn_in_file(source, file, &ty)
}

fn extract_annotation_target_variant(
    source: &SourceFile,
    expr: &ast::Expr,
) -> Option<(String, Span)> {
    let mut segs: Vec<(String, Span)> = Vec::new();
    if !collect_member_access_path(source, expr, &mut segs) {
        return None;
    }
    if segs.len() < 2 {
        return None;
    }

    // 允许：`AnnotationTarget.Field` / `scoop.core.AnnotationTarget.Field`
    let penultimate = segs.get(segs.len().saturating_sub(2))?.0.as_str();
    if penultimate != "AnnotationTarget" {
        return None;
    }
    segs.last().cloned()
}

fn collect_member_access_path(
    source: &SourceFile,
    expr: &ast::Expr,
    out: &mut Vec<(String, Span)>,
) -> bool {
    match &expr.kind {
        ast::ExprKind::Ident(id) => {
            out.push((source.slice(id.span).to_string(), id.span));
            true
        }
        ast::ExprKind::MemberAccess { receiver, member } => {
            if !collect_member_access_path(source, receiver, out) {
                return false;
            }
            out.push((source.slice(member.span).to_string(), member.span));
            true
        }
        _ => false,
    }
}

fn extract_builtin_deprecated_info(
    source: &SourceFile,
    annotations: &[ast::AnnotationUse],
    primary_target: AnnotationTargetKind,
) -> Option<DeprecatedAnnotationInfo> {
    for ann in annotations {
        if builtin_annotation_kind(source, ann) != Some(BuiltinAnnotationKind::Deprecated) {
            continue;
        }
        if effective_annotation_target(source, ann, primary_target) != primary_target {
            continue;
        }
        if let Ok(info) = parse_deprecated_annotation(source, ann) {
            return Some(info);
        }
    }
    None
}

fn effective_annotation_target(
    source: &SourceFile,
    ann: &ast::AnnotationUse,
    primary: AnnotationTargetKind,
) -> AnnotationTargetKind {
    let Some(target) = ann.use_site_target.as_ref() else {
        return primary;
    };
    match source.slice(target.span) {
        "file" => AnnotationTargetKind::Module,
        "property" => AnnotationTargetKind::Property,
        "field" => AnnotationTargetKind::Field,
        "param" => AnnotationTargetKind::Param,
        "get" | "set" => AnnotationTargetKind::Property,
        _ => primary,
    }
}

fn type_member_span(member: &ast::TypeMember) -> Span {
    match member {
        ast::TypeMember::EnumVariant(v) => v.span,
        ast::TypeMember::Property(p) => p.span,
        ast::TypeMember::InitBlock(i) => i.span,
        ast::TypeMember::SecondaryCtor(c) => c.span,
        ast::TypeMember::Fun(f) => f.span,
        ast::TypeMember::Type(t) => t.span,
        ast::TypeMember::Object(o) => o.span,
    }
}

fn extract_string_literal_text(source: &SourceFile, expr: &ast::Expr) -> Option<String> {
    if !matches!(expr.kind, ast::ExprKind::StringLit) {
        return None;
    }
    // 当前 AST 的 StringLit 仅保留 span；这里做最小切片解析即可满足 `"cone"` / `"comptime"`。
    let raw = source.slice(expr.span);
    let s = raw
        .strip_prefix("\"\"\"")
        .and_then(|t| t.strip_suffix("\"\"\""))
        .or_else(|| raw.strip_prefix('\"').and_then(|t| t.strip_suffix('\"')))
        .unwrap_or(raw);
    Some(s.to_string())
}

fn package_prefix(source: &SourceFile, pkg: Option<&ast::PackageDecl>) -> String {
    let Some(pkg) = pkg else {
        return String::new();
    };
    pkg.path
        .iter()
        .map(|id| source.slice(id.span))
        .collect::<Vec<_>>()
        .join(".")
}

fn join_prefix(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{prefix}.{name}")
    }
}

fn build_import_table_best_effort(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
) -> ImportTable {
    let mut table = ImportTable::default();
    add_auto_prelude_star_imports(&mut table);

    for import in &file.imports {
        let path = import
            .path
            .iter()
            .map(|id| source.slice(id.span))
            .collect::<Vec<_>>()
            .join(".");

        if import.has_star {
            table.star.push(path);
            continue;
        }

        let local = import
            .alias
            .as_ref()
            .map(|id| source.slice(id.span))
            .or_else(|| import.path.last().map(|id| source.slice(id.span)))
            .unwrap_or("")
            .to_string();

        // 只把确实存在且在当前文件可见的 type symbol 写入 type 命名空间的显式 import 表。
        if let Some(syms) = index.by_fqn.get(&path)
            && let Some(sym) = syms.ty.as_ref()
            && is_symbol_visible_from(index.cone_of_source(source), source, sym)
        {
            table.ty.explicit.entry(local).or_default().push(path);
        }
    }

    // 稳定化（便于 Debug/测试 & 未来可能的缓存命中）。
    table.star.sort();
    table.star.dedup();
    for v in table.ty.explicit.values_mut() {
        v.sort();
        v.dedup();
    }

    table
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast;
    use crate::session::Session;
    use crate::source::SourceOrigin;
    use crate::ty::TypeStore;
    use crate::typecheck;

    #[test]
    fn sysroot_type_env_contains_option_arity() {
        let sess = Session::new().unwrap();
        let mut pairs: Vec<(&SourceFile, &ast::File)> = Vec::new();
        for f in sess.sysroot().index_files() {
            pairs.push((&f.source, &f.ast));
        }
        let index = Index::build(&pairs).unwrap();
        let env = TypeEnv::from_sysroot(sess.sysroot(), &index).unwrap();

        assert_eq!(env.type_param_count("scoop.core.Option"), Some(1));
    }

    #[test]
    fn type_env_import_context_filters_internal_types_by_cone() {
        let dep = SourceFile::new_virtual(
            "/tmp/scoop-type-env-dep.scoop",
            "package dep\ninternal struct Hidden {}\npublic struct Shown {}\n",
        );
        let app = SourceFile::new_virtual(
            "/tmp/scoop-type-env-app.scoop",
            "package app\nimport dep.Hidden\nimport dep.Shown\nfun main() {}\n",
        );
        let dep_ast = crate::parser::parse_file(&dep).unwrap();
        let app_ast = crate::parser::parse_file(&app).unwrap();
        let index = Index::build_with_cones(&[
            crate::resolve::IndexedFile {
                cone: crate::resolve::ConeId::new(2),
                cone_kind: crate::cone::ConeKind::Lib,
                source: &dep,
                file: &dep_ast,
            },
            crate::resolve::IndexedFile {
                cone: crate::resolve::ConeId::new(1),
                cone_kind: crate::cone::ConeKind::Bin,
                source: &app,
                file: &app_ast,
            },
        ])
        .unwrap();

        let mut env = TypeEnv::default();
        env.extend_from_file(&dep, &dep_ast, &index).unwrap();
        env.extend_from_file(&app, &app_ast, &index).unwrap();
        let app_ctx = env.file_type_context(app.path()).unwrap();

        assert_eq!(app_ctx.cone.kind, crate::cone::ConeKind::Bin);
        assert!(!app_ctx.imports.ty.explicit.contains_key("Hidden"));
        assert_eq!(
            app_ctx.imports.ty.explicit.get("Shown"),
            Some(&vec!["dep.Shown".to_string()])
        );
        assert_eq!(
            env.type_symbol_owner_cone_info("dep.Hidden"),
            Some(crate::resolve::ConeInfo {
                id: crate::resolve::ConeId::new(2),
                kind: crate::cone::ConeKind::Lib,
            })
        );
    }

    #[test]
    fn sysroot_type_env_contains_anyref_anyvalue_markers() {
        let sess = Session::new().unwrap();
        let mut pairs: Vec<(&SourceFile, &ast::File)> = Vec::new();
        for f in sess.sysroot().index_files() {
            pairs.push((&f.source, &f.ast));
        }
        let index = Index::build(&pairs).unwrap();
        let env = TypeEnv::from_sysroot(sess.sysroot(), &index).unwrap();

        for fqn in [ANY_REF_MARKER_FQN, ANY_VALUE_MARKER_FQN] {
            let sym = env
                .type_symbol(fqn)
                .expect("sysroot marker should be registered");
            assert_eq!(sym.kind, TypeSymbolKind::Nominal(ast::TypeKind::Interface));
            assert!(sym.is_sealed_interface);
            assert_eq!(sym.type_param_count, 0);
            assert!(
                env.sealed_marker_direct_supers(fqn)
                    .is_some_and(|supers| supers.is_empty())
            );
            assert!(
                env.sealed_marker_transitive_supers(fqn)
                    .is_some_and(|supers| supers.is_empty())
            );
            assert!(env.direct_supertypes(fqn).is_none());
        }

        assert!(!env.sealed_marker_implies(ANY_REF_MARKER_FQN, ANY_VALUE_MARKER_FQN));
        assert!(!env.sealed_marker_implies(ANY_VALUE_MARKER_FQN, ANY_REF_MARKER_FQN));
        for (fqn, supers) in &env.supertypes {
            assert!(
                !supers
                    .iter()
                    .any(|st| st == ANY_REF_MARKER_FQN || st == ANY_VALUE_MARKER_FQN),
                "{fqn} should not record sealed markers as runtime supertypes"
            );
        }
    }

    #[test]
    fn sysroot_type_env_contains_refcell_box_classes() {
        let sess = Session::new().unwrap();
        let mut pairs: Vec<(&SourceFile, &ast::File)> = Vec::new();
        for f in sess.sysroot().index_files() {
            pairs.push((&f.source, &f.ast));
        }
        let index = Index::build(&pairs).unwrap();
        let env = TypeEnv::from_sysroot(sess.sysroot(), &index).unwrap();

        for fqn in ["scoop.core.RefCell", "scoop.core.Box"] {
            let sym = env
                .type_symbol(fqn)
                .expect("sysroot shared-state class should be registered");
            assert_eq!(sym.kind, TypeSymbolKind::Nominal(ast::TypeKind::Class));
            assert!(!sym.is_sealed_interface);
            assert_eq!(sym.type_param_count, 1);
        }
    }

    #[test]
    fn sysroot_type_env_allows_anyref_anyvalue_generic_bounds() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<sealed-marker-user>",
            r#"
package fixtures.typecheck

import scoop.core.*

class RefBox<T> where T: AnyRef {}
class ValueBox<T> where T: AnyValue {}
class C()
struct S(val value: Int)
typealias RefOk = RefBox<C>
typealias ValueStructOk = ValueBox<S>
typealias ValueIntOk = ValueBox<Int>
"#,
        );
        let ast = sess.parse(&source).unwrap();
        let mut pairs: Vec<(&SourceFile, &ast::File)> = Vec::new();
        for f in sess.sysroot().index_files() {
            pairs.push((&f.source, &f.ast));
        }
        pairs.push((&source, &ast));
        let index = Index::build(&pairs).unwrap();

        let mut env = TypeEnv::from_sysroot(sess.sysroot(), &index).unwrap();
        env.extend_from_file(&source, &ast, &index).unwrap();

        let imports = env
            .file_type_context(source.path())
            .unwrap()
            .imports
            .clone();
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        typecheck::check_file_type_refs(
            &source, &ast, &index, &imports, &env, &mut types, builtins,
        )
        .unwrap();
    }

    #[test]
    fn sysroot_type_env_collects_option_variants() {
        let sess = Session::new().unwrap();
        let mut pairs: Vec<(&SourceFile, &ast::File)> = Vec::new();
        for f in sess.sysroot().index_files() {
            pairs.push((&f.source, &f.ast));
        }
        let index = Index::build(&pairs).unwrap();
        let env = TypeEnv::from_sysroot(sess.sysroot(), &index).unwrap();

        let decl = env
            .enum_decl("scoop.core.Option")
            .expect("Option 应当是 enum");
        let source = env
            .source(&decl.decl_file)
            .expect("sysroot source 应当已被收集进 TypeEnv");

        assert_eq!(decl.type_params, vec!["T".to_string()]);
        assert_eq!(decl.variants.len(), 2);

        let some = &decl.variants[0];
        assert_eq!(some.name, "Some");
        assert_eq!(some.tag, 0);
        assert_eq!(some.fields.len(), 1);
        assert_eq!(some.fields[0].name, "value");
        match &some.fields[0].ty {
            ast::TypeRef::Path(p) => {
                assert_eq!(p.segments.len(), 1);
                assert_eq!(source.slice(p.segments[0].span), "T");
            }
            other => panic!("Option.Some.value: 期望 TypeRef::Path，但得到 {other:?}"),
        }

        let none = &decl.variants[1];
        assert_eq!(none.name, "None");
        assert_eq!(none.tag, 1);
        assert!(none.fields.is_empty());
    }

    #[test]
    fn type_env_collects_where_constraints_for_type_decl() {
        let sess = Session::new().unwrap();
        let src = SourceFile::new_virtual(
            "<mem>",
            r#"
package fixtures.typecheck

interface Show {}

struct Bad {}

class Box<T> where T: Show {}
"#,
        );
        let ast = sess.parse(&src).unwrap();

        let mut pairs: Vec<(&SourceFile, &ast::File)> = Vec::new();
        for f in sess.sysroot().index_files() {
            pairs.push((&f.source, &f.ast));
        }
        pairs.push((&src, &ast));

        let index = Index::build(&pairs).unwrap();

        let mut env = TypeEnv::from_sysroot(sess.sysroot(), &index).unwrap();
        env.extend_from_file(&src, &ast, &index).unwrap();

        let sym = env
            .type_symbol("fixtures.typecheck.Box")
            .expect("Box 应当被收集进 TypeEnv");
        assert_eq!(sym.type_param_names, vec!["T".to_string()]);
        assert_eq!(sym.where_constraints.len(), 1);
        assert_eq!(sym.where_constraints[0].param_index, 0);

        match &sym.where_constraints[0].bound {
            ast::TypeRef::Path(p) => {
                assert_eq!(p.segments.len(), 1);
                assert_eq!(src.slice(p.segments[0].span), "Show");
            }
            other => panic!("where bound: 期望 TypeRef::Path，但得到 {other:?}"),
        }
    }

    fn parse_virtual(
        sess: &Session,
        path: &str,
        text: &str,
        origin: SourceOrigin,
    ) -> (SourceFile, ast::File) {
        let source = if origin == SourceOrigin::Sysroot {
            SourceFile::new_virtual_trusted_syslib(path, text)
        } else {
            SourceFile::new_virtual_with_origin(path, text, origin)
        };
        let ast = sess.parse(&source).unwrap();
        (source, ast)
    }

    #[test]
    fn sealed_marker_type_env_records_metadata_and_closure() {
        let sess = Session::new().unwrap();
        let (source, ast) = parse_virtual(
            &sess,
            "<sealed-sysroot>",
            r#"
package scoop.core

sealed interface AnyRef
sealed interface AnyValue
sealed interface RefMarker : AnyRef
sealed interface DeepRefMarker : RefMarker
"#,
            SourceOrigin::Sysroot,
        );
        let pairs = vec![(&source, &ast)];
        let index = Index::build(&pairs).unwrap();

        let mut env = TypeEnv::default();
        env.extend_from_file(&source, &ast, &index).unwrap();

        assert!(env.is_sealed_interface("scoop.core.AnyRef"));
        assert_eq!(
            env.sealed_marker_direct_supers("scoop.core.DeepRefMarker"),
            Some(["scoop.core.RefMarker".to_string()].as_slice())
        );
        assert_eq!(
            env.sealed_marker_transitive_supers("scoop.core.DeepRefMarker"),
            Some(
                [
                    "scoop.core.AnyRef".to_string(),
                    "scoop.core.RefMarker".to_string(),
                ]
                .as_slice()
            )
        );
    }

    #[test]
    fn sealed_marker_type_env_rejects_user_definition() {
        let sess = Session::new().unwrap();
        let (source, ast) = parse_virtual(
            &sess,
            "<sealed-user>",
            r#"
package fixtures.typecheck

sealed interface UserMarker
"#,
            SourceOrigin::User,
        );
        let pairs = vec![(&source, &ast)];
        let index = Index::build(&pairs).unwrap();

        let mut env = TypeEnv::default();
        let err = env.extend_from_file(&source, &ast, &index).unwrap_err();
        assert!(matches!(
            err,
            TypeEnvError::SealedInterfaceUserDefinitionNotAllowed { .. }
        ));
    }

    #[test]
    fn sealed_marker_type_env_rejects_non_empty_body() {
        let sess = Session::new().unwrap();
        let (source, ast) = parse_virtual(
            &sess,
            "<sealed-sysroot>",
            r#"
package scoop.core

sealed interface Bad {
    fun f(): Unit
}
"#,
            SourceOrigin::Sysroot,
        );
        let pairs = vec![(&source, &ast)];
        let index = Index::build(&pairs).unwrap();

        let mut env = TypeEnv::default();
        let err = env.extend_from_file(&source, &ast, &index).unwrap_err();
        assert!(matches!(
            err,
            TypeEnvError::SealedInterfaceMustBeEmpty { .. }
        ));
    }

    #[test]
    fn sealed_marker_type_env_rejects_non_sealed_supertype_and_cycles() {
        let sess = Session::new().unwrap();
        let (bad_super_source, bad_super_ast) = parse_virtual(
            &sess,
            "<sealed-bad-super>",
            r#"
package scoop.core

interface Normal
sealed interface Bad : Normal
"#,
            SourceOrigin::Sysroot,
        );
        let bad_pairs = vec![(&bad_super_source, &bad_super_ast)];
        let bad_index = Index::build(&bad_pairs).unwrap();
        let mut bad_env = TypeEnv::default();
        let bad_err = bad_env
            .extend_from_file(&bad_super_source, &bad_super_ast, &bad_index)
            .unwrap_err();
        assert!(matches!(
            bad_err,
            TypeEnvError::SealedInterfaceSupertypeMustBeSealed { .. }
        ));

        let (cycle_source, cycle_ast) = parse_virtual(
            &sess,
            "<sealed-cycle>",
            r#"
package scoop.core

sealed interface A : B
sealed interface B : A
"#,
            SourceOrigin::Sysroot,
        );
        let cycle_pairs = vec![(&cycle_source, &cycle_ast)];
        let cycle_index = Index::build(&cycle_pairs).unwrap();
        let mut cycle_env = TypeEnv::default();
        let cycle_err = cycle_env
            .extend_from_file(&cycle_source, &cycle_ast, &cycle_index)
            .unwrap_err();
        assert!(matches!(
            cycle_err,
            TypeEnvError::SealedInterfaceInheritanceCycle { .. }
        ));
    }

    #[test]
    fn sealed_marker_type_env_rejects_anyref_anyvalue_marker_mix() {
        let sess = Session::new().unwrap();
        let (source, ast) = parse_virtual(
            &sess,
            "<sealed-mutual>",
            r#"
package scoop.core

sealed interface AnyRef
sealed interface AnyValue
sealed interface Bad : AnyRef, AnyValue
"#,
            SourceOrigin::Sysroot,
        );
        let pairs = vec![(&source, &ast)];
        let index = Index::build(&pairs).unwrap();

        let mut env = TypeEnv::default();
        let err = env.extend_from_file(&source, &ast, &index).unwrap_err();
        assert!(matches!(
            err,
            TypeEnvError::SealedInterfaceMutuallyExclusiveBound { .. }
        ));
    }

    #[test]
    fn sealed_marker_bounds_accept_automatic_ref_and_value_types() {
        let sess = Session::new().unwrap();
        let (marker_source, marker_ast) = parse_virtual(
            &sess,
            "<sealed-markers>",
            r#"
package scoop.core

sealed interface AnyRef
sealed interface AnyValue
"#,
            SourceOrigin::Sysroot,
        );
        let (user_source, user_ast) = parse_virtual(
            &sess,
            "<sealed-user>",
            r#"
package fixtures.typecheck

import scoop.core.*

class RefBox<T> where T: AnyRef {}
class ValueBox<T> where T: AnyValue {}
class C()
struct S(val value: Int)
typealias RefOk = RefBox<C>
typealias ValueOk = ValueBox<S>
"#,
            SourceOrigin::User,
        );
        let pairs = vec![(&marker_source, &marker_ast), (&user_source, &user_ast)];
        let index = Index::build(&pairs).unwrap();

        let mut env = TypeEnv::default();
        env.extend_from_file(&marker_source, &marker_ast, &index)
            .unwrap();
        env.extend_from_file(&user_source, &user_ast, &index)
            .unwrap();

        let imports = env
            .file_type_context(user_source.path())
            .unwrap()
            .imports
            .clone();
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        typecheck::check_file_type_refs(
            &user_source,
            &user_ast,
            &index,
            &imports,
            &env,
            &mut types,
            builtins,
        )
        .unwrap();
    }

    #[test]
    fn sealed_marker_non_bound_type_position_is_rejected() {
        let sess = Session::new().unwrap();
        let (marker_source, marker_ast) = parse_virtual(
            &sess,
            "<sealed-markers>",
            r#"
package scoop.core

sealed interface AnyRef
sealed interface AnyValue
"#,
            SourceOrigin::Sysroot,
        );
        let (user_source, user_ast) = parse_virtual(
            &sess,
            "<sealed-user>",
            r#"
package fixtures.typecheck

import scoop.core.*

typealias Bad = AnyRef
"#,
            SourceOrigin::User,
        );
        let pairs = vec![(&marker_source, &marker_ast), (&user_source, &user_ast)];
        let index = Index::build(&pairs).unwrap();

        let mut env = TypeEnv::default();
        env.extend_from_file(&marker_source, &marker_ast, &index)
            .unwrap();
        env.extend_from_file(&user_source, &user_ast, &index)
            .unwrap();

        let imports = env
            .file_type_context(user_source.path())
            .unwrap()
            .imports
            .clone();
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let err = typecheck::check_file_type_refs(
            &user_source,
            &user_ast,
            &index,
            &imports,
            &env,
            &mut types,
            builtins,
        )
        .unwrap_err();
        assert!(matches!(
            err,
            typecheck::TypeLowerError::SealedInterfaceBoundOnly { .. }
        ));
    }
}
