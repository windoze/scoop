# M23-8 实际验收

状态：已完成并验收（2026-10-02）。最终代码、测试与格式基线为 `8c9b12746`；前置阶段为已验收的 [M23-6a](../stage6a/ACCEPTANCE.md) 和 [M23-7](../stage7/ACCEPTANCE.md)。完成门见 [设计](DESIGN.md)，各功能提交与阶段验证见 [进度记录](PROGRESS.md)。

## 1. 交付结果

多个独立 Cone 的实际产物现在通过同一私有入口 [`scoop_rt_run_program(images, image_count, root_entry)`](../../../runtime/src/startup.h) 启动。完整登记和发布发生在首个 managed initializer 之前；随后按确定顺序调用 eager gateway，再调用 root gateway，最终沿现有 runtime 路径 shutdown。

- 已加载 Mach-O 范围按实际 VM 权限验证；含重定位的只读元数据与最终 stackmap 位于 `__DATA_CONST`。六类 registration 分别建立 typed ID 和地址索引，Strong 重复拒绝，ODR 同址合并并保留独立 helper member 的合法并集。
- 在完整 root 可达闭包上检查依赖、坐标和版本；每次从当前 ready set 选择最小坐标的 Kahn 顺序，image 内按持久 unit ID 调度 eager。提前 ensure、lazy 和失败状态继续使用同一初始化机制。
- 实际 TD、callable、safepoint、scan、immortal、static storage、failure slot 与初始化单元在完整集合上解析。GC 直接复用唯一静态根、精确 immortal 起点和 PC 索引；运行期只查询已登记地址，静态布局验证不在热点重复执行。
- LLVM v3 stackmap 解析全部连续 blob、root pairs 和 live-outs；与 typed site、owner、实际 PC 及完整规范化 payload 关联。C SHA-256 向量与现有 Rust normalizer 一致，合法 ODR 重复合并为唯一 PC 项。
- 主线程以 NativeSafe attach；每次无参数 gateway 独立进入 EntryPending，首个真实 poll 才发布活动栈帧。GC 等待活动线程的真实 safepoint，保留外层 managed 段和 callback 的现有握手。
- root catch 在 native catch 内物化异常并写入专用 failure root，之后才 EndCatch。逐次 C wrapper 验证 0/1 状态；报告时从稳定 slot 读取异常，移动 GC 后仍报告正确类型。重复启动和启动期间重入被明确拒绝。
- 唯一 352-byte `InitializationUnitV1` registration 同时承担 coordinator 参数；16-byte cell 保留。旧 88-byte coordinator、副本符号/摘要、runtime main、`scoop_main` 绑定、合成根表及单 image 循环已退役。

格式原子迁移为 runtime metadata ABI **3**、`cross-cone-generic/2`、`cone-production/4`、`layout-link-closure/4`、`link-identity-closure/8` 和 object verifier **4**。HIR interface **43**、MIR type bridge **6**、LIR layout ABI **5**、manifest **2** 保持。旧 profile、退休字段及旧 ABI 有明确拒绝用例，旧 `.slib` 与对应构建缓存需要重建。

## 2. 实际源码、产物与 runtime 矩阵

运行辅助使用普通 executable runner Cone，引用各产物原 image/root descriptor 并调用同一生产入口；消费和运行前移走 provider 源码，image 输入使用逆序枚举。新增 8 组实际启动测试均执行普通模式和 `SCOOP_GC_STRESS_MOVE=1` 模式。

| 验收项目 | 已验证行为 | 主要测试入口 |
| --- | --- | --- |
| 多 image 启动与顺序 | 3 个 image 的 empty/ordinary/NoGC main；6 个 image 的链、菱形依赖、动态 ready-set 顺序；跨 image heap/String 静态根 | [实际启动](../../../compiler/driver/src/request/preflight/end_to_end_tests/runtime_images.rs) |
| eager/lazy/失败与环 | 六层提前 ensure 恰一次；未访问 lazy 无副作用；失败不重试，内部引用经 GC 保留；间接环及 initializer 内捕获环后成功；失败阻止后续 eager/main | [初始化组合](../../../compiler/driver/src/request/preflight/end_to_end_tests/runtime_images/initialization.rs) |
| 启动生命周期 | 空/缺 core 输入在副作用前失败；重复启动及 initializer 内 C 重入被拒绝；root/eager 未捕获异常报告真实类型 | [生命周期](../../../compiler/driver/src/request/preflight/end_to_end_tests/runtime_images/lifecycle.rs)、[实际异常](../../../compiler/driver/src/request/preflight/end_to_end_tests/runtime_images.rs) |
| 加载范围与闭包 | guard page、只读权限、对齐和溢出；六种相同 ID bytes 的独立命名空间、Strong/ODR、member 并集、依赖/坐标/版本损坏及动态 Kahn 顺序 | [地址范围](../../../runtime/tests/image_ranges_test.c)、[完整登记](../../../runtime/tests/image_registry_test.c) |
| 类型与静态存储 | TD/dispatch/scan 关系；合法空 itable slots 与非法非空指针；EncodedStaticValue leaf、String、ZST、区间重叠、初始化角色与 eager ID 顺序 | [类型](../../../runtime/tests/image_type_test.c)、[存储](../../../runtime/tests/image_storage_test.c) |
| 完整 stackmap | 三个连续 blob、零 root、ConstantIndex 规范化、ODR；损坏的 site/owner/PC/location/live-out/fingerprint | [stackmap 登记](../../../runtime/tests/image_stackmap_test.c)、[跨语言固定向量](../../../runtime/tests/stackmap_fingerprint_test.c) |
| gateway 结构与 GC 握手 | 3 种合法 LIR gateway 和 23 种结构错误；入口前/Pending/首次 poll/活动段/返回/两次 gateway 之间的真实 GC，嵌套 callback | [LIR 验证](../../../compiler/lir/src/gateway/tests.rs)、[线程握手](../../../runtime/tests/gateway_entry_test.c) |
| 失败报告与并发初始化 | 生产 wrapper 的错误状态/空 failure/错误 cell；另一线程移动异常后正确报告；一名初始化者与三名等待者共享成功或失败，失败根再次移动后不重试 | [gateway 报告](../../../runtime/tests/startup_gateway_test.c)、[并发与移动 GC](../../../runtime/tests/initialization_gc_test.c)、[初始化环](../../../runtime/tests/initialization_coordinator_test.c) |

既有完整回归继续在新 startup 上运行 Stage 7 的泛型及 sibling ODR、独立 adapter/member 并集、委托 storage/cell/failure 共享、函数值、协程挂起/恢复/异常/finally、ZST/大值/Option，以及重建 core 后的协议和产物双视图组合。实际 provider 源码移走后的消费与再次发布要求保持。

并发用例用 world mutex/condition variable 等待实际 wait edge 和状态发布，不用 sleep 猜测时序；等待中及成功/失败发布后都触发真实移动 GC。失败发布后清空临时 storage，只保留 failure root，再次收集后重读同一失败。focused C fixture 直接提供所测的已解析 V1 record，加载边界由独立测试覆盖。

## 3. 最终验证与格式向量

本机为 Apple Silicon／`aarch64-apple-darwin`，LLVM **22.1**。使用 `opt-level=1`、保留 debug assertions 和整数溢出检查，关闭增量编译与调试符号。完整回归明确删除子进程环境中所有 `SCOOP_UPDATE_*`、名称含 `SNAPSHOT` 的 `SCOOP_*`、`INSTA_UPDATE` 和 `RUST_MIN_STACK`，并设置实际配套编译器。

```sh
export LLVM_SYS_221_PREFIX=/opt/homebrew/opt/llvm@22
export LLVM_CONFIG_PATH=/opt/homebrew/opt/llvm@22/bin/llvm-config
export CARGO_TARGET_DIR="$PWD/target/m23-8"
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_PROFILE_TEST_OPT_LEVEL=1
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
export CARGO_PROFILE_DEV_DEBUG_ASSERTIONS=true CARGO_PROFILE_TEST_DEBUG_ASSERTIONS=true
export CARGO_PROFILE_DEV_OVERFLOW_CHECKS=true CARGO_PROFILE_TEST_OVERFLOW_CHECKS=true
export SCOOP_TEST_PAIRED_SCOOPC="$CARGO_TARGET_DIR/debug/scoopc"
cargo fmt --all -- --check
cargo clippy --workspace --all-targets
cargo build -p scoopc --bin scoopc
cargo test --workspace --no-fail-fast -- --test-threads=16
```

fmt、全 workspace/all-targets Clippy 和配套构建全部通过。最终完整 workspace **37 个测试组、5356 passed、0 failed、0 ignored**，进程退出码 **0**，耗时 **590.16 秒**；driver **226 项**全部通过，耗时 **546.40 秒**；codegen/runtime **310 项**、LIR **473 项**、slib **584 项**全部通过。

本次受控生成共更新 56 份快照，其中 45 份是 artifact fingerprint 快照。除摘要外的变化逐项核对为旧 coordinator 的摘要节点和符号/引用数量减少，以及发布 profile `/1` → `/2`；HIR/MIR 和其他语言结构保持。2 个缓存向量和 5 个 ODR 摘要字段同步，原 body/site 身份不变。生成运行不计最终验收；上述完整 workspace 已关闭所有更新开关，包含全部 25 项 core/layout/Link 组合及 8 项新启动组合。

实际配套编译器 SHA-256：

- `cargo build` 后：`f949b0f363e768c795baae0d62dbf92958b6796b878365cad3e47b87e3953fc8`。
- workspace 测试后：`f6f92d7778ef50ed5123c81a8ad8f3efb45b33ebc8ef54acd3a4ab8cf989e6a1`。

Cargo 在 workspace 测试准备阶段生成测试 profile 的配套二进制；前后摘要分别记录，最终计数只来自这次完整退出的回归。

本轮本地证据目录：

`/Users/chenxu/Documents/Codex/2026-10-01/m23-7-target-agents-md/work`

- `final-clippy.log`、`final-build.log`：最终 lint 与配套构建。
- `final-workspace.log`、`final-workspace-results.json`：完整输出、退出码、37 组逐项计数、耗时及前后编译器摘要。
- `snapshot-cache-vectors.log`、`snapshot-odr-vectors.log`：关闭更新的缓存/实际 ODR 专项；`empty-itable-bounds.log`：4 项合法 bound 组合回归。
- `init-gc-codegen.log`：并发初始化与移动 GC 纳入后的 310 项 codegen/runtime 回归。
- `final-clean.log`：最终专用 target 清理结果。

## 4. 代码组织与后续交接

功能按完成顺序独立提交，spec 先于实现调整，之后先格式化与 lint，再验证。stackmap 解析、image 登记、startup、gateway 验证和测试辅助按职责拆分；相对 M23-7 新增 65 个 `.rs/.c/.h` 文件，新增生产文件最长 **183 行**，新增测试文件最长 **321 行**。

各批提交后的 target 清理保留在进度记录。完整测试结束后再次执行 `cargo clean --target-dir target/m23-8`，删除 **1806 个文件、2.1 GiB**，最终只提交验收文档与状态同步。

本阶段检查集中于实际类型、格式、引用、地址、ABI 和 GC 契约，后续运行复用已经登记的静态结果。未引入来源授权、防伪、通用资源预算或没有实际调用的框架。

M23-9 可直接使用本阶段的生产 startup 入口和共有 reader 的完整 Link 数据，继续完成正式启动对象、runtime-build 和 artifact-only program-link。现有真实多 Cone fixture 可作为接入后的运行回归。
