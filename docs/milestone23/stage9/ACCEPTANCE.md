# M23-9 实际验收

状态：已完成并验收（2026-10-02）。最终代码、测试与格式基线为 `f1f3830ba`；前置阶段为已验收的 [M23-8](../stage8/ACCEPTANCE.md)。完成门见 [设计](DESIGN.md)，功能提交见第 5 节。

## 1. 交付结果

新增 `compiler/linker` 的 `scoop-linker` 库和低层 `scoop-link` 进程入口，只凭 executable `.slib`、完整依赖 `.slib`、普通 runtime 对象索引和明确的 Darwin/AArch64 toolchain 产生原生可执行文件。`scoop` 的 `link_built_program` 使用构建阶段保留的不可变产物快照调用同一链接入口；独立进程和同次构建编排均已有真实运行用例。

- 独立 Link reader 直接消费原 foundation 的 typed identity/key、ABI、production/import、registration、Strong/ODR 和对象，不创建 HIR 模板世界、不执行 MIR 语义重放。Compile 与 Link 共用既有 Code、对象、引用和逐 member ODR 检查。
- `link-support/1` 仅补 runtime String 数据 alias，指向定义方实际 TD。内嵆 tuple、函数值和指针的存储从完整 exact key 与 target 取得，复用原布局检查；不要求这些结构化组成另有导出的 nominal layout。
- source extern 在原 provider 生成普通 Strong Scoop ABI 薄入口。参数和结果保持原声明，入口保留 Managed poll、native transition 与 caller-root publication；原 native callee 的 GC effect 仍保存在 SourceExtern 合同。generated-C 使用 `-fno-builtin`，保留声明所指的实际符号调用。
- 所有 SourceExtern 统一比较完整合同，从实际 runtime 对象及 SDK 系统 export/re-export 解析定义。未使用的声明同样参与冲突检查；普通库可以调用 core/runtime 未使用的系统 export。缺失额外物理 provider 有明确诊断。
- 正式 runtime builder 按明确 C/assembly 源集、头文件依赖、compiler 内容、target、flags、ABI 和规则构建普通对象。内容缓存覆盖新增 include 候选与实际 SDK/resource 头文件；损坏缓存按 miss 重建，显式损坏索引报错。
- 启动 C 对象只引用闭包中每个原 image 和唯一 root descriptor，调用已有 `scoop_rt_run_program`。实际 `ld`、SDK/libSystem、deployment、PIE、stackmap section 和 ad-hoc signing 参数均由明确 profile 提供；String alias 与原 TD 同址。
- 最终 Mach-O 检查 entry、load commands、exports/bindings、实际 Strong/ODR 地址、startup 引用、相关 relocation/fixup、stackmap/EH section 和必要签名结构。验证后在输出文件系统原子 rename；失败保持已有输出，并发链接只发布完整可执行文件。

普通调用方式：

```sh
scoop-link --root-slib app.slib \
  --dep-slib core.slib --dep-slib library.slib \
  --runtime-objects runtime/index.cbor \
  --target aarch64-apple-darwin --dump-plan -o program
```

`index.cbor` 由 `scoop::build_runtime` 返回的普通对象集合写入；它是本地缓存／进程交接索引。M23-11 再把这一组库入口包装进公开 umbrella `scoop build/run/link`。

## 2. 真实源码、产物与运行矩阵

[program-link 集成测试](../../../compiler/scoop/tests/program_link.rs) 使用配套 `scoopc` 生成真实 core/library/executable 产物，再删除本次编译使用的 Scoop 与 runtime 源目录。全新 `scoop-link` 进程使用受限 PATH 和不可用的 Scoop sysroot／LLVM locator，只从磁盘产物、对象索引与明确 toolchain 完成链接。适用用例分别执行普通模式与 `SCOOP_GC_STRESS_MOVE=1`。

| 验收项目 | 实际检查 | 测试入口 |
| --- | --- | --- |
| 基础链接、计划与 startup | core/library/executable、library 普通 main、empty/NoGC main；真实 startup C/对象引用与 canonical plan golden | [基础入口](../../../compiler/scoop/tests/program_link.rs)、[计划](../../../compiler/scoop/tests/program_link/plans.rs) |
| 图顺序和初始化 | 六 Cone 链/diamond、动态 ready-set Kahn 顺序；eager 提前 ensure、lazy 无副作用、failure/cycle/恢复、root/eager 未捕获异常 | [初始化](../../../compiler/scoop/tests/program_link/initialization.rs) |
| 共享 ODR、值与协程 | sibling delegate 的 storage/cell/failure 恰一次；独立 adapter/helper member 并集；ZST、大值、含引用 struct/enum/tuple、Option/Array、函数值、协程与 finally | [共享实例](../../../compiler/scoop/tests/program_link/siblings.rs)、[组合](../../../compiler/scoop/tests/program_link/combinations.rs) |
| 重建 core 与 String 身份 | 将真实 String 声明移入另一 package，通过公开 typealias 保留调用；alias target 必须不同于基线，并与普通同名 String 共存运行 | [重建 core](../../../compiler/scoop/tests/program_link/rebuilt_core.rs) |
| 普通 extern 与系统输入 | core/普通库/root 同一符号合同；未被 core/runtime 使用的 `abs` export；未使用声明的 ABI/effect 冲突、kind/library/保留名/缺符号诊断 | [native](../../../compiler/scoop/tests/program_link/native.rs) |
| 输入边界 | 缺失/stale/版本/重复/额外/executable dependency；optional/diagnostic 中的 Mach-O 不参与链接；Compile-only payload 不触发前端 decode；未知 required member、截断和 digest 错误 | [artifact 输入](../../../compiler/scoop/tests/program_link/inputs.rs) |
| 编排和发布 | 删除源码、构建缓存及原 artifact locator 后保留快照仍可链接；并发同输出、rename 失败与旧结果保留 | [编排](../../../compiler/scoop/tests/program_link/orchestration.rs)、[发布](../../../compiler/scoop/tests/program_link/publication.rs) |
| runtime 对象及缓存 | 源码/头文件/include 候选/flags 变化，SDK/resource header 内容变化；索引往返、移动/重排、重复对象、缺入口、错 digest 与损坏缓存重建 | [runtime builder](../../../compiler/scoop/src/runtime_build/tests.rs) |
| 最终文件损坏 | 基于真实 executable 的 11 类 entry/provider/CodeDirectory/alias/额外定义/stackmap/rebase/export/binding/ODR 地址损坏 | [最终 Mach-O](../../../compiler/linker/src/final_image/tests.rs) |
| stage 依赖边界 | `scoop` 不依赖 compiler 实现，driver 不依赖编排，artifact linker 不依赖前端／LLVM 实现 | [依赖边界](../../../compiler/scoop/tests/dependency_boundary.rs) |

Stage 7/8 的完整 Strong/ODR、gateway、真实线程握手、初始化并发、加载范围和移动 GC 回归继续保留。新增 program-link fixture 使用正式语言与 runtime 输出观察结果，未额外注入 fixture-native object。

## 3. 格式与兼容性

`org.scoop-lang.lir/link-support/1` 是必需的 Compile|Link section，固定单字段 map `{1=runtime_data_aliases}`，空集合编码为 `a10180`。记录保留 `RuntimeSymbolContractId` 与原 `StrongDefinitionOwnerV1`，不复制 ABI、布局或身份表。

`cross-cone-generic/2` ID 保持，required inventory 的 descriptor fingerprint 更新为 `803512d065288817215dd104c6c9c94400fecb92dea092efefc99b6657e3cc5e`。旧 required inventory 和旧 generated-C flags fingerprint 的产物／缓存需要重建，不借前端重放兼容。runtime metadata ABI 3、144-byte TD、现有 image/root/gateway 和 String 表示保持。

本阶段只交付 Darwin/AArch64、单主 Mach-O 与完整静态 Cone 图。一般用户 native library、archive/dynamic 输入由 [M23-10](../stage10/DESIGN.md) 接入同一解析器；后续设计不要求为尚无生产用途的 capability 增加 handler，unknown required 输入仍诊断。umbrella CLI、single-file 与历史 fixture 总迁移归 M23-11。final-link cache 仍为后续可选优化。

## 4. 最终验证与快照审阅

本机为 Apple Silicon／`aarch64-apple-darwin`，Rust **1.99.0**、LLVM **22.1.8**。每批实现先格式化和全 workspace/all-targets Clippy，再执行相关 fixture。最终命令使用以下构建配置：

```sh
export LLVM_SYS_221_PREFIX=/opt/homebrew/opt/llvm@22
export LLVM_CONFIG_PATH=/opt/homebrew/opt/llvm@22/bin/llvm-config
export CARGO_TARGET_DIR="$PWD/target"
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_PROFILE_TEST_OPT_LEVEL=1
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
export CARGO_PROFILE_DEV_DEBUG_ASSERTIONS=true CARGO_PROFILE_TEST_DEBUG_ASSERTIONS=true
export CARGO_PROFILE_DEV_OVERFLOW_CHECKS=true CARGO_PROFILE_TEST_OVERFLOW_CHECKS=true
export SCOOP_TEST_PAIRED_SCOOPC="$CARGO_TARGET_DIR/debug/scoopc"
export SCOOP_TEST_PAIRED_SCOOP_LINK="$CARGO_TARGET_DIR/debug/scoop-link"
cargo fmt --all -- --check
cargo clippy --workspace --all-targets
cargo build -p scoopc -p scoop-linker --bins
cargo test --workspace --no-fail-fast -- --test-threads=16
```

运行器在启动命令前删除所有 `SCOOP_UPDATE_*`、名称含 `SNAPSHOT` 的 `SCOOP_*`、`INSTA_UPDATE`、`INSTA_FORCE_PASS`、`INSTA_ACCEPT_UNSEEN`、`UPDATE_EXPECT` 和 `RUST_MIN_STACK`，再设置上述配置与实际配套工具路径。最终 fmt、Clippy、配套构建与完整 workspace 的退出码均为 **0**。

完整 workspace **41 个测试组、5390 passed、0 failed、0 ignored**，耗时 **669.84 秒**。新 program-link **21 项**和 stage 依赖边界 **3 项**均通过。主要既有回归如下：

| 测试组 | 通过项数 |
| --- | --- |
| scoop 编排与 runtime 构建 | 98 |
| codegen／runtime | 310 |
| HIR lowering | 1340 |
| MIR lowering | 114 |
| LIR 数据及验证 | 473 |
| slib 产物读取与验证 | 586 |
| driver 端到端 | 227 |

收尾同步了 142 份既有快照：49 份只改变摘要，9 份只改变局部函数编号，41 份 core LIR 摘要的 callable registration 与 object member 各增加 10；其余 43 份记录新增 extern 薄体、Managed/native transition 及相应对象、registration、safepoint 和符号计数。原类型布局、初始化单元和 ODR 结构保持；全部差异已审阅。旧 hello-world/print-overload 下标断言与 C bridge 配置影响的缓存固定向量同步，相关关闭更新的专项通过。受控生成运行不计验收，上述完整 workspace 已关闭全部更新开关。

实际配套工具 SHA-256：

| 工具 | 配套构建后 | 完整测试后 |
| --- | --- | --- |
| `scoopc` | `fc64ccf518b8e98c1c9871d6018a1b1fe0efbb3f10a40287cc933d49eb52a935` | `40b77ca17eb7ac06ad0da2a72315874930a93ed18befc250f43d835648f308ba` |
| `scoop-link` | `d6359ac22442250a1af3615d3bfec3650ffcd1a38321a1251bdaa3fee4e442f1` | `d6359ac22442250a1af3615d3bfec3650ffcd1a38321a1251bdaa3fee4e442f1` |

Cargo 在 workspace 测试准备阶段生成测试配置的配套二进制；前后摘要分别记录，最终计数来自同一次完整退出的回归。测试结束后工作区无任何未提交源码或快照变化。

本轮本地证据目录：

`/Users/chenxu/Documents/Codex/2026-10-02/m23-9`

- `final-fmt`、`final-clippy`、`final-build` 的 `.log/.json`：最终格式、lint、配套构建、命令与退出码。
- `final-workspace.log/.json`：完整输出、41 组逐项计数、耗时、环境处理及前后工具摘要。
- `snapshot-review.json`、`snapshot-structure.diff`：142 份收尾快照的分类及排除摘要/编号后的结构差异。
- `generate-driver-link-snapshots.log/.json`：最后两组较长产物组合的受控生成；`regression-cache-mir`、`regression-hir-defaults`、`relocated-core-test` 保留关闭更新的相关专项。
- `pre-regression-clean.log/.json`、`final-clean.log/.json`：回归前及最终 target 清理。

## 5. 功能提交与代码组织

功能按完成顺序提交，最终代码与测试基线为 `f1f3830ba`；其后的验收提交仅同步文档和阶段状态。

| 提交 | 完成内容 |
| --- | --- |
| `c7842c75c` | spec 先行：独立链接、runtime 对象、启动和验证职责 |
| `f7ecba985` | Code 与逐 member ODR 检查解除前端 section 持有要求 |
| `26702a3c6` | 独立 executable artifact 图与机器 Link reader |
| `6d513de98` | 必需 Link support section 与 typed String alias |
| `bf2c7d202` | 明确 Darwin linker、SDK 和系统 export/re-export 闭包 |
| `35a753336` | 普通 runtime 构建、对象索引和头文件内容缓存 |
| `fb616d75f` | source extern 的原 provider Scoop ABI 入口 |
| `da3e94777` | 正式启动对象、实际系统链接与 executable 发布 |
| `da1d291ca` | scoop 构建快照编排复用同一 program linker |
| `191122318` | canonical startup、共享 delegate 和失败缓存组合 |
| `15f5adc2f` | extern 入口 native transition 与真实 native 符号调用 |
| `dd51773c9` | 最终 exports、定义地址、fixup、Mach-O 格式与损坏用例 |
| `1fdbd60b1` | 结构化值存储的 artifact layout/ABI 读取 |
| `5052aa74d` | 并发发布与 rename 失败保留 |
| `6d1fbdd31` | startup/计划 golden 与有效输入 fingerprint 变化 |
| `75414c797` | artifact 输入边界、具体错误定位与目录代码拆分 |
| `6e0f509e9` | 重建 core 后真实 String TD owner 变化的运行验证 |
| `f1f3830ba` | 142 份 stage/产物快照及旧断言、缓存向量同步 |

相对本阶段实施前基线 `f65c346e5`，新增 78 个 `.rs/.c/.h` 文件，最长 233 行。最终文件的命令、export、签名、section、startup、fixup 与引用检查，以及 runtime 源输入、依赖、编译和测试，分别组织为职责明确的子模块。原 Code directory 逻辑从超长父模块拆出并复用。

实现过程中分批清理 target；本轮回归前删除 3096 个文件、4.6 GiB，最终全仓通过后执行 `cargo clean` 删除 3114 个文件、3.8 GiB，仓库 target 目录已不存在。此后仅修改验收文档与阶段状态，不重跑无变化的编译器测试。

后续阶段复用已完成的 reader、原对象/ODR 检查、统一 native 解析、runtime 对象、启动代码与最终检查，围绕实际 native 输入和公开 CLI 继续交付。不新增来源授权、防伪、通用资源预算或无实际调用的平行框架。
