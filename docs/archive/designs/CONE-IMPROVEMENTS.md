# CONE 工程化改进设计（目录结构 / 构建产物 / profile / 增量构建）

> 生成时间：2026-04-07  
> 目的：把 CONE 项目从“能跑”提升到“可持续开发”：目录结构清晰、构建产物可预测、`build/run` 行为完整，并为未来 cross compilation 与 incremental build 预留空间。

## 0. 设计目标与非目标

### 0.1 目标（Goals）

1. **目录结构可预测**：`scoop new` 生成的项目结构统一，并包含必要的 `.gitignore`。
2. **构建产物落盘位置一致**：所有构建产物都进入项目内的 `build/` 目录；按 build profile 分目录。
3. **`scoop run` 完整**：在 CONE 项目目录下运行时，若未构建则先构建，再执行已构建产物；支持 `--debug/--release`。
4. **为 cross compilation 预留结构**：即使当前还不支持 cross compile，也要预留在 `build/` 下隔离不同目标架构（如 `x86_64` / `arm64`）的目录结构。
5. **为 incremental build 预留路线**：先做到 “always rebuild but correct outputs”，再逐步引入增量（作为优化），并保持语义/诊断/可回归优先。

### 0.2 非目标（Non-goals）

- 本文不定义新的语言语义，也不引入新的编译器 intrinsic。
- 本文不要求立刻实现细粒度增量编译（依赖分析/模块图/缓存复用）；只定义演进路线。

## 1. CONE 项目目录结构

### 1.1 根目录结构（`scoop new` 生成）

建议 `scoop new <project-name>` 生成如下结构：

```text
<project-name>/
├── .gitignore
├── Cone.toml
├── README.md
├── src/
│   └── main.scoop
└── build/                # 构建产物目录（运行时生成；应被 git 忽略）
```

说明：

- `build/` 目录不必由 `scoop new` 预先创建；但 `.gitignore` 必须忽略它。
- `src/main.scoop` **必须包含 `println`**，确保新项目“开箱可运行且有可观察输出”。

### 1.2 `.gitignore` 约定（最小但足够）

项目根目录的 `.gitignore` 至少包含：

```gitignore
# Scoop/Cone build artifacts
/build/

# OS / editor noise
.DS_Store
*.swp
```

> 备注：如果未来引入更多本地缓存目录（例如 `.scoop-cache/`、`.cone/` 等），可以在本文基础上继续扩展，但不要默认把源代码/fixtures 目录忽略掉。

### 1.3 `src/main.scoop` 模板（最小可运行）

自动生成的 `src/main.scoop` 需要包含 `println`，推荐模板：

```scoop
package <project_name>

fun main() {
    println("Hello, Scoop!")
}
```

> 备注：`println` 依赖 sysroot/stdlib 的既有实现；这里的目标是让“新项目能在 run-pass 下直接回归”。

## 2. `build/` 目录布局（profile + 预留 cross compile）

### 2.1 现阶段（无 cross compile）

当前没有 cross compile 时，所有产物放在：

```text
build/
├── debug/
└── release/
```

其中 `<profile>` 为：

- `debug`：默认 profile（`scoop build` / `scoop run` 默认使用）
- `release`：`--release` 对应 profile

### 2.2 未来（cross compile 预留）

当未来支持 cross compile 时，约定把不同目标架构产物放在 `build/<target>/` 下：

```text
build/
├── debug/                 # 默认/host（兼容现有结构）
├── release/
├── x86_64/
│   ├── debug/
│   └── release/
└── arm64/
    ├── debug/
    └── release/
```

说明：

- `build/<profile>/` 始终表示“默认/host target”的产物（保持简单、兼容现有/早期习惯）。
- `build/<target>/<profile>/` 表示未来 cross compile 的产物隔离。
- `target` 的命名建议从 **架构名** 起步（`x86_64` / `arm64`），后续如需更精确隔离（OS/ABI/triple），可把 `target` 逐步升级为更完整的字符串（例如 `x86_64-unknown-linux-gnu`），但不影响 profile 层级。

### 2.3 profile 目录内部的建议布局（可选但推荐）

为了后续增量构建与诊断便利，建议 profile 目录内部结构明确：

```text
build/<profile>/
├── bin/                   # 最终可执行文件
├── obj/                   # C/C++/runtime 编译得到的 .o 等中间产物
├── tmp/                   # 临时文件（可随时清理）
└── build.json              # 构建元信息（fingerprint/输入/工具链版本等）
```

> 本节为“推荐”，v0 实现可以只先落地 `bin/`（或直接把可执行放在 `build/<profile>/`），但建议尽早固定结构，避免后续迁移成本。

## 3. 构建流程完整性（`scoop build` / `scoop run`）

### 3.1 `scoop build`：产物输出到正确的 profile 目录

行为要求：

- 在 CONE 项目目录下执行 `scoop build` 时：
  - 默认输出到 `build/debug/…`
  - `scoop build --release` 输出到 `build/release/…`
  - `scoop build --debug` 显式选择 debug（与默认一致，但便于脚本化）
- 所有构建产物应写入项目内 `build/`，而不是 `/tmp` 或 workspace 全局目录。

产物命名建议：

- 最终可执行文件路径：
  - `build/<profile>/bin/<project-name>`（Windows 平台可为 `<project-name>.exe`）

### 3.2 `scoop run`：在项目目录下自动构建并执行

行为要求：

- 在 CONE 项目目录（存在 `Cone.toml`）下执行 `scoop run`：
  1. 若目标 profile 的可执行文件不存在：先 `build`，再执行
  2. 若可执行文件存在：
     - v0：允许仍然“always rebuild”（见 §4），但至少要保证输出目录正确且行为一致
     - 未来（增量）：如果输入未变化，直接执行（跳过 build）
- profile 支持：
  - `scoop run --release` 运行 release 产物
  - `scoop run --debug` 运行 debug 产物

> 备注：`scoop run` 应尽量复用 `scoop build` 的参数/配置解析逻辑（profile、linker、c-sources 等），避免两套路径行为分叉。

## 4. Incremental builds（增量构建）路线

### 4.1 v0（立即可做）：always rebuild，但输出目录与行为稳定

这是“先把地基铺平”的阶段：

- 每次 `scoop build` 都重建，但**始终**把产物放到 `build/<profile>/…`。
- `scoop run` 至少具备“未构建则构建”的完整行为。

### 4.2 v1（粗粒度增量）：输入 fingerprint 未变则跳过 build

在 v0 稳定后，引入最小增量优化：

- 在 `build/<profile>/build.json` 写入 fingerprint：
  - `Cone.toml` 内容 hash（或 mtime + size）
  - `src/**/*.scoop` 文件列表 + 每个文件的 hash（或 mtime + size）
  - 关键 build flags（profile、linker/link-flags、c-sources/cxx-sources 等）
- 再次 `build/run` 时：
  - 若 fingerprint 相同且 `bin/<project-name>` 存在，则跳过 build（直接 run）
  - 否则执行完整构建并更新 fingerprint

该阶段的收益：显著提升“重复运行/调试”的速度，且实现成本可控。

### 4.3 v2（细粒度增量）：依赖图 + 受影响组件重建

更长远的目标（后置）：

- 解析/resolve/typecheck 的结果缓存（按文件或 module 粒度）
- 依赖图（imports、跨文件引用、C/C++ sources）驱动“只重建受影响部分”
- 与未来 cross compilation 一起扩展到 `build/<target>/<profile>/…`

> 风险提示：细粒度增量容易引入“不一致但不报错”的 silent corruption。必须用 fixtures + `--gc-stress` +（如有）`--gc-verify` 的组合做强回归。
