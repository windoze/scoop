# TODO（Sysroot Reshape R2）

> 生成时间：2026-05-19
> 设计基线：[`SYSROOT_RESHAPE_R2.md`](./SYSROOT_RESHAPE_R2.md)
> 计划基线：[`PLAN.md`](./PLAN.md)
> 当前状态：`P4-T03` 已完成；下一任务为 `P5-T01`。
> 执行原则：严格按 P0 -> P10 顺序推进；同一阶段内可按任务依赖拆 PR，但每个任务完成后必须保持仓库无 failing fixture，并回写完成记录。

## 全局约束

- [`SYSROOT_RESHAPE_R2.md`](./SYSROOT_RESHAPE_R2.md) 是本轮唯一设计基线；[`PLAN.md`](./PLAN.md) 是本轮唯一执行计划基线。若实现中需要改变 cone kind、sysroot layout、native boundary、runtime core 边界或 object/top-level 初始化语义，必须先更新设计与计划。
- 本轮先退场现有 `.cone` archive active path，回到 source-only cone 模型。不保留当前 `.cone` 格式兼容性，不做 binary distribution 新设计。
- Cone kind 只包含 `bin`、`lib`、`syslib`。`syslib` 只允许位于 `sysroot/lib/<cone.fqn>/` 下。
- `SourceOrigin::Sysroot` 不再直接授予语义特权；`@Intrinsic`、`@file:AllowIntrinsic`、sealed marker 等特权必须来自所属 cone 的 trusted `syslib` 身份。
- `lib` / `syslib` 不要求 entry point；只有 consumer `bin` cone 要求 entry point。
- Auto dependency 和 prelude package 是两个独立列表。自动依赖只决定 cone 是否加载/编译/链接；prelude 只决定未显式 import 时的短名可见性。
- Cone-local native sources 必须参与完整编译和链接；不能继续只编译当前 executable cone 的 C/C++ 源。
- `runtime/c/` 中实际属于某个 sysroot cone 的 C 代码必须迁移到对应 cone 的 `native/` 目录，并通过该 cone 的 `native-build` metadata 编译链接。
- Runtime core 只能保留 GC、allocation、root/native transition、object/type descriptor、global root、GC thread lifecycle、fatal trap substrate 和稳定公开 runtime header 所需声明。
- Object 初始化使用 codegen LLVM atomics + compiler-generated TLS object-init frame stack；不依赖 `Mutex`、`CondVar`、`scoop.sync.Once` 或 `scoop_once_begin/end`。
- Top-level `val` / annotated top-level `var` 必须覆盖所有 linked source cones，并在进入用户 `main` body 前 eager 初始化；不得 lazy first-access。
- Top-level `var` 必须显式标注 `@Global` 或 `@ThreadLocal`。
- 仓库内不得留下 failing fixture。过时 archive fixture 要删除或改写为 source-only cone fixture；删除必须说明被测对象已消失或被哪些新 fixture 覆盖。
- 每个任务完成后必须回写：改动范围、核心决策、验证结果、与 `PLAN.md` / `SYSROOT_RESHAPE_R2.md` 的闭合项、如有 fixture 删除/改写则说明理由。

## 固定定位清单

### 设计与计划

- `SYSROOT_RESHAPE_R2.md`：R2 设计基线。
- `PLAN.md`：R2 阶段计划。
- `TODO.md`：本任务清单。
- `SCOOP_FULL_SPEC.md`：最终需要同步 cone/sysroot/native boundary 语义。

### Cone / manifest / package

- `crates/scoopc/src/cone/manifest.rs`：`Cone.toml` 解析，当前已有 `[native-build]`。
- `crates/scoopc/src/cone/package.rs`：source package 加载，当前要求 `src/main.scoop`。
- `crates/scoopc/src/cone/archive.rs` / `consume.rs` / `scoopir/*`：`.cone` archive 写入/读取/API 注入路径。
- `crates/scoop/src/commands/package.rs`：`scoop package` archive CLI。
- `crates/scoop/src/commands/build.rs`：normal build context；P1-T02 后不再加载 `.cone` dependency graph。

### Sysroot / frontend

- `crates/scoopc/src/sysroot/mod.rs`：当前递归扫描 sysroot `.scoop` 文件。
- `crates/scoopc/src/frontend.rs`：support sources、`ProjectInput` / input-only `ProjectContext`、entry selection。
- `crates/scoopc/src/resolve/imports.rs`：自动 prelude star imports。
- `crates/scoopc/src/source.rs`：`SourceOrigin::Sysroot` / `SourceFile::is_sysroot()`。

### Native boundary / ABI

- `crates/scoopc/src/typecheck/annotations.rs`：`@Extern`、`@CallingConvention`、native/scoop ABI gates。
- `crates/scoopc/src/hir/mod.rs`：`ExternAbi`、`CallableAbiIdentity`、`ExternFun.calling_convention`。
- `crates/scoopc/src/llvm/codegen/call/abi.rs`：native callable ABI classification 和 `enter_native/leave_native` 插入。
- `crates/scoopc/src/llvm/codegen/main/declare.rs`：top-level function declaration、callconv、exported/object symbols。

### Runtime / native build

- `crates/scoop/src/commands/build.rs`：当前只编译 current explicit cone native sources。
- `crates/scoop/src/toolchain.rs`：C/C++ 编译与 runtime C 编译/链接。
- `runtime/c/scoop_runtime_api.h`：runtime core export allowlist。
- `runtime/c/scoop_thread.c` / `scoop_sync.c` / `scoop_test.c` / `scoop_once.c`：迁移或退场重点。
- `crates/scoop_runtime/src/abi_exports_allowlist.rs`：runtime export allowlist 测试。

### Fixture 入口

- `tests/fixtures/run_pass_cone/c_sources_extern_call_basic/`：现有 current-cone C source FFI 编译/链接基线。
- `tests/fixtures/typecheck_cone_archive/**`：当前 archive API 注入 fixtures，需删除或改写。
- `tests/testdata/cone/selectors_basic/`：source package selector 测试数据。

## 执行顺序总览

```text
P0 baseline freeze
  -> P1 archive 退场
    -> P2 native boundary annotations
      -> P3 cone kind
        -> P4 sysroot/lib layout
          -> P5 source cone graph
            -> P6 auto dependency + prelude
              -> P7 native build graph
                -> P8 runtime FFI migration
                  -> P9 object once + eager top-level init
                    -> P10 final verification
```

## 任务索引

| ID | 状态 | 阶段 | 标题 |
| --- | --- | --- | --- |
| `P0-T01` | [DONE] | P0 | 冻结 R2 baseline 与迁移清单 |
| `P1-T01` | [DONE] | P1 | 禁用/删除 `scoop package` archive CLI |
| `P1-T02` | [DONE] | P1 | 移除 normal build 的 `.cone` dependency flow |
| `P1-T03` | [DONE] | P1 | 删除或改写 archive fixtures 与 archive-only tests |
| `P2-T01` | [DONE] | P2 | `@Extern` 支持 `callingConvention` property |
| `P2-T02` | [DONE] | P2 | 有 body 的 `@CallingConvention` 生成 object-level native callable symbol |
| `P2-T03` | [DONE] | P2 | 支持 `Any as?` closed Pure function runtime cast |
| `P3-T01` | [DONE] | P3 | `Cone.toml` 解析 `kind = bin/lib/syslib` |
| `P3-T02` | [DONE] | P3 | `lib/syslib` 无 entry point 加载规则 |
| `P3-T03` | [DONE] | P3 | `syslib` path trust gate 与 intrinsic privilege gate |
| `P4-T01` | [DONE] | P4 | 重排 sysroot 到 `sysroot/lib/<cone>/src` |
| `P4-T02` | [DONE] | P4 | sysroot loader 改为加载 `sysroot/lib/*/Cone.toml` |
| `P4-T03` | [DONE] | P4 | sysroot overlay 迁移到 `overlay/lib/<cone>/...` |
| `P5-T01` | [TODO] | P5 | 引入 source cone graph 数据结构 |
| `P5-T02` | [TODO] | P5 | 支持本地 source path dependency fixtures |
| `P5-T03` | [TODO] | P5 | 保留 cone identity/kind 到 resolver/typecheck/codegen |
| `P5-T04` | [TODO] | P5 | 生成 per-cone init routine 与 final system entry 调用骨架 |
| `P6-T01` | [TODO] | P6 | 实现 auto dependency cone 列表 |
| `P6-T02` | [TODO] | P6 | 实现 prelude package 列表并与 auto dependency 解耦 |
| `P7-T01` | [TODO] | P7 | 将 native-build 扩展到所有 loaded source cones |
| `P7-T02` | [TODO] | P7 | dependency cone C++/link-flags/linker driver 覆盖 |
| `P8-T01` | [TODO] | P8 | 建立公开 `scoop_runtime.h` runtime core header |
| `P8-T02` | [TODO] | P8 | 迁移 `scoop.runtime.test` native helpers |
| `P8-T03` | [TODO] | P8 | 迁移 `scoop.sync` native implementation |
| `P8-T04` | [TODO] | P8 | 迁移 `scoop.thread` native implementation 与 thread entry trampoline |
| `P8-T05` | [TODO] | P8 | 迁移 string/native helper 边界并收窄 runtime allowlist |
| `P9-T01` | [TODO] | P9 | Object once 改为 LLVM atomics + TLS init-frame stack |
| `P9-T02` | [TODO] | P9 | Top-level `val` / annotated `var` 改为 per-cone eager init |
| `P9-T03` | [TODO] | P9 | top-level `var` annotation gate 与 storage 语义 |
| `P10-T01` | [TODO] | P10 | 全仓旧模型残留审计与 spec 文档同步 |
| `P10-T02` | [TODO] | P10 | 全量验证与最终完成记录 |

---

## [DONE] P0-T01：冻结 R2 baseline 与迁移清单

- 参考：`PLAN.md` §3、`SYSROOT_RESHAPE_R2.md` §0-§2。
- 目标：在删除旧 archive 和重排 sysroot 前，固定现状与迁移范围。
- 当前实现入口：`crates/scoopc/src/cone/**`、`crates/scoopc/src/sysroot/mod.rs`、`crates/scoop/src/commands/build*.rs`、`runtime/c/**`。
- 必须实现：
  1. 列出现有 archive 入口：`scoop package`、`ConeArchiveApi`、`load_cone_archive_api`、`inject_cone_dependency_public_api`、`typecheck_cone_archive` fixtures、`api.scoopir` export/consume。
  2. 列出现有 sysroot 特权入口：`SourceOrigin::Sysroot`、`SourceFile::is_sysroot()`、`@Intrinsic` gate、sealed marker sysroot-only gate。
  3. 列出现有 runtime-core 外溢 symbols：`scoop_thread_*` user API、`scoop_sync_*`、`scoop_test_*`、`scoop_once_*`。
  4. 记录 current-cone native-build 能力与 fixture：`tests/fixtures/run_pass_cone/c_sources_extern_call_basic/`。
- 验证：`cargo build`、`cargo test --all --all-targets`、`cargo run -p scoop -- test`。
- 完成条件：本任务末尾有完成记录，后续 P1-P10 触碰文件簇清晰。

### 完成记录（2026-05-20）

- 改动范围：仅冻结和记录 R2 基线；未改编译器/runtime 行为，未删除或改写 fixture；`PLAN.md` 和 `SYSROOT_RESHAPE_R2.md` 的阶段级计划未变化。
- Archive active path 基线：`crates/scoop/src/cli.rs` 仍暴露 `scoop package`；`crates/scoop/src/commands/mod.rs` 仍 dispatch 到 `crates/scoop/src/commands/package.rs::run`；`crates/scoopc/src/cone/archive.rs` 写出 `Cone.toml`、`api.scoopir`、`SOURCES_SHA256` 等 archive entries；`crates/scoop/src/commands/build/deps.rs` 仍通过 `SCOOP_CONE_PATH`、`cone/`、`deps/` 和 consumer root 搜索 `.cone`；`crates/scoopc/src/frontend.rs::ProjectContext` / `run_frontend` 仍携带并注入 `Vec<ConeArchiveApi>`；`crates/scoopc/src/cone/consume.rs` 仍实现 `ConeArchiveApi`、`load_cone_archive_api`、`inject_cone_dependency_public_api` 与 `api.scoopir` consume；`crates/scoop/src/fixtures/mod.rs` 仍把 `tests/fixtures/typecheck_cone_archive/**` 作为真实 `.cone` 注入 suite 运行。
- Sysroot privilege 基线：`crates/scoopc/src/source.rs` 仍以 `SourceOrigin::Sysroot` / `SourceFile::is_sysroot()` 表示 sysroot 来源；`crates/scoopc/src/sysroot/mod.rs` 仍递归收集 sysroot/overlay 下 `.scoop` 并用 `SourceFile::load_sysroot` 加载；`crates/scoopc/src/frontend.rs::load_default_support_sources` 仍把 sysroot files 作为 support sources 加入输入；`crates/scoopc/src/typecheck/builtin_annotations.rs` 仍识别 `@file:AllowIntrinsic`；`crates/scoopc/src/typecheck/annotations.rs::check_intrinsic_builtin_annotation_gate` 仍允许 `file_allows_intrinsic || source_is_sysroot(source)`；`crates/scoopc/src/typecheck/type_env.rs::check_sealed_interface_decl_shape` 仍用 `source.is_sysroot()` 限制 sealed marker 定义。
- Runtime-core 外溢基线：`runtime/c/scoop_runtime_api.h` allowlist 仍登记 `scoop_once_begin/end/guard_canonicalize`、`scoop_sync_mutex_*`、`scoop_sync_condvar_*`、`scoop_sync_once_*`、大量 `scoop_test_*`、以及 user-level `scoop_thread_spawn/join/yield/sleep_millis/current_id` 等符号；实现集中在 `runtime/c/scoop_once.c`、`runtime/c/scoop_sync.c`、`runtime/c/scoop_test.c`、`runtime/c/scoop_thread.c`，并由 `crates/scoopc/src/llvm/codegen/runtime_symbols.rs` / `runtime_abi.rs` 等 active codegen 路径引用。
- Current-cone native-build 基线：`crates/scoopc/src/cone/manifest.rs` 已解析 `[native-build]` 的 `c-sources`、`c-flags`、`cxx-sources`、`cxx-flags`、`linker`、`link-flags`；`crates/scoop/src/commands/build.rs` 仅当 `front.input().is_explicit_cone()` 时编译当前 consumer cone 的 C/C++ sources 并透传 link flags；`tests/fixtures/run_pass_cone/c_sources_extern_call_basic/` 以 `native/add.c` + `@Extern(name = "cone_add_int")` 覆盖当前显式 cone 的 C source 编译/链接/run 能力。
- 后续触碰文件簇：P1 聚焦 `scoop` CLI/package/build deps、`scoopc::frontend`、`scoopc::cone::{archive,consume,scoopir}` 和 `typecheck_cone_archive` fixtures；P2 聚焦 `typecheck/annotations.rs`、`hir` ABI fields、LLVM call/declare ABI、parser/typecheck/build fixtures；P3 聚焦 `cone/manifest.rs`、`cone/package.rs`、frontend entry selection、syslib trust/intrinsic gates；P4 聚焦 `sysroot/` layout、`sysroot/mod.rs`、overlay fixtures；P5 聚焦 source cone graph、`ProjectInput/ProjectContext`、resolver/typecheck/codegen cone identity；P6 聚焦 auto dependency/prelude lists 和 `resolve/imports.rs`；P7 聚焦 `commands/build.rs`、`toolchain.rs` 和 all-loaded-cone native build fixtures；P8 聚焦 `runtime/c/` headers/sources、sysroot cone `native/` ownership 和 runtime allowlist tests；P9 聚焦 LLVM object/top-level init codegen、entry emission 和 once runtime 退场；P10 聚焦全仓旧模型 grep、`SCOOP_FULL_SPEC.md` 与最终验证。
- 核心决策：`P0-T01` 是基线冻结任务，不拆分，不引入 workaround，不修改阶段级计划；archive fixture 删除/改写留给 P1，sysroot/source cone/trust 语义迁移留给 P3-P6，native/runtime 迁移留给 P7-P9。
- 与计划/设计闭合：已覆盖 `PLAN.md` §3 和 `SYSROOT_RESHAPE_R2.md` §0-§2 所要求的旧 archive、sysroot privilege、runtime-core 外溢和 current-cone native-build 基线；未发现需要更新 `PLAN.md` 的阶段级依赖变化。
- 验证结果：`cargo build` 通过；`cargo test --all --all-targets` 通过；`cargo run -p scoop -- test` 通过（fixtures: ok，1558 checks）；额外运行 `cargo clippy --all-targets -- -D warnings` 通过。

## [DONE] P1-T01：禁用/删除 `scoop package` archive CLI

- 参考：`PLAN.md` §4。
- 目标：让当前 `.cone` archive 不再被呈现为可用发布能力。
- 当前实现入口：`crates/scoop/src/commands/package.rs`、CLI command dispatch、`crates/scoopc/src/cone/archive.rs`。
- 必须实现：
  1. `scoop package` 删除或改为稳定 diagnostic，说明 `.cone` archive 暂未支持、等待重设计。
  2. 如果保留 archive 写入代码作为 future isolated helper，不得由 normal CLI 暴露为可用功能。
  3. 更新相关 CLI tests / help 文案。
- 验证：`cargo test -p scoop package -- --nocapture` 或对应 CLI tests；`cargo build`。
- 完成条件：用户不能通过 normal CLI 生成看似可用的 `.cone` 依赖包。

### 完成记录（2026-05-20）

- 改动范围：`crates/scoop/src/commands/package.rs`、`crates/scoop/src/cli.rs`；未修改 archive writer/reader future helper 代码，未改变 normal build dependency flow，未更新 `PLAN.md`。
- 核心决策：保留 `scoop package` 解析入口以提供稳定 diagnostic，但命令实现不再 canonicalize 输入、不创建输出目录、不调用 `write_cone_archive_v0`，因此 normal CLI 不能写出 `.cone` archive。
- Help / tests：`package` help 文案改为说明 `.cone` archive packaging 在 source-only cone redesign 期间暂不支持；原写 archive 的 package 单元测试改为断言稳定 unsupported diagnostic 和不写出 `.cone` 文件。
- 与 `PLAN.md` / `SYSROOT_RESHAPE_R2.md` 闭合项：满足 P1 中“`scoop package` 要么删除子命令，要么改为稳定 diagnostic”的要求；P1 后续 `.cone` build consume flow 与 archive fixtures 退场仍由 `P1-T02` / `P1-T03` 处理。
- 验证结果：`cargo fmt` 通过；`cargo build` 通过；`cargo test -p scoop package -- --nocapture` 首次与并行 build 争用后超时，重跑通过（6 passed, 0 failed）；`cargo clippy --all-targets -- -D warnings` 通过。

## [DONE] P1-T02：移除 normal build 的 `.cone` dependency flow

- 参考：`PLAN.md` §4。
- 目标：normal `scoop build/run` 不再从 `.cone` archive 读取 dependency API。
- 当前实现入口：`crates/scoop/src/commands/build/deps.rs`、`crates/scoopc/src/frontend.rs::ProjectContext`、`crates/scoopc/src/cone/consume.rs`。
- 必须实现：
  1. 移除 `SCOOP_CONE_PATH`、`cone/`、`deps/` 下 `.cone` 搜索在 normal build path 的使用。
  2. `ProjectContext` active path 不再携带 `Vec<ConeArchiveApi>`。
  3. `run_frontend` 不再调用 `inject_cone_dependency_public_api` 作为 normal dependency 机制。
  4. 保留 future/debug 代码时必须隔离，不能参与 build/run。
- 验证：grep active build path 中 `ConeArchiveApi`、`load_cone_archive_api`、`SCOOP_CONE_PATH`；`cargo build`。
- 完成条件：normal build/frontend 不依赖 archive API 注入。

### 完成记录（2026-05-20）

- 改动范围：`crates/scoop/src/commands/build.rs`、删除 `crates/scoop/src/commands/build/deps.rs`、`crates/scoopc/src/frontend.rs`；未修改 `PLAN.md`，未删除 archive writer/reader future helper，未处理 archive fixture suite（留给 `P1-T03`）。
- 核心决策：normal `scoop build/run` 不再解析 manifest `[dependencies]` 为 `.cone` archive graph，不再搜索 `SCOOP_CONE_PATH` / `cone/` / `deps/`，`ProjectContext` 改为 input-only，`run_frontend` 不再接收 `Vec<ConeArchiveApi>` 或调用 `inject_cone_dependency_public_api`。
- 测试更新：删除原本证明 normal build 可消费 `.cone` 的 build 单元测试；新增 `build_cone_package_ignores_archive_dependencies_in_normal_path`，用 manifest dependency + 无效 `.cone` 文件证明 normal build 不读取 archive。
- 与 `PLAN.md` / `SYSROOT_RESHAPE_R2.md` 闭合项：满足 P1 中“移除 normal build `.cone` dependency flow”的要求；source-only dependency graph、source path dependency、archive fixture 删除/改写仍按后续 `P1-T03` / `P5-*` 任务推进。
- 验证结果：`cargo fmt` 通过；grep 确认 Rust 源码中无 `SCOOP_CONE_PATH` / `load_dependency_graph`，`crates/scoop/src/commands` 中无 `ConeArchiveApi` / `load_cone_archive_api` / `inject_cone_dependency_public_api`，`crates/scoopc/src/frontend.rs` 中无 archive API 符号；`cargo test -p scoop --bin scoop build_cone_package_ignores_archive_dependencies_in_normal_path -- --nocapture` 通过；`cargo build` 通过；`cargo clippy --all-targets -- -D warnings` 通过；`cargo test -p scoop --bin scoop` 通过（121 passed）。
- 备注：一次未限定 `--bin scoop` 的 filtered test 在目标单元测试已通过后继续启动无关 integration test binary 并超过 120s；随后用限定命令重跑目标测试通过。

## [DONE] P1-T03：删除或改写 archive fixtures 与 archive-only tests

- 参考：`PLAN.md` §4。
- 目标：移除 `.cone` archive fixture 对 active test suite 的影响。
- 当前实现入口：`tests/fixtures/typecheck_cone_archive/**`、`crates/scoop/src/fixtures/mod.rs` 中 archive fixture runner。
- 必须实现：
  1. 对每个 `typecheck_cone_archive` fixture 决定删除或改写为 source-only dependency fixture。
  2. 被测对象是 archive API 注入本身的 fixture 删除；被测对象是 cone boundary/type visibility 的 fixture 改写。
  3. 删除 archive fixture runner active path 或改为 future-disabled。
- 验证：`cargo run -p scoop -- test tests/fixtures/typecheck_cone_archive/` 不再作为 active archive suite；`cargo run -p scoop -- test`。
- 完成条件：仓库无 active fixture 依赖 `.cone` archive。

### 完成记录（2026-05-20）

- 改动范围：`crates/scoop/src/fixtures/mod.rs`、`crates/scoop/src/fixtures/expectations.rs`、`crates/scoopc/src/cone/consume.rs`、`tests/fixtures/typecheck_cone/**`、`tests/fixtures/run_pass_cone/**`、删除 `tests/fixtures/typecheck_cone_archive/**` 的旧 `.cone` fixture 文件；未修改 `PLAN.md`。
- 核心决策：`typecheck_cone_archive` 从 active fixture routing 中退场，目录只保留 `README.md` 作为 retired marker；`cargo run -p scoop -- test tests/fixtures/typecheck_cone_archive/` 现在返回 0 checks，不再打包或读取 `.cone`。
- Fixture 删除/改写：`deps_visibility_filter` 改写为 source-only `tests/fixtures/typecheck_cone/deps_visibility_filter/`，保留 public/internal/private 跨 cone 可见性覆盖；`typealias_export_generic` 改写为 source-only `tests/fixtures/typecheck_cone/typealias_export_generic/`，保留跨 cone typealias 与泛型实例化覆盖；`program_boundary_export_entry_points` 改写为 manifest-backed `tests/fixtures/run_pass_cone/export_entry_point*/`，保留多导出入口 pass、缺失 closed Pure row、open row、non-Pure row、未声明 Raise 的诊断覆盖。
- Fixture 删除理由：`deps_api_injection` 的被测对象是旧 `.cone` `api.scoopir` 依赖注入本身，已随 archive active path 退场删除；`annotation_retention_export` 覆盖 `.cone` 导出 retention 过滤，属于旧 archive API 表面，删除；`pre_specialize_id_int` 与 `pre_specialize_type_box_int` 覆盖 `.cone` archive pre-specialize metadata 命中统计，删除，同时移除仅供这些 archive fixtures 使用的 `EXPECT-MONOMORPH-*` / `EXPECT-TYPE-MONOMORPH-*` expectation 解析测试。
- Archive-only tests：`crates/scoopc/src/cone/consume.rs` 中保留的 `.cone` reader 单测已重命名并注释为 future isolated archive helper tests，不参与 active dependency/build/fixture flow。
- 与 `PLAN.md` / `SYSROOT_RESHAPE_R2.md` 闭合项：满足 P1 中“删除或重写 `tests/fixtures/typecheck_cone_archive/**`，archive fixture 不再作为 active suite 依赖 `.cone`”以及“archive-only tests 删除或明确标记为 future isolated”的要求；未改变阶段级计划或设计基线。
- 验证结果：`cargo fmt` 通过；`cargo run -p scoop -- test tests/fixtures/typecheck_cone/deps_visibility_filter` 通过（4 checks）；`cargo run -p scoop -- test tests/fixtures/typecheck_cone/typealias_export_generic` 通过（2 checks）；五个 `run_pass_cone/export_entry_point*` 定向 fixture 均通过；`cargo run -p scoop -- test tests/fixtures/typecheck_cone_archive/` 通过（0 checks）；`cargo test -p scoop --bin scoop` 通过（118 passed）；`cargo test -p scoopc cone:: -- --nocapture` 通过（19 passed）；`cargo build` 通过；`cargo clippy --all-targets -- -D warnings` 通过；`cargo run -p scoop -- test` 通过（fixtures: ok，1552 checks）；`cargo test --all --all-targets` 通过（Rust tests: ok，包含 `scoopc` 904 tests）。

## [DONE] P2-T01：`@Extern` 支持 `callingConvention` property

- 参考：`PLAN.md` §5、`SYSROOT_RESHAPE_R2.md` §6。
- 目标：外部函数导入的 machine calling convention 由 `@Extern(..., callingConvention = "...")` 表达。
- 当前实现入口：`crates/scoopc/src/typecheck/annotations.rs`、`hir::ExternFun.calling_convention`、parser annotation args。
- 必须实现：
  1. `@Extern` annotation parser 接受 `callingConvention` 字符串 property。
  2. `@Extern(abi = "c", callingConvention = "C"/"cdecl")` 保存到 `ExternFun.calling_convention`。
  3. `@Extern(abi = "scoop", callingConvention = ...)` 稳定拒绝。
  4. 外部函数上单独叠加 `@CallingConvention` 的旧用法改为拒绝或迁移到新语义，避免两处声明冲突。
- 验证：新增 parser/typecheck fixtures；`cargo test -p scoopc typecheck::annotations -- --nocapture`。
- 完成条件：外部 C symbol 的 symbol name、boundary ABI、machine calling convention 都由 `@Extern` 单点表达。

### 完成记录（2026-05-20）

- 改动范围：`crates/scoopc/src/typecheck/annotations.rs`、`crates/scoopc/src/hir/mod.rs`、`crates/scoopc/src/hir/lower/util/annotations.rs`、`crates/scoopc/src/hir/lower/main/tests.rs`、新增/更新 parse/typecheck/UMB fixtures；未修改 `PLAN.md`。
- 核心决策：`@Extern` 现在接受 `callingConvention = "..."` 字符串 property，并复用当前支持集 `"c"` / `"cdecl"`；HIR `ExternFun.calling_convention` 改由 `@Extern` property 保存，不再从叠加的 `@CallingConvention` 读取。
- 语义门禁：`@Extern(abi = "c", callingConvention = "C"/"cdecl")` 通过并保存 machine calling convention；省略 `abi` 时仍默认 C ABI；`@Extern(abi = "scoop", callingConvention = ...)` 稳定拒绝；旧 `@Extern` 函数单独叠加 `@CallingConvention` 稳定拒绝，避免 boundary ABI 与 machine calling convention 两处声明冲突。
- Fixture / tests：新增 `tests/fixtures/parse/extern_fun_calling_convention_property.scoop`、`tests/fixtures/typecheck/extern_fun_calling_convention_property_ok.scoop`、`extern_fun_calling_convention_invalid_is_error.scoop`、`extern_fun_calling_convention_annotation_is_error.scoop`；更新 `extern_fun_scoop_abi_calling_convention_is_error.scoop` 与对应 UMB fixture 为新 property 形态；新增 HIR 单测确认 side table 保存 `callingConvention`。
- 与 `PLAN.md` / `SYSROOT_RESHAPE_R2.md` 闭合项：满足 P2 中“`@Extern` 拥有 `name`、`abi` 和可选 `callingConvention`”以及“`@Extern` 和 `@CallingConvention` 互斥”的要求；有 body 的 `@CallingConvention` 语义仍留给 `P2-T02`。
- 验证结果：`cargo fmt` 通过；`cargo test -p scoopc hir_collects_extern_calling_convention_property -- --nocapture` 通过；`cargo test -p scoopc typecheck::annotations -- --nocapture` 通过（0 个匹配单测）；`cargo run -p scoop -- test tests/fixtures/parse/extern_fun_calling_convention_property.scoop` 通过；`cargo run -p scoop -- test tests/fixtures/typecheck/` 通过（496 checks）；`cargo build` 通过；`cargo clippy --all-targets -- -D warnings` 通过；`cargo test --all --all-targets` 通过（905 passed）；`cargo run -p scoop -- test` 通过（1556 checks）。

## [DONE] P2-T02：有 body 的 `@CallingConvention` 生成 object-level native callable symbol

- 参考：`PLAN.md` §5、`SYSROOT_RESHAPE_R2.md` §6。
- 目标：Scoop 函数可生成 object-level native callable symbol，供 cone-local C 调用，但不表示 dylib/package export。
- 当前实现入口：`typecheck/annotations.rs`、`llvm/codegen/main/declare.rs`、ABI mangler/stable symbol 逻辑。
- 必须实现：
  1. `@CallingConvention(name = "...", convention = "C")` 可用于有 body 的普通 Scoop 函数。
  2. 该函数签名必须满足 C ABI / GC-free surface。
  3. codegen 生成指定 object symbol 和 LLVM callconv。
  4. 函数体仍按 managed Scoop codegen；native caller 必须已 GC attach，不自动 enter/leave native。
  5. `@Extern` 与 `@CallingConvention` 同时出现稳定拒绝。
- 验证：新增 build/IR fixture 检查 symbol/callconv；新增 negative fixtures 覆盖互斥和 GC ref 参数拒绝。
- 完成条件：cone-local C 可以链接调用 compiler 生成的 C-callable Scoop body function。

### 完成记录（2026-05-20）

- 改动范围：`crates/scoopc/src/typecheck/annotations.rs`、HIR lowering side table、LLVM codegen native callable wrapper emission、`llvm/emit.rs` 调用入口、相关 HIR 单测与 build/typecheck/run-pass-cone fixtures；未修改 `PLAN.md`。
- 核心决策：`@CallingConvention(name = "...", convention = "C")` 用于有 body 的普通 Scoop 函数时，不改变该函数的 ordinary managed Scoop ABI；后端额外生成 object-level C ABI wrapper symbol，wrapper 直接调用 plain managed entry，不插入 `scoop_enter_native` / `scoop_leave_native`，native caller 需自行满足 GC attach 等运行时前提。
- 语义门禁：`@Extern` 与 `@CallingConvention` 同时出现继续稳定拒绝；`@CallingConvention` body 函数必须有 body、非泛型、无 effect row/effect row 参数，且 receiver/参数/返回值必须属于当前 native value surface（标量、`UIntPtr`、`Ptr<T>`、纯 `FunPtr<F>` token、tuple、`@CLayout` struct）。
- Fixture / tests：新增 HIR 单测 `hir_collects_native_callable_body_symbol`；新增 `tests/fixtures/build/calling_convention_body_symbol_emit_llvm.scoop` 检查 wrapper symbol、plain call 和无 enter/leave native；新增 `calling_convention_body_gc_ref_param_is_error.scoop`、`calling_convention_body_effect_row_is_error.scoop`；更新互斥 fixture 为 `name` + `convention` 形态；新增 `tests/fixtures/run_pass_cone/c_sources_calling_convention_body_link/`，通过 cone-local C object 对 generated symbol 的引用证明链接可解析。
- 与 `PLAN.md` / `SYSROOT_RESHAPE_R2.md` 闭合项：满足 P2 中“`@CallingConvention` 用于有 body 的 Scoop 函数生成 object-level native callable symbol”、“与 `@Extern` 互斥”、“C ABI / GC-free surface gate”和“不表示 package/dylib export”的要求；未发现需要改变阶段级计划或设计基线的 blocker。
- 验证结果：`cargo fmt` 通过；`cargo test -p scoopc hir_collects_native_callable_body_symbol -- --nocapture` 通过；`cargo run -p scoop -- test tests/fixtures/build/calling_convention_body_symbol_emit_llvm.scoop` 通过；`cargo run -p scoop -- test tests/fixtures/typecheck/` 通过（498 checks）；`cargo run -p scoop -- test tests/fixtures/run_pass_cone/c_sources_calling_convention_body_link` 通过；`cargo build` 通过；`cargo clippy --all-targets -- -D warnings` 通过；`cargo test --all --all-targets` 首次 120s 超时，600s 重跑通过；`cargo run -p scoop -- test` 通过（fixtures: ok，1560 checks）。

## [DONE] P2-T03：支持 `Any as?` closed Pure function runtime cast

- 参考：`PLAN.md` §5、`SYSROOT_RESHAPE_R2.md` §6。
- 目标：补齐 `Any -> () -> Unit / Pure!` 方向的 runtime cast，支撑 thread entry Scoop shim。
- 当前实现入口：`typecheck/expr/infer.rs::check_function_type_cast_boundary`、`llvm/codegen/main/expr_op.rs::codegen_ref_is_instance_of_nonnull`、closure type descriptor 生成。
- 必须实现：
  1. 允许 `Any as? closed Pure function type`，至少 `Any as? () -> Unit / Pure!`。
  2. 继续拒绝任何 non-Pure/effectful function target。
  3. closure/function value runtime descriptor 必须区分函数签名，不能只用统一 `ScoopClosure` descriptor。
  4. `codegen_ref_is_instance_of_nonnull` 支持纯函数目标的 descriptor 检查。
  5. 成功 cast 后按普通 function value 调用路径调用。
- 验证：新增 run-pass/typecheck fixture：Pure closure -> Any -> `as? () -> Unit / Pure!` -> call；新增 effectful negative fixture。
- 完成条件：thread entry 可在 Scoop 代码中从 `GcHandle` 恢复 closure 并安全调用。

### 完成记录（2026-05-20）

- 改动范围：`crates/scoopc/src/typecheck/expr/infer.rs`、`typecheck/expr/error.rs`、LLVM closure/type-test/MIR codegen 与 validation、failure-policy audit、相关 typecheck/run-pass fixtures；未修改 `PLAN.md`。
- 核心决策：`Any as? (...)->R / Pure!` 现在作为唯一支持的 function runtime cast 进入 typecheck；`Any as`、open Pure function target、effectful function target 继续稳定拒绝。闭合性是编译期门禁，不进入 runtime descriptor key；runtime signature descriptor 只区分 receiver/参数/返回形状。
- Runtime descriptor：closure allocation 不再只写统一 `ScoopClosure` descriptor，而是写 signature-specific closure descriptor，并以统一 `ScoopClosure` descriptor 作为 parent；`codegen_ref_is_instance_of_nonnull` 对 function target 使用同一 signature descriptor 做 type-desc chain 检查。
- MIR 支持：runtime type descriptor/codegen support 与 materialized validation 将 `RuntimeTypeDescriptorKind::Function` 视为支持的 runtime-ref target；MIR closure allocation 从 materialized closure callable 的 env 后参数与 return type 生成 runtime signature descriptor，避免 open/closed Pure 静态差异破坏 runtime cast。
- Fixture / tests：删除旧的 `fn_type_cast_closed_pure_asq_is_error.scoop`，新增 `fn_type_cast_closed_pure_asq_ok.scoop`；新增 `fn_type_cast_any_to_effectful_asq_is_error.scoop`；新增 `tests/fixtures/run_pass_cone/fn_any_asq_closed_pure_call/`，覆盖 Pure closure -> Any -> `as? () -> Unit / Pure!` -> call，并用 `(Int) -> Unit / Pure!` 目标验证不同函数签名不会误匹配。
- 与 `PLAN.md` / `SYSROOT_RESHAPE_R2.md` 闭合项：满足 P2 中“补齐 `Any as? closed Pure function type`”、“继续拒绝 effectful function target”和“function/closure runtime cast 增加 signature-specific runtime descriptor”的要求；未发现需要改变阶段级计划或设计基线的 blocker。
- 验证结果：`cargo fmt` 通过；新 run-pass/typecheck 定向 fixtures 通过；`cargo run -p scoop -- test tests/fixtures/typecheck/` 通过（499 checks）；`cargo test -p scoopc mir_value_primitives_reject_open_function_type_cast_before_mir -- --nocapture` 通过；`cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture` 通过；`cargo build` 通过；`cargo clippy --all-targets -- -D warnings` 通过；`cargo test --all --all-targets` 首次 600s 超时后以 1200s 重跑通过；`cargo run -p scoop -- test` 通过（fixtures: ok，1562 checks）。

## [DONE] P3-T01：`Cone.toml` 解析 `kind = bin/lib/syslib`

- 参考：`PLAN.md` §6。
- 目标：manifest 层正式表达 cone kind。
- 当前实现入口：`crates/scoopc/src/cone/manifest.rs`。
- 必须实现：
  1. 新增 `ConeKind` enum。
  2. 解析 `[cone].kind`，仅允许 `bin`、`lib`、`syslib`。
  3. 明确缺失 kind 策略。推荐主线要求显式 kind；如临时默认 `bin`，必须在完成记录中登记后续退场。
- 验证：manifest parser unit tests 覆盖三种 kind、非法 kind、缺失 kind。
- 完成条件：后续 loader/build 不再用目录形态猜测 cone kind。

### 完成记录（2026-05-20）

- 改动范围：`crates/scoopc/src/cone/manifest.rs`、`crates/scoopc/src/cone/mod.rs`、`crates/scoopc/src/frontend.rs`、`crates/scoopc/src/stable_id.rs`、`crates/scoopc/src/cone/scoopir/tests.rs`、`crates/scoop/src/commands/{build,new}.rs`、`crates/scoop/src/commands/build/incremental.rs`、`crates/scoop/src/fixtures/mod.rs`、`crates/scoop/tests/t1124_incremental_cone_run.rs`、仓库内现有 `Cone.toml` fixtures/testdata；未修改 `PLAN.md`。
- 核心决策：新增 `ConeKind::{Bin, Lib, Syslib}`，`[cone].kind` 只接受 `"bin"`、`"lib"`、`"syslib"`；缺失 `kind` 稳定报错，不引入临时 `bin` 默认，因此没有后续默认值退场项。
- Fixture / 模板更新：现有 source-cone fixtures/testdata 和 `scoop new` 模板均显式写入 `kind = "bin"`；临时测试中动态生成的 `Cone.toml` 同步补齐 `version` 与 `kind`，保证 active loader/build path 不再依赖目录形态或隐式默认判断 cone kind。
- 与 `PLAN.md` / `SYSROOT_RESHAPE_R2.md` 闭合项：满足 P3 中“`ConeManifest` 解析 `[cone].kind`，允许值仅为 `bin`、`lib`、`syslib`，并用 enum 表达 cone kind”的要求；`lib/syslib` 无 entry 和 `syslib` path trust gate 仍按 `P3-T02` / `P3-T03` 推进。
- 验证结果：`cargo fmt` 通过；`rg --files-without-match '^kind =' -g 'Cone.toml'` 无输出；`cargo test -p scoopc cone::manifest -- --nocapture` 通过（10 passed）；`cargo test -p scoopc cone::package -- --nocapture` 通过（2 passed）；`cargo test -p scoop --bin scoop` 通过（118 passed）；`cargo build` 通过；`cargo clippy --all-targets -- -D warnings` 通过；`cargo test --all --all-targets` 通过（Rust tests: ok，包含 `scoopc` 909 tests）；`cargo run -p scoop -- test` 通过（fixtures: ok，1562 checks）。

## [DONE] P3-T02：`lib/syslib` 无 entry point 加载规则

- 参考：`PLAN.md` §6。
- 目标：`lib` 和 `syslib` source package 不再要求 `src/main.scoop`。
- 当前实现入口：`crates/scoopc/src/cone/package.rs`、`frontend.rs` entry selection。
- 必须实现：
  1. `load_cone_source_package` 对 `lib/syslib` 只要求 `src/**/*.scoop` 非空。
  2. `bin` 仍要求 entry anchor，并最终由 entry package/main 选择入口。
  3. 依赖 `lib` 中存在 `main` 时不能被 consumer `bin` 误选。
- 验证：loader tests；source dependency negative fixture。
- 完成条件：source library cone 能作为 dependency 存在而不带 entry point。

### 完成记录（2026-05-20）

- 改动范围：`crates/scoopc/src/cone/package.rs`、`crates/scoopc/src/frontend.rs`；更新 `memory/claude_plan.md` 执行记录；未修改 `PLAN.md` 或 `SYSROOT_RESHAPE_R2.md`。
- 核心决策：`ConeSourcePackage.main` 改为 `Option<PathBuf>`，仅 `kind = "bin"` 要求并保存 `src/main.scoop` entry anchor；`lib` / `syslib` 只要求 `src/**/*.scoop` 非空，`src/main.scoop` 若存在也只是普通 source，platform selector 也只对 `bin` 的 main anchor 做不可移除校验。
- Frontend 入口规则：目录输入作为 executable consumer 时仍必须是 `bin` cone；`frontend` 对 `pkg.main` 做 `bin` 专属校验后再建立 `ProjectInput`，避免 `lib/syslib` 无 main 时进入 executable entry 选择路径。
- Source dependency entry 覆盖：新增 `frontend` 单元回归，使用 resolver 中不同 `ConeId` 的 synthetic dependency source 验证 dependency `main` 不会被当成 consumer entry；当 consumer 和 dependency 存在同 FQN `main` 时，entry selection 仍选择 consumer cone 的 overload。完整 source path dependency fixture 仍由已排期的 `P5-T01` / `P5-T02` source cone graph 任务承接。
- Loader tests：新增 `lib` 无 main 成功、`syslib` 无 main 成功、`bin` 无 main 稳定失败、`lib` 中 main 命名文件非 entry anchor、`bin` selector 不可移除 main anchor 覆盖。
- 与 `PLAN.md` / `SYSROOT_RESHAPE_R2.md` 闭合项：满足 P3 中“`load_cone_source_package` 允许 `lib/syslib` 无 `src/main.scoop`”和“`bin` 仍要求 entry point”的要求；`syslib` path trust gate 与 intrinsic privilege gate 仍按 `P3-T03` 推进；source graph / path dependency 的端到端 fixture 仍按 P5 任务推进，未改变阶段级计划。
- 验证结果：`cargo fmt` 通过；`cargo test -p scoopc cone::package -- --nocapture` 通过（7 passed）；`cargo test -p scoopc frontend::tests -- --nocapture` 通过（2 passed）；`cargo build` 通过；`cargo clippy --all-targets -- -D warnings` 通过；`cargo test --all --all-targets` 通过（Rust tests: ok，包含 `scoopc` 916 tests）；`cargo run -p scoop -- test` 通过（fixtures: ok，1562 checks）。

## [DONE] P3-T03：`syslib` path trust gate 与 intrinsic privilege gate

- 参考：`PLAN.md` §6、`SYSROOT_RESHAPE_R2.md` §8。
- 目标：把 sysroot-origin 特权迁移到 trusted `syslib` cone。
- 当前实现入口：`source.rs`、`typecheck/annotations.rs`、sealed marker typecheck gate、sysroot loader。
- 必须实现：
  1. `kind = "syslib"` 仅在 `sysroot/lib/<cone.fqn>/Cone.toml` 有效。
  2. 用户/外部 source dependency 声明 `syslib` 稳定拒绝。
  3. `@Intrinsic` / `@file:AllowIntrinsic` 权限改查所属 cone 是否 trusted syslib。
  4. sealed marker 等 sysroot-only gate 改为 syslib-only gate。
- 验证：用户 cone 自声明 syslib negative fixture；普通 lib 声明 intrinsic negative fixture；syslib 声明 intrinsic positive fixture。
- 完成条件：`SourceOrigin::Sysroot` 不再直接授予语言特权。

### 完成记录（2026-05-20）

- 改动范围：`SourceFile` 增加显式 `SourceTrust::TrustedSyslib`；sysroot/session/frontend support source loading 改为显式加载 trusted syslib source；`typecheck/annotations.rs` 与 sealed marker gate 改查 `is_trusted_syslib()`；`cone/package.rs` 增加 `kind = "syslib"` 的 `sysroot/lib/<cone.fqn>/` path gate；fixture runner 对 companion `.sysroot` overlay source 执行 typecheck，以便 trusted syslib overlay fixtures 覆盖 annotation/intrinsic 诊断；未修改 `PLAN.md` 或 `SYSROOT_RESHAPE_R2.md`。
- 核心决策：`SourceOrigin::Sysroot` 仅保留物理来源含义，不再直接授予 `@Intrinsic`、`@file:AllowIntrinsic` 或 sealed marker 定义权限；`@file:AllowIntrinsic` 本身也只能出现在 trusted `syslib` source 中，普通用户源码不能用文件级 gate 自行开权。
- Path trust gate：`load_cone_source_package` 对用户/外部路径下的 `kind = "syslib"` 给出稳定 diagnostic；新增 `run_pass_cone/user_syslib_kind_is_error` 覆盖用户 cone 自声明 `syslib` 被拒；单元测试覆盖 trusted sysroot lib path 下的 `syslib` package 仍可无 main 加载。
- Fixture 改写：原用户源码中直接声明 `@Intrinsic` / `@file:AllowIntrinsic` 的 typecheck、run-pass、UMB fixtures 改为由 companion `.sysroot` trusted syslib overlay 持有 intrinsic surface，用户 fixture 只消费这些 surface；用户未获 trusted syslib 身份时的 intrinsic declaration negative fixtures 更新为 `intrinsic_decl_requires_trusted_syslib`；`@file:AllowIntrinsic` malformed-args fixture 保留在 trusted syslib overlay 中覆盖参数诊断。
- HIR golden 更新：sysroot `AllowIntrinsic` 注释收口后，`scoop.core.Int.plus` / `Int.equals` 的 source spans 改变，已同步相关 HIR golden 的 `target_decl_span`。
- 与 `PLAN.md` / `SYSROOT_RESHAPE_R2.md` 闭合项：满足 P3 中“`syslib` path gate”、“普通用户 cone 无法 self-elevate 到 `syslib`”、“普通 source/lib 不能声明 intrinsic”、“trusted syslib 可声明 intrinsic”和“sealed marker 改为 syslib-only gate”的要求；P4 的物理 sysroot layout 迁移仍按后续任务推进。
- 验证结果：`cargo fmt` 通过；`cargo test -p scoopc cone::package -- --nocapture` 通过（8 passed）；`cargo test -p scoopc typecheck::type_env -- --nocapture` 通过（13 passed）；`cargo test -p scoopc named_intrinsic -- --nocapture` 通过（8 passed）；`cargo test -p scoop --bin scoop` 通过（118 passed）；`cargo build` 通过；`cargo clippy --all-targets -- -D warnings` 通过；`cargo test --all --all-targets` 通过（917 passed）；`cargo run -p scoop -- test tests/fixtures/typecheck/` 通过（499 checks）；`cargo run -p scoop -- test tests/fixtures/run-pass/` 通过（416 checks）；`cargo run -p scoop -- test tests/fixtures/umb_fix/` 通过（152 checks）；`cargo run -p scoop -- test` 通过（fixtures: ok，1563 checks）。

## [DONE] P4-T01：重排 sysroot 到 `sysroot/lib/<cone>/src`

- 参考：`PLAN.md` §7。
- 目标：把现有内置 sysroot 文件迁移到真实 cone source layout。
- 当前实现入口：`sysroot/scoop.*/*.scoop`。
- 必须实现：
  1. 创建 `sysroot/lib/`。
  2. 迁移现有 `sysroot/scoop.core/*.scoop` 等到 `sysroot/lib/scoop.core/src/*.scoop`。
  3. 为每个 sysroot cone 添加 `Cone.toml`。
  4. 创建保留目录 `sysroot/bin/`、`sysroot/docs/`，但不参与加载。
- 验证：文件布局检查；`cargo build`。
- 完成条件：所有内置标准库源码位于 `sysroot/lib/<cone>/src`。

### 完成记录（2026-05-20）

- 改动范围：迁移现有 10 个内置 sysroot `.scoop` 源文件到 `sysroot/lib/<cone>/src/`；为 8 个现有 sysroot cone 添加 `Cone.toml`；新增 `sysroot/bin/.gitkeep` 与 `sysroot/docs/.gitkeep` 保留目录；同步 active `.sysroot/lib/scoop.core/src/*.scoop` overlay fixture 路径、相关源码注释与 HIR golden；更新本任务记录和后续 P8 sysroot 路径引用；未修改 `PLAN.md` 或 `SYSROOT_RESHAPE_R2.md`。
- 核心决策：`Cone.toml` 先保持最小 `[cone]` metadata，不在 P4-T01 提前引入 manifest dependency edges；source cone graph、auto dependency 与 prelude dependency 语义仍由 P5/P6 任务落地。
- kind 分类：`scoop.core`、`scoop.unsafe`、`scoop.collections`、`scoop.delegates`、`scoop.thread`、`scoop.sync`、`scoop.runtime.test` 按保守策略设为 `syslib`；`scoop.lang.string` 当前只使用普通 Scoop 与 `@Extern`，设为 `lib`。
- Fixture 迁移说明：旧 base path 改为 `sysroot/lib/scoop.core/src/core.scoop` 后，原 `.sysroot/scoop.core/*.scoop` overlay 不再能按相对路径替换 base core 文件；本任务同步迁移这些 active overlay fixture 到 `.sysroot/lib/scoop.core/src/*.scoop`，避免生成重复 core surface。P4-T03 仍负责把 loader/overlay 规则整体改为 manifest/cone-relative 规则。
- 与 `PLAN.md` / `SYSROOT_RESHAPE_R2.md` 闭合项：满足 P4 中“内置 cones 位于 `sysroot/lib/`，每个 cone 拥有 `Cone.toml` 和 `src/` 目录”以及“创建 `sysroot/bin`、`sysroot/docs` 保留目录”的要求；P4-T02 的 loader manifest discovery 与 P4-T03 的 overlay merge 语义尚未改变。
- 验证结果：布局检查确认 `sysroot/scoop.*/*.scoop` 和 active `.sysroot/scoop.core/*.scoop` 均已清空，`sysroot/lib/*/src/*.scoop` 有 10 个源文件，`sysroot/lib/*/Cone.toml` 有 8 个 manifest；`cargo fmt` 通过；`cargo build` 通过；`cargo test -p scoopc sysroot -- --nocapture` 通过（20 passed）；`cargo run -p scoop -- test tests/fixtures/build/` 通过（47 checks）；`cargo run -p scoop -- test tests/fixtures/typecheck/` 通过（499 checks）；`cargo clippy --all-targets -- -D warnings` 通过；完整 `cargo run -p scoop -- test` 首次因 5 个 HIR golden span 漂移失败，更新 golden 后 `cargo run -p scoop -- test tests/fixtures/hir/` 通过（26 checks），重跑完整 fixture suite 通过（fixtures: ok，1563 checks）；`cargo test --all --all-targets` 通过（Rust tests: ok，包含 `scoopc` 917 tests）。

## [DONE] P4-T02：sysroot loader 改为加载 `sysroot/lib/*/Cone.toml`

- 参考：`PLAN.md` §7。
- 目标：sysroot loader 不再递归扫描整个 `sysroot/**/*.scoop`。
- 当前实现入口：`crates/scoopc/src/sysroot/mod.rs`。
- 必须实现：
  1. loader 只发现 `sysroot/lib/*/Cone.toml`。
  2. 每个 sysroot cone 用 source cone package rules 加载。
  3. `sysroot/docs/*.scoop` 不会进入 compilation unit。
  4. 更新 sysroot unit tests。
- 验证：新增 `sysroot/docs/foo.scoop` 不加载测试；`cargo test -p scoopc sysroot -- --nocapture`。
- 完成条件：sysroot loading 基于 cone manifest，不基于盲递归 `.scoop` 搜索。

### 完成记录（2026-05-20）

- 改动范围：`crates/scoopc/src/sysroot/mod.rs`、`crates/scoopc/src/cone/package.rs`、`crates/scoopc/src/frontend.rs`、`memory/claude_plan.md`；未修改 `PLAN.md` 或 `SYSROOT_RESHAPE_R2.md`。
- 核心决策：base sysroot loader 改为只枚举 `sysroot/lib/*/Cone.toml`，并对每个 manifest 所在 cone root 调用 source cone package loader，因此 base sysroot sources 来自 `src/**/*.scoop`、platform selector 与 `lib/syslib` 无 entry point 规则，而不是从 `sysroot/**/*.scoop` 盲递归收集。
- Trust 语义：sysroot source 的 `SourceOrigin::Sysroot` 仍表示物理来源；`trusted syslib` 权限现在由该 cone manifest 的 `kind = "syslib"` 决定，`kind = "lib"` 的 sysroot source 以 sysroot origin 加载但不授予 `@Intrinsic` / sealed marker 等 syslib 特权。
- Overlay 边界：为避免抢做 `P4-T03`，本任务只把 base sysroot discovery 切到 manifest-based source cone rules；新布局 overlay 路径位于已知 `lib/<cone>/...` 时按 owning cone manifest kind 继承 trust，旧式未知 overlay 兼容扫描和旧 overlay fixture 迁移仍由 `P4-T03` 处理。
- Tests：新增/更新 sysroot 单元测试，覆盖 `sysroot/docs/foo.scoop` 不进入 `Sysroot::files` 或 support sources、manifest kind 控制 source trust、新 `lib/<cone>/src` layout 下的 overlay replacement，以及 overlay 新增到已知 `lib` cone 的 source 不获得 trusted syslib 权限。
- 与 `PLAN.md` / `SYSROOT_RESHAPE_R2.md` 闭合项：满足 P4 中“`Sysroot::default_path()` 仍指向 sysroot umbrella，但 loader 只扫描 `sysroot/lib/*/Cone.toml`”、“`sysroot/bin` / `sysroot/docs` 不参与 source loading”和“不再递归扫描任意 base `sysroot/**/*.scoop`”的要求；overlay mirror / cone-relative replacement 仍按 `P4-T03` 推进，阶段级计划未变化。
- 验证结果：`cargo fmt` 通过；`cargo test -p scoopc sysroot -- --nocapture` 首次在 lib tests 全通过后因 120s timeout 停在后续 test binary，600s 重跑通过（22 passed）；`cargo build` 通过；`cargo clippy --all-targets -- -D warnings` 通过；`cargo test --all --all-targets` 通过（919 passed）；`cargo run -p scoop -- test` 通过（fixtures: ok，1563 checks）。

## [DONE] P4-T03：sysroot overlay 迁移到 `overlay/lib/<cone>/...`

- 参考：`PLAN.md` §7。
- 目标：overlay 规则镜像新 sysroot layout。
- 当前实现入口：`sysroot/mod.rs` overlay merge、overlay fixtures。
- 必须实现：
  1. `SCOOP_SYSROOT_OVERLAY` 使用 `lib/<cone>/...` 结构。
  2. overlay 替换按 cone/file-relative path，而非全目录盲扫。
  3. 迁移旧 `.sysroot/` overlay fixtures。
- 验证：overlay tests；相关 fixture suite。
- 完成条件：旧 overlay 结构不再是 active path。

### 完成记录（2026-05-20）

- 改动范围：`crates/scoopc/src/sysroot/mod.rs` overlay merge 逻辑、sysroot overlay 单元测试、旧 `.sysroot/fixtures/...` 与 `.sysroot/scoop.*` fixture 文件迁移到 `.sysroot/lib/scoop.core/src/...`、本轮执行记录 `memory/claude_plan.md`；未修改 `PLAN.md` 或 `SYSROOT_RESHAPE_R2.md`。
- 核心决策：base sysroot 仍只通过 `sysroot/lib/*/Cone.toml` 发现已知 source cones；overlay 不再递归扫描整个 overlay 根目录，而是只对这些已知 cones 的 `overlay/lib/<cone>/src/**/*.scoop` 做 cone/file-relative merge。
- Trust 语义：overlay replacement 与 overlay-added source 都继承 owning base cone 的 manifest kind/trust；删除旧逻辑中未知 overlay path 默认 `trusted_syslib = true` 的行为，避免旧目录或拼错路径自带 syslib 特权。
- Fixture 迁移说明：所有仍在旧 active overlay 子树下的 `*.sysroot/fixtures/**/*.scoop` 和 `*.sysroot/scoop.*/*.scoop` 已迁到 `*.sysroot/lib/scoop.core/src/...`，保留原 package 声明和测试意图；既有已经使用 `*.sysroot/lib/scoop.core/src/*.scoop` 的 overlay fixtures 保持不变。
- Tests：新增 `legacy_overlay_paths_outside_lib_cones_are_ignored`，覆盖旧 `fixtures/...` 和 overlay `docs/*.scoop` 不进入 sysroot/support sources；现有 overlay replacement/addition/trust tests 继续覆盖 `lib/<cone>/src` active path。
- 与 `PLAN.md` / `SYSROOT_RESHAPE_R2.md` 闭合项：满足 P4 中“`SCOOP_SYSROOT_OVERLAY` 镜像 `lib/<cone>/...` 结构”、“overlay replacement 按 cone/file-relative paths，而非盲递归 source collection”和“旧 overlay 结构不再是 active path”的要求；阶段级计划未变化。
- 验证结果：`cargo fmt` 通过；`cargo test -p scoopc sysroot -- --nocapture` 通过（23 passed，重跑后无 warning）；`cargo run -p scoop -- test tests/fixtures/build/` 通过（47 checks）；`cargo run -p scoop -- test tests/fixtures/typecheck/` 通过（499 checks）；`cargo run -p scoop -- test tests/fixtures/run-pass/` 通过（416 checks）；`cargo run -p scoop -- test tests/fixtures/umb_fix/B-30-named-unsafe-funptr/` 通过（4 checks）；`cargo run -p scoop -- test tests/fixtures/umb_fix/B-32-print-panic-sysroot/` 通过（3 checks）；`cargo build` 通过；`cargo clippy --all-targets -- -D warnings` 通过；`cargo test --all --all-targets` 通过（920 passed）；`cargo run -p scoop -- test` 通过（fixtures: ok，1563 checks）。

## P5-T01：引入 source cone graph 数据结构

- 参考：`PLAN.md` §8。
- 目标：用 source cone graph 替代 support sources + current project 的扁平输入。
- 当前实现入口：`frontend.rs::ProjectInput/ProjectContext`、`cone/package.rs`。
- 必须实现：
  1. 定义 graph node：root、manifest、kind、sources、native-build、trust status、dependency edges。
  2. graph 支持 sysroot auto cones、consumer cone、local source dependency cones。
  3. 保留 deterministic DAG order。
- 验证：unit tests 构造 graph 并检查 order/kind/source set。
- 完成条件：frontend/build 能接收 source cone graph 作为 authoritative input。

## P5-T02：支持本地 source path dependency fixtures

- 参考：`PLAN.md` §8。
- 目标：为 source-only dependency 提供最小可测试路径。
- 当前实现入口：manifest dependencies 解析、build context loader、fixture runner。
- 必须实现：
  1. 定义本轮最小 path dependency 语法或 fixture-only 约定。
  2. `bin` cone 可依赖本地 `lib` cone。
  3. dependency `lib` public API 可被 consumer 解析/typecheck/codegen。
  4. dependency internal/private 不可见。
- 验证：source dependency positive fixture、internal visibility negative fixture。
- 完成条件：不依赖 `.cone` 也能覆盖跨 cone source dependency。

## P5-T03：保留 cone identity/kind 到 resolver/typecheck/codegen

- 参考：`PLAN.md` §8。
- 目标：flatten 成 compilation unit 后仍不丢 cone 边界。
- 当前实现入口：`resolve::ConeId`、`IndexedFile`、`TypeEnv`、HIR lowering setup、LLVM codegen inputs。
- 必须实现：
  1. 每个 source/indexed file 带所属 cone id/kind。
  2. resolver/typecheck 可查询 symbol owner cone。
  3. codegen 可访问当前 callable/source 所属 cone，用于 stable identity、native ownership、init routine。
- 验证：unit tests 和 source dependency fixtures。
- 完成条件：visibility、syslib privilege、native ownership 不依赖文件路径猜测。

## P5-T04：生成 per-cone init routine 与 final system entry 调用骨架

- 参考：`PLAN.md` §8、§12。
- 目标：先建立按 cone DAG 调 init routine 的结构，为 P9 eager init 落地做准备。
- 当前实现入口：LLVM entry emission、top-level init metadata、object/top-level value codegen。
- 必须实现：
  1. 收集每个 linked cone 的 top-level initializer roots。
  2. 为每个 linked cone 生成独立 init routine stub。
  3. final system entry 按 source cone DAG 调用 init routines 后进入用户 `main`。
  4. 初期 routine 可为空或只承接现有逻辑，但结构必须稳定。
- 验证：LLVM IR fixture 检查 init routine 和 call order。
- 完成条件：P9 可在该骨架上替换 top-level lazy init。

## P6-T01：实现 auto dependency cone 列表

- 参考：`PLAN.md` §9。
- 目标：自动加载基础标准 cones，但不自动 import 它们的短名。
- 当前实现入口：source cone graph builder、sysroot loader。
- 必须实现：
  1. 初始 auto dependencies：`scoop.core`、`scoop.lang.string`、`scoop.collections`、`scoop.delegates`。
  2. `scoop.thread`、`scoop.sync`、`scoop.runtime.test` 不进入默认 auto dependency。
  3. 显式 dependency 与 auto dependency 去重。
  4. `scoop.unsafe` 优先作为 `scoop.core` manifest dependency；如临时 auto-load，记录退场。
- 验证：未显式依赖 thread/sync 时不可解析其短名且不链接其 native objects。
- 完成条件：auto dependency 只负责加载/编译/链接，不影响短名 import。

## P6-T02：实现 prelude package 列表并与 auto dependency 解耦

- 参考：`PLAN.md` §9。
- 目标：自动 `import scoop.core.*` 与 `import scoop.lang.string.*`，并证明非 prelude auto dependency 不自动短名可见。
- 当前实现入口：`resolve/imports.rs`。
- 必须实现：
  1. Prelude packages 初始为 `scoop.core`、`scoop.lang.string`。
  2. Prelude package 所属 cone 未加载时报 compiler configuration error。
  3. auto dependency 中非 prelude package 的短名不可见，显式 import 后可见。
- 验证：prelude positive fixture；non-prelude auto dependency short-name negative fixture。
- 完成条件：auto dependency 与 prelude package 语义完全分离。

## P7-T01：将 native-build 扩展到所有 loaded source cones

- 参考：`PLAN.md` §10。
- 目标：所有 loaded source cones 的 C/C++ sources 都会编译和链接。
- 当前实现入口：`commands/build.rs` native-build block、`toolchain.rs`。
- 必须实现：
  1. 遍历 source cone graph nodes，收集 native-build sources。
  2. C/C++ source path 按 owning cone root 解析。
  3. object 输出名带 cone identity，避免冲突。
  4. flags 只作用于 owning cone。
  5. 复用 current-cone native-build 能力，并推广到 all loaded cones。
- 验证：`bin` 依赖 `lib`，`lib/native/add.c` 被编译链接并运行。
- 完成条件：cone-local FFI 不再局限 consumer executable cone。

## P7-T02：dependency cone C++/link-flags/linker driver 覆盖

- 参考：`PLAN.md` §10。
- 目标：dependency cone 的 C++ 和 link flags 参与最终链接决策。
- 当前实现入口：`commands/build.rs` linker selection、`toolchain.rs`。
- 必须实现：
  1. 任一 loaded cone 有 `cxx-sources` 时默认 C++ linker driver 选择规则生效。
  2. 各 cone `link-flags` 按 dependency-topological order 稳定追加。
  3. duplicate symbol 等 linker 错误不被隐藏。
- 验证：dependency C++ fixture、dependency link-flags fixture。
- 完成条件：native build graph 从编译到最终链接完整覆盖 loaded cones。

## P8-T01：建立公开 `scoop_runtime.h` runtime core header

- 参考：`SYSROOT_RESHAPE_R2.md` §7、`PLAN.md` §11。
- 目标：给 cone-local C FFI 提供稳定 runtime core 入口，避免每个 FFI hack private runtime layout。
- 当前实现入口：`runtime/c/` headers、`toolchain.rs` include path、runtime allowlist。
- 必须实现：
  1. 新增公开 header 路径，例如 `runtime/c/include/scoop_runtime.h`。
  2. 暴露 GC thread attach/detach substrate。
  3. 暴露 GC handle/pin APIs、必要 alloc/type descriptor/string/array helper 的最小稳定 surface。
  4. 不暴露 Immix/heap/thread-list/platform/private root internals。
  5. Cone native build 自动加入公开 header include path。
- 验证：cone-local C fixture include `scoop_runtime.h` 编译通过。
- 完成条件：迁移后的 cone native C 不再 include runtime private headers，除明确 syslib/core substrate 例外。

## P8-T02：迁移 `scoop.runtime.test` native helpers

- 参考：`PLAN.md` §11。
- 目标：test-only helper 不再随普通 runtime core 链接。
- 当前实现入口：`runtime/c/scoop_test.c`、`sysroot/lib/scoop.runtime.test/src/runtime_test.scoop`、runtime allowlist、runtime_gc fixtures。
- 必须实现：
  1. 将 `scoop_test_*` native code 迁到 `sysroot/lib/scoop.runtime.test/native/`。
  2. `scoop.runtime.test` 不进入默认 auto dependency。
  3. 测试 fixture 显式依赖或由 test harness 注入该 cone。
  4. runtime allowlist 删除迁出 symbols。
- 验证：runtime/test helper fixtures 通过；普通 hello-world 链接不含 `scoop_test_*`。
- 完成条件：test-only ABI 不再污染 normal runtime core。

## P8-T03：迁移 `scoop.sync` native implementation

- 参考：`PLAN.md` §11。
- 目标：Mutex/CondVar/user-visible Once native implementation 归 `scoop.sync` cone。
- 当前实现入口：`runtime/c/scoop_sync.c`、`sysroot/lib/scoop.sync/src/sync.scoop`、`llvm/codegen/intrinsics/sync.rs`。
- 必须实现：
  1. 将 sync native C 迁到 `sysroot/lib/scoop.sync/native/`。
  2. 能用 C ABI + GC-free handle 表达的底层 primitive 优先转 C ABI。
  3. 必须 managed ABI 的 helper 说明原因，并使用 `scoop_runtime.h`。
  4. public Scoop API 尽量用 Scoop wrapper 表达，不把整个 API 都写成 FFI。
  5. runtime allowlist 删除 `scoop_sync_*` runtime-core exports。
- 验证：sync fixtures 通过；普通不依赖 sync 程序不链接 sync native objects。
- 完成条件：`scoop.sync` C 实现由 cone native-build 提供。

## P8-T04：迁移 `scoop.thread` native implementation 与 thread entry trampoline

- 参考：`PLAN.md` §11、`SYSROOT_RESHAPE_R2.md` §6-§7。
- 目标：user-level thread API 归 `scoop.thread` cone，runtime core 只保留 GC thread lifecycle substrate。
- 当前实现入口：`runtime/c/scoop_thread.c`、`sysroot/lib/scoop.thread/src/thread.scoop`、`llvm/codegen/intrinsics/thread.rs`。
- 必须实现：
  1. 将 thread native C 迁到 `sysroot/lib/scoop.thread/native/`。
  2. C entry trampoline：attach current OS thread -> call `@CallingConvention` Scoop thread entry object symbol -> normal detach。
  3. Fatal trap 直接 abort，不承诺 detach/recovery。
  4. Closure/userdata 通过 GC handle raw token 传递，不裸传 GC ref。
  5. runtime core 保留并公开 GC thread attach/detach。
  6. runtime allowlist 删除 user-level `scoop_thread_spawn/join/sleep/currentId/yield` core exports。
- 验证：thread fixtures 通过；普通不依赖 thread 程序不链接 thread native objects。
- 完成条件：runtime core 不再实现 user-level thread API。

## P8-T05：迁移 string/native helper 边界并收窄 runtime allowlist

- 参考：`PLAN.md` §11。
- 目标：按 ownership 迁移 string-from-array 等 helper，并完成 runtime core allowlist 收口。
- 当前实现入口：`runtime/c/scoop_runtime.c` string helpers、`sysroot/lib/scoop.lang.string/src/lang_string.scoop`、runtime allowlist。
- 必须实现：
  1. 对 string helpers 做 ownership 分类：core substrate vs `scoop.lang.string` helper。
  2. 迁移属于 `scoop.lang.string` 的 native code 到 `sysroot/lib/scoop.lang.string/native/`。
  3. 保留 runtime core 中确属 canonical String object/type descriptor substrate 的部分。
  4. 更新 `scoop_runtime_api.h` allowlist 和 tests。
- 验证：string/lang fixtures、runtime export allowlist test、普通 link smoke。
- 完成条件：runtime core 只保留必要 string substrate，不承载高层 string cone FFI。

## P9-T01：Object once 改为 LLVM atomics + TLS init-frame stack

- 参考：`PLAN.md` §12。
- 目标：object lazy initialization 不再调用 runtime once helper。
- 当前实现入口：`llvm/codegen/object_init.rs`、`runtime/c/scoop_once.c`。
- 必须实现：
  1. object guard global zero-init，`0/1/2` 状态。
  2. codegen atomic load/cmpxchg/release-store helper。
  3. compiler-generated TLS object-init frame stack。
  4. `initializing` 时扫描 TLS stack，命中 same guard fatal trap，否则 busy wait。
  5. 检测 `A -> B -> A` 同线程间接环。
  6. `object_init.rs` 不再声明/调用 `scoop_once_begin/end`。
- 验证：IR 不含 `@scoop_once_begin/end`；self recursion、A/B cycle、cross-thread wait fixtures。
- 完成条件：object initialization 仅依赖 codegen atomics/TLS stack，不依赖 runtime once/sync/thread。

## P9-T02：Top-level `val` / annotated `var` 改为 per-cone eager init

- 参考：`PLAN.md` §12。
- 目标：top-level values 不再 lazy first-access，而在 final system entry 中按 cone DAG eager 初始化。
- 当前实现入口：`llvm/codegen/main/immut_value.rs`、entry emission、P5-T04 init routine 骨架。
- 必须实现：
  1. top-level immutable/mutable init path 不再调用 once helper。
  2. 每个 linked source cone 的 init routine 初始化本 cone roots。
  3. final system entry 按 source cone DAG 调用 init routines。
  4. 覆盖 consumer bin、explicit deps、auto deps、transitive deps。
  5. GC-containing global storage 在 initializer 可能触发 GC 前注册 root。
  6. GC-free top-level storage 可使用 `.bss` / `.data` / constant fold 等实现细节，但语义仍是 main 前完成。
- 验证：consumer/dependency/auto dependency top-level init order fixtures。
- 完成条件：top-level value access 不再触发 lazy init。

## P9-T03：top-level `var` annotation gate 与 storage 语义

- 参考：`PLAN.md` §12。
- 目标：明确 top-level mutable storage 必须显式选择 `@Global` 或 `@ThreadLocal`。
- 当前实现入口：typecheck annotations/properties、top-level var codegen、TLS/global storage codegen。
- 必须实现：
  1. 未标注 `@Global` / `@ThreadLocal` 的 top-level `var` 稳定拒绝。
  2. `@Global var` 使用 global storage 并在 cone init routine 初始化。
  3. entry thread 的 `@ThreadLocal var` 在 cone init routine 初始化 TLS storage。
  4. 明确非 entry thread 的 TLS 初始化策略；若本轮只覆盖 entry thread，文档和 diagnostic 必须写清。
- 验证：`@Global var`、`@ThreadLocal var` run-pass；unannotated var negative fixture。
- 完成条件：top-level var 初始化语义与 storage policy 明确。

## P10-T01：全仓旧模型残留审计与 spec 文档同步

- 参考：`PLAN.md` §13。
- 目标：删除或明确隔离旧模型残留，并更新语言/spec 文档。
- 当前实现入口：全仓搜索、`SCOOP_FULL_SPEC.md`、`docs/spec/**`。
- 必须实现：
  1. 搜索确认 normal build/frontend/codegen 不再使用 archive dependency injection。
  2. 搜索确认 `SourceOrigin::Sysroot` 不再直接授予 intrinsic/syslib 特权。
  3. 搜索确认 object init 不再调用 `scoop_once_begin/end`。
  4. 搜索确认 top-level value init 走 per-cone init routine + final system entry DAG。
  5. 搜索确认 runtime core 不再导出已迁移 feature-specific symbols。
  6. 更新 `SCOOP_FULL_SPEC.md` cone/sysroot/native boundary 章节。
- 验证：对应 grep 清单、`cargo run -p scoop_tools -- spec-fixtures check`。
- 完成条件：文档与实现一致，旧模型无 active path。

## P10-T02：全量验证与最终完成记录

- 参考：`PLAN.md` §13-§14。
- 目标：完成 R2 收尾，确认所有 fixture 与测试通过。
- 必须实现：
  1. 运行全量验证命令。
  2. 补齐所有任务完成记录。
  3. 如新增/修改 spec fixture code block，运行 sync/check。
  4. 更新 `PLAN.md` / `TODO.md` 当前状态，必要时按惯例归档。
- 验证：
  - `cargo fmt`
  - `cargo build`
  - `cargo test --all --all-targets`
  - `cargo run -p scoop_tools -- spec-fixtures check`
  - `cargo run -p scoop -- test`
  - `cargo clippy --all-targets -- -D warnings`
- 完成条件：R2 总完成判据全部满足，仓库无 failing fixture。
