//! v0 阶段 archive：per-cone AST 文件 + collection 级共享 `TypedHir`/`Interner`。
//!
//! **显式 transitional**（PLAN.md M1）：为最快建立「阶段产出落地 + 下游从文件
//! 驱动 + oracle」的机制，v0 序列化的是前端现状（AST + `TypedHir` 侧表形态），
//! 允许 collection 级共享 arena 段。M2 element 体系落地后由 v1 per-cone archive
//! 替换，本格式整体退役（打通即弃的升级 shim）。
//!
//! 磁盘布局：
//!
//! ```text
//! <dir>/<cone-name>.hirarch    // per cone：版本头 + 该 cone 的用户文件（AST）
//! <dir>/collection.hirv0       // 成员清单（排序）+ 共享 TypedHir（含 interner）
//! ```
//!
//! 装配（[`load_hir_collection`]）是本格式的唯一可失败解析点（PLAN.md C7）：
//! 版本 / 编译器版本不匹配即拒；成员缺失 / 重复 file_id / 多余 archive 均为
//! 装配错误。装配成功后，MIR 侧只消费内存中的重构结果，不再有可失败查询。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use scoop2_base::diag::Diagnostic;
use scoop2_base::{FileId, StableConeKey, archive_fingerprint, archive_schema, compiler_version};
use scoop2_hir::hir::TypedHir;
use scoop2_hir::hir::TypedFile;
use scoop2_hir::resolve::{InputOrigin, cone_name_of};

use crate::pipeline::BuiltProgram;

/// archive 魔数（"Scoop Cone Archive"）。
pub const MAGIC: [u8; 4] = *b"SCPA";

/// 每个 archive 文件的版本头。
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ArchiveHeader {
    pub magic: [u8; 4],
    pub schema_version: u32,
    /// 阶段标签（`hir` / `mir` / `lir`）。
    pub stage: String,
    /// 本 archive 所属 cone 的稳定 key。
    pub cone_key: String,
    pub compiler_version: String,
    /// 输入集合指纹（C7 缓存失效键；HIR 阶段输入为空集 + 全局参数）。
    pub fingerprint: u64,
}

/// 一个用户文件的归档条目（来源元信息——M2-6 起 **不含 AST**：MIR 只消费
/// HIR 树 + 骨架，body 语义在 hir-build 期已 baking 进树）。
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ArchivedFile {
    pub file_id: FileId,
    pub origin: InputOrigin,
    pub trusted: bool,
}

/// per-cone HIR archive（v1：AST 片段移除；per-cone 分区——本 cone 的
/// TypedFile（树 + 骨架 + per-file 表）随 archive 携带，符号/类型表在
/// collection 共享段）。
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HirConeArchive {
    pub header: ArchiveHeader,
    pub files: Vec<ArchivedFile>,
    /// 本 cone 的 typed 文件（per-cone 分区；装配时按 file_id 归并）。
    pub typed_files: Vec<TypedFile>,
}

/// collection 清单 + 共享段（v0）。
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HirCollection {
    /// 成员 cone 稳定 key（升序）。
    pub members: Vec<String>,
    /// 影响产出的全局参数（键值对，按键排序参与指纹）。
    pub params: Vec<(String, String)>,
    /// 共享段：完整 TypedHir（内含 interner 与 per-file 表）。
    pub hir: TypedHir,
}

/// archive 装配 / 读写错误。
#[derive(Debug)]
pub enum ArchiveError {
    Io(PathBuf, std::io::Error),
    Decode(PathBuf, String),
    /// 版本头不匹配（magic / schema / compiler）。
    VersionMismatch {
        path: PathBuf,
        detail: String,
    },
    /// 清单声明的成员文件缺失。
    MissingMember(String),
    /// 目录中存在清单未声明的 archive（闭合性破坏）。
    UnlistedMember(String),
    /// file_id 冲突（同一 FileId 出现在多个文件中）。
    DuplicateFileId(FileId),
}

impl std::fmt::Display for ArchiveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArchiveError::Io(p, e) => write!(f, "archive I/O 失败 {}: {e}", p.display()),
            ArchiveError::Decode(p, e) => write!(f, "archive 解码失败 {}: {e}", p.display()),
            ArchiveError::VersionMismatch { path, detail } => {
                write!(f, "archive 版本不匹配 {}: {detail}", path.display())
            }
            ArchiveError::MissingMember(m) => write!(f, "collection 成员缺失: {m}.hirarch"),
            ArchiveError::UnlistedMember(m) => {
                write!(f, "目录存在清单未声明的 archive: {m}.hirarch")
            }
            ArchiveError::DuplicateFileId(id) => write!(f, "file_id 冲突: {}", id.0),
        }
    }
}

impl std::error::Error for ArchiveError {}

/// staged MIR 阶段错误：装配错误或 MIR 诊断（原始数据，渲染由持源码方负责）。
#[derive(Debug)]
pub enum StageError {
    Archive(ArchiveError),
    /// MIR lowering / 单态化 / verify 失败（合法程序在 staged 路径不应出现；
    /// 出现即 one-shot 与 staged 行为分叉，属回归）。
    Mir(Vec<Diagnostic>),
}

impl std::fmt::Display for StageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StageError::Archive(e) => write!(f, "{e}"),
            StageError::Mir(diags) => {
                write!(f, "staged MIR 产出诊断（与 one-shot 分叉）:")?;
                for d in diags {
                    write!(f, "\n  [{}] {}", d.code, d.message)?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for StageError {}

impl From<ArchiveError> for StageError {
    fn from(e: ArchiveError) -> Self {
        StageError::Archive(e)
    }
}

/// collection 文件名。
pub const COLLECTION_FILE: &str = "collection.hirv0";
/// cone archive 扩展名。
pub const CONE_EXT: &str = "hirarch";

fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, ArchiveError> {
    bincode::serde::encode_to_vec(value, bincode::config::standard())
        .map_err(|e| ArchiveError::Decode(PathBuf::from("<encode>"), e.to_string()))
}

fn write_bytes(path: &Path, bytes: &[u8]) -> Result<(), ArchiveError> {
    std::fs::write(path, bytes).map_err(|e| ArchiveError::Io(path.to_path_buf(), e))
}

/// 把 typecheck 完成的程序按 cone 分区写出为 v0 HIR archive collection。
///
/// 仅归档**用户文件**（origin User）的 AST；sysroot 是预构建依赖（rlib 故事，
/// PLAN.md M6），其符号信息已在共享 `TypedHir` 段中。
///
/// 返回写出的文件列表（manifest 在前，成员按 key 升序）。
pub fn write_hir_collection(
    dir: &Path,
    program: &BuiltProgram,
    hir: &TypedHir,
    params: &[(String, String)],
) -> Result<Vec<PathBuf>, ArchiveError> {
    std::fs::create_dir_all(dir).map_err(|e| ArchiveError::Io(dir.to_path_buf(), e))?;

    // cone key → 该 cone 的用户文件（BTreeMap：成员序确定）。
    let mut by_cone: BTreeMap<String, Vec<ArchivedFile>> = BTreeMap::new();
    for (i, pf) in program.parsed.iter().enumerate() {
        let origin = if program.user_indices.contains(&i) {
            InputOrigin::User
        } else {
            InputOrigin::Sysroot
        };
        if origin != InputOrigin::User {
            continue;
        }
        let cone_key =
            StableConeKey::from_cone_name(&cone_name_of(&pf.file, &program.interner, origin));
        by_cone
            .entry(cone_key.as_str().to_string())
            .or_default()
            .push(ArchivedFile {
                file_id: FileId(i as u32),
                origin,
                trusted: i != 0,
            });
    }

    // per-cone 分区：TypedFile（file_id → 所属 cone）随 cone archive 携带。
    let mut typed_by_cone: BTreeMap<String, Vec<TypedFile>> = BTreeMap::new();
    {
        let mut id_to_cone: std::collections::HashMap<FileId, String> =
            std::collections::HashMap::new();
        for (cone_key, files) in &by_cone {
            for f in files {
                id_to_cone.insert(f.file_id, cone_key.clone());
            }
        }
        for tf in &hir.files {
            let cone = id_to_cone.get(&tf.file_id).cloned().unwrap_or_else(|| {
                // 树未覆盖的文件（sysroot body 双态等）留在共享段——装配时归位。
                String::new()
            });
            if cone.is_empty() {
                continue;
            }
            typed_by_cone.entry(cone).or_default().push(tf.clone());
        }
    }

    let mut written = Vec::new();
    for (cone_key, files) in &by_cone {
        let header = ArchiveHeader {
            magic: MAGIC,
            schema_version: archive_schema::V1,
            stage: "hir".to_string(),
            cone_key: cone_key.clone(),
            compiler_version: compiler_version().to_string(),
            fingerprint: archive_fingerprint(
                archive_schema::V1,
                scoop2_base::ArchiveStage::Hir,
                &StableConeKey::from_cone_name(cone_key),
                std::iter::empty::<&str>(),
                params,
            ),
        };
        let path = dir.join(format!("{cone_key}.{CONE_EXT}"));
        write_bytes(
            &path,
            &encode(&HirConeArchive {
                header,
                files: files.clone(),
                typed_files: typed_by_cone.get(cone_key).cloned().unwrap_or_default(),
            })?,
        )?;
        written.push(path);
    }

    // 共享段剔除已分区文件（v1：共享段只剩符号/类型表 + 未分区残余）。
    let mut shared_hir = hir.clone();
    let partitioned: std::collections::HashSet<FileId> = typed_by_cone
        .values()
        .flat_map(|v| v.iter().map(|tf| tf.file_id))
        .collect();
    shared_hir
        .files
        .retain(|tf| !partitioned.contains(&tf.file_id));
    let collection = HirCollection {
        members: by_cone.keys().cloned().collect(),
        params: params.to_vec(),
        hir: shared_hir,
    };
    let manifest_path = dir.join(COLLECTION_FILE);
    write_bytes(&manifest_path, &encode(&collection)?)?;
    let mut out = vec![manifest_path];
    out.extend(written);
    Ok(out)
}

/// 装配完成的 HIR collection：共享 `TypedHir` + 全部用户文件（按 file_id 升序
/// = 原 parse 序）。
pub struct LoadedCollection {
    pub hir: TypedHir,
    pub files: Vec<ArchivedFile>,
    pub members: Vec<String>,
}

impl std::fmt::Debug for LoadedCollection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoadedCollection")
            .field("members", &self.members)
            .field(
                "file_ids",
                &self.files.iter().map(|f| f.file_id).collect::<Vec<_>>(),
            )
            .finish_non_exhaustive()
    }
}

/// 从目录装配 v0 HIR archive collection。
///
/// 这是本格式的唯一可失败解析点：读 `collection.hirv0` → 逐成员加载并校验
/// 版本头 / cone key → 闭合性检查（成员齐全、无多余）→ file_id 去重 → 按
/// file_id 排序。成功后调用方全程不可失败消费。
pub fn load_hir_collection(dir: &Path) -> Result<LoadedCollection, ArchiveError> {
    let manifest_path = dir.join(COLLECTION_FILE);
    let bytes =
        std::fs::read(&manifest_path).map_err(|e| ArchiveError::Io(manifest_path.clone(), e))?;
    let (collection, _): (HirCollection, usize) =
        bincode::serde::decode_from_slice(&bytes, bincode::config::standard())
            .map_err(|e| ArchiveError::Decode(manifest_path, e.to_string()))?;

    // 目录中实际存在的成员（闭合性检查用）。
    let mut present: Vec<String> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == CONE_EXT) {
                let stem = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or_default()
                    .to_string();
                present.push(stem);
            }
        }
    }
    present.sort_unstable();
    if present != collection.members {
        for m in &collection.members {
            if !present.contains(m) {
                return Err(ArchiveError::MissingMember(m.clone()));
            }
        }
        for m in &present {
            if !collection.members.contains(m) {
                return Err(ArchiveError::UnlistedMember(m.clone()));
            }
        }
    }

    let mut files: Vec<ArchivedFile> = Vec::new();
    let mut cone_typed_files: Vec<TypedFile> = Vec::new();
    let mut hir = collection.hir;
    for member in &collection.members {
        let path = dir.join(format!("{member}.{CONE_EXT}"));
        let bytes = std::fs::read(&path).map_err(|e| ArchiveError::Io(path.clone(), e))?;
        let (archive, _): (HirConeArchive, usize) =
            bincode::serde::decode_from_slice(&bytes, bincode::config::standard())
                .map_err(|e| ArchiveError::Decode(path.clone(), e.to_string()))?;
        if archive.header.magic != MAGIC {
            return Err(ArchiveError::VersionMismatch {
                path,
                detail: "magic 不匹配".to_string(),
            });
        }
        if archive.header.schema_version != archive_schema::V1 {
            return Err(ArchiveError::VersionMismatch {
                path,
                detail: format!(
                    "schema {} ≠ {}（不支持迁移）",
                    archive.header.schema_version,
                    archive_schema::V1
                ),
            });
        }
        if archive.header.compiler_version != compiler_version() {
            return Err(ArchiveError::VersionMismatch {
                path,
                detail: format!(
                    "compiler {} ≠ {}",
                    archive.header.compiler_version,
                    compiler_version()
                ),
            });
        }
        if archive.header.cone_key != *member {
            return Err(ArchiveError::VersionMismatch {
                path,
                detail: format!(
                    "cone key {} 与文件名 {} 不符",
                    archive.header.cone_key, member
                ),
            });
        }
        files.extend(archive.files);
        cone_typed_files.extend(archive.typed_files);
    }

    // file_id 去重 + 排序（恢复原 parse 序）。
    let mut seen = std::collections::HashSet::new();
    for f in &files {
        if !seen.insert(f.file_id) {
            return Err(ArchiveError::DuplicateFileId(f.file_id));
        }
    }
    files.sort_by_key(|f| f.file_id);

    // per-cone 分区归并：TypedFile 按 file_id 升序回到 hir.files（与写出前
    // 的 parse 序一致）。
    {
        let mut seen_t = std::collections::HashSet::new();
        for tf in &cone_typed_files {
            if !seen_t.insert(tf.file_id) {
                return Err(ArchiveError::DuplicateFileId(tf.file_id));
            }
        }
        cone_typed_files.sort_by_key(|tf| tf.file_id);
        hir.files.extend(cone_typed_files);
        hir.files.sort_by_key(|tf| tf.file_id);
    }

    Ok(LoadedCollection {
        hir,
        files,
        members: collection.members,
    })
}

/// 从装配好的 collection 走 MIR：lower → 门禁 → 单态化 → verify → dump。
/// 不读任何源文件（M2-6 起 archive 不含 AST——树 + 骨架即全部输入）。
pub fn mir_dump_from_collection(loaded: &LoadedCollection) -> Result<String, StageError> {
    run_mir_and_dump(&loaded.hir).map(|(dump, _)| dump)
}

/// MIR 阶段核心（one-shot 与 staged 共用同一序列，oracle 据此隔离序列化保真度）：
/// lower → 门禁 → 单态化（entry=main）→ verify（module + materialized）→ dump。
/// M2-5 翻转后只消费 `TypedHir` 的树 + 骨架（AST 不再是 MIR 输入）；
/// M3-6 起附带返回单态化整程序产物（MIR archive 载荷）。
pub fn run_mir_and_dump(
    hir: &TypedHir,
) -> Result<(String, scoop2_mir::mir::materialize::MaterializedMir), StageError> {
    use scoop2_mir::mir;

    let mut lower_diags = scoop2_base::diag::DiagnosticSink::new();
    let lower_result = mir::lower_tree::lower_module_from_trees(&hir, &mut lower_diags);
    if lower_diags.has_errors() || !lower_result.errors.is_empty() {
        return Err(StageError::Mir(lower_diags.into_vec()));
    }

    // 单态化（触发单态化阶段错误检测；dump 输出仍是 generic 模板模块）。
    let entry = lower_result
        .module
        .items
        .iter()
        .find_map(|it| match it {
            mir::Item::Fun(fd) if fd.name == "main" => Some(fd.fqn.clone()),
            _ => None,
        })
        .unwrap_or_else(|| "main".to_string());
    let monomorph = mir::materialize::materialize(lower_result.module.clone(), Some(&entry), hir);
    if let Err(merr) = monomorph {
        lower_diags.push(merr.to_diagnostic());
        return Err(StageError::Mir(lower_diags.into_vec()));
    }

    // 外部符号集（与 one-shot 相同的构造方式；集合成员序无关于结果）。
    let mut external_symbols = std::collections::HashSet::new();
    for (&fqn_sym, _) in &hir.top_level_funs {
        external_symbols.insert(hir.interner.resolve(fqn_sym).to_string());
    }
    for (&type_sym, _) in &hir.enum_variants {
        external_symbols.insert(hir.interner.resolve(type_sym).to_string());
    }
    for (&type_sym, _) in &hir.members {
        external_symbols.insert(hir.interner.resolve(type_sym).to_string());
    }
    for (&type_sym, methods) in &hir.member_funs {
        let type_fqn_text = hir.interner.resolve(type_sym).to_string();
        external_symbols.insert(type_fqn_text.clone());
        for (&method_sym, _) in methods {
            let method_name = hir.interner.resolve(method_sym);
            external_symbols.insert(format!("{type_fqn_text}.{method_name}"));
        }
    }
    for (&val_sym, _) in &hir.top_level_vals {
        external_symbols.insert(hir.interner.resolve(val_sym).to_string());
    }
    let verify_errors =
        mir::verify::verify_module_with_external(&lower_result.module, &external_symbols);
    if !verify_errors.is_empty() {
        for ve in &verify_errors {
            lower_diags.push(Diagnostic::error(ve.code, ve.message.clone()));
        }
        return Err(StageError::Mir(lower_diags.into_vec()));
    }
    let mat_errors = mir::verify::verify_materialized_with_external(
        &monomorph.as_ref().expect("materialize 已成功（错误路径已 return）").module,
        &external_symbols,
    );
    if !mat_errors.is_empty() {
        for ve in &mat_errors {
            lower_diags.push(Diagnostic::error(ve.code, ve.message.clone()));
        }
        return Err(StageError::Mir(lower_diags.into_vec()));
    }

    let dump = mir::dump::dump_module(&lower_result.module, &hir.interner);
    let mat = monomorph.expect("materialize 已成功（错误路径已 return）");
    Ok((dump, mat))
}

// ---------------------------------------------------------------------------
// MIR archive（M3-6）：单态化整程序产物落地
// ---------------------------------------------------------------------------

/// MIR archive（整程序——单态化产物无 per-cone 边界；自包含：interner 随行）。
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MirArchive {
    pub header: ArchiveHeader,
    pub module: scoop2_mir::mir::Module,
    /// 实例化键（发现序）。
    pub instance_keys: Vec<scoop2_mir::mir::materialize::InstanceKey>,
    /// 语言级 backend contracts（分派表/布局唯一来源——M3-3）。
    pub backend_contracts: scoop2_mir::mir::materialize::BackendContracts,
    /// dump 渲染所需的 interner 快照（自包含）。
    pub interner: scoop2_base::Interner,
    /// 声明侧定稿（M4：LIR 输入切换的数据源——成员序/enum variant/class 集合）。
    pub decls: scoop2_mir::mir::decls::MirDecls,
}

/// MIR archive 文件名。
pub const MIR_ARCHIVE_FILE: &str = "module.mirarch";

/// 写 MIR archive：输入指纹 = 参与 HIR collection 的 cone 稳定 key + 全局参数
///（C7/M3-5：新增 cone 声明子类 → 指纹变化 → 旧 MIR archive 失效）。
pub fn write_mir_archive(
    dir: &Path,
    hir: &TypedHir,
    mat: &scoop2_mir::mir::materialize::MaterializedMir,
    members: &[String],
    params: &[(String, String)],
) -> Result<PathBuf, ArchiveError> {
    std::fs::create_dir_all(dir).map_err(|e| ArchiveError::Io(dir.to_path_buf(), e))?;
    let header = ArchiveHeader {
        magic: MAGIC,
        schema_version: archive_schema::V1,
        stage: "mir".to_string(),
        cone_key: "__program__".to_string(),
        compiler_version: compiler_version().to_string(),
        fingerprint: archive_fingerprint(
            archive_schema::V1,
            scoop2_base::ArchiveStage::Mir,
            &StableConeKey::from_cone_name("__program__"),
            members.iter().map(|s| s.as_str()),
            params,
        ),
    };
    let path = dir.join(MIR_ARCHIVE_FILE);
    write_bytes(
        &path,
        &encode(&MirArchive {
            header,
            module: mat.module.clone(),
            instance_keys: mat.instance_keys.clone(),
            backend_contracts: mat.backend_contracts.clone(),
            interner: hir.interner.clone(),
            decls: scoop2_mir::mir::decls::MirDecls::from_hir(hir),
        })?,
    )?;
    Ok(path)
}

/// 读 MIR archive（版本头校验——schema/compiler 不匹配即拒）。
pub fn load_mir_archive(dir: &Path) -> Result<MirArchive, ArchiveError> {
    let path = dir.join(MIR_ARCHIVE_FILE);
    let bytes = std::fs::read(&path).map_err(|e| ArchiveError::Io(path.clone(), e))?;
    let (archive, _): (MirArchive, usize) =
        bincode::serde::decode_from_slice(&bytes, bincode::config::standard())
            .map_err(|e| ArchiveError::Decode(path.clone(), e.to_string()))?;
    if archive.header.magic != MAGIC {
        return Err(ArchiveError::VersionMismatch {
            path,
            detail: "magic 不匹配".to_string(),
        });
    }
    if archive.header.schema_version != archive_schema::V1 {
        return Err(ArchiveError::VersionMismatch {
            path,
            detail: format!(
                "schema {} ≠ {}（不支持迁移）",
                archive.header.schema_version,
                archive_schema::V1
            ),
        });
    }
    if archive.header.compiler_version != compiler_version() {
        return Err(ArchiveError::VersionMismatch {
            path,
            detail: format!(
                "compiler {} ≠ {}",
                archive.header.compiler_version,
                compiler_version()
            ),
        });
    }
    Ok(archive)
}

/// 从 MIR archive 渲染 dump（纯读——不回 HIR archive）。
pub fn dump_from_mir_archive(archive: &MirArchive) -> String {
    scoop2_mir::mir::dump::dump_module(&archive.module, &archive.interner)
}

// ---------------------------------------------------------------------------
// LIR archive（M4-3）：LIR 程序产物落地（自包含）
// ---------------------------------------------------------------------------

/// LIR archive（整程序；自包含：types + interner 随行——不回引用 MIR archive）。
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LirArchive {
    pub header: ArchiveHeader,
    pub program: scoop2_lir::LirProgram,
    /// LIR 自包含的类型段（TypeId 渲染上下文）。
    pub types: scoop2_mir::ty::TypeStore,
    /// 符号文本解析依据。
    pub interner: scoop2_base::Interner,
}

/// LIR archive 文件名。
pub const LIR_ARCHIVE_FILE: &str = "program.lirarch";

/// 写 LIR archive（指纹链 = MIR archive 指纹 + 本阶段参数——级联失效）。
pub fn write_lir_archive(
    dir: &Path,
    lir_program: &scoop2_lir::LirProgram,
    types: &scoop2_mir::ty::TypeStore,
    interner: &scoop2_base::Interner,
    mir_fingerprint: u64,
) -> Result<PathBuf, ArchiveError> {
    std::fs::create_dir_all(dir).map_err(|e| ArchiveError::Io(dir.to_path_buf(), e))?;
    let header = ArchiveHeader {
        magic: MAGIC,
        schema_version: archive_schema::V1,
        stage: "lir".to_string(),
        cone_key: "__program__".to_string(),
        compiler_version: compiler_version().to_string(),
        fingerprint: mir_fingerprint,
    };
    let path = dir.join(LIR_ARCHIVE_FILE);
    write_bytes(
        &path,
        &encode(&LirArchive {
            header,
            program: lir_program.clone(),
            types: types.clone(),
            interner: interner.clone(),
        })?,
    )?;
    Ok(path)
}

/// 读 LIR archive（版本头校验）。
pub fn load_lir_archive(dir: &Path) -> Result<LirArchive, ArchiveError> {
    let path = dir.join(LIR_ARCHIVE_FILE);
    let bytes = std::fs::read(&path).map_err(|e| ArchiveError::Io(path.clone(), e))?;
    let (archive, _): (LirArchive, usize) =
        bincode::serde::decode_from_slice(&bytes, bincode::config::standard())
            .map_err(|e| ArchiveError::Decode(path.clone(), e.to_string()))?;
    if archive.header.magic != MAGIC
        || archive.header.schema_version != archive_schema::V1
        || archive.header.compiler_version != compiler_version()
    {
        return Err(ArchiveError::VersionMismatch {
            path,
            detail: "版本头不匹配（magic/schema/compiler）".to_string(),
        });
    }
    Ok(archive)
}
