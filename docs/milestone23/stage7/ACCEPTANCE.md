# M23-7 实际验收

状态：已完成并验收（2026-10-01）。对应 [设计第 12、14 节](DESIGN.md)；实现与阶段证据保留在 [进度记录](PROGRESS.md)。最终代码、测试与格式基线为 `6f4f660d5`，包含旧 Strong 专用发布入口的清理。

## 1. 交付结果

[M23-6a 的共同 HIR 前置条件](../stage6a/ACCEPTANCE.md)已经验收，本次完整回归继续覆盖。当前源码与产物使用同结构的声明／正文、共同查询与具体化；依赖记录用于实际存储和原 typed 身份定位。provider 发布模板产物后移走源码，下游可以用自身或其他依赖的类型实例化，并再次发布可消费 `.slib`。

- 泛型函数、名义类型、构造、成员、默认值、bound 和 hidden support 的实际机器定义贯通 MIR、LIR、ABI、layout／scan、TD／dispatch、对象及 reader。Strong 定义由原 Cone 提供，generic application 与结构 helper 具有各自完整 typed 身份和 ODR 归属。
- ODR 按共同 member 的完整 ABI、canonical LIR、规范化对象、关联数据与 registration 内容比较；独立 adapter／helper member 合法取并集。source 文件顺序、消费路径与物理分片变化由实际 fixture 验证，不按机器代码内容合并不同身份。
- lambda、函数引用、静态／动态函数适配、原生 NoGC 地址与协程完成实际产物消费、再次发布和运行。协程使用真实 core `Continuation<R>`／`SuspendTask<R>`／`SuspendRegistration<R>`、实际成员槽与完整隐藏 ABI；有限 source exact 的 start 由定义 Cone 发布，泛型／结构结果按实际需求生成 ODR helper。
- 泛型 delegated extension 的 ensure、storage、cell、failure root 和 initializer 使用完整 application 身份；两个 sibling 的实际读写、初始化恰一次与失败共享在移动 GC 下成立。
- core、普通 provider、consumer、reader、publisher 和构建缓存复用共有生产路径。旧 `SingleConeStrongArtifactInputV1`／`AssembledSingleConeStrongArtifactV1`、无构造点的 driver Strong-profile 错误及 HIR `GenericOdrRequired` 已删除；原格式 fixture 复用 canonical archive 构造器。

当前泛型产物 profile 为 `cross-cone-generic/1`，共有 HIR interface 为 `/43`、MIR type bridge 为 `/6`、LIR layout ABI 为 `/5`、cone-production 为 `/3`，runtime metadata ABI 为 **2**。storage bridge 与 coroutine start 分别使用 MIR lowering role 12／13；旧 `ContinuationShell` generated callable tag 10 已退役且未复用。相关固定向量、fingerprint、旧格式拒绝与重建规则同步，旧 `.slib` 和构建缓存需重建。

## 2. 真实源码与产物矩阵

下列测试入口对应仓库中的独立与组合 fixture；HIR／MIR／LIR golden 锁定实际阶段输出。provider／中间 consumer 发布后移走源码，再次编译只接收明确的产物依赖。运行使用正式 reader 返回的真实对象与登记，链接 C runtime 和系统 linker。

| 设计要求 | 已验证行为 | 主要测试入口 |
| --- | --- | --- |
| generic functions | 推导／显式／`_`、local／extension／final method、完整宿主与方法实参、class／interface bound、value／ref；消费方自有类型再次实例化 | [泛型产物](../../../compiler/driver/src/request/preflight/end_to_end_tests/generic_bodies/publication.rs)、[bound 与引用](../../../compiler/driver/src/request/preflight/end_to_end_tests/generic_bodies/bounds.rs) |
| generic nominals | class／struct／enum／interface、主次构造、嵌套 payload、父类型替换、default／abstract override、protected、property／setter | [名义类型](../../../compiler/driver/src/request/preflight/end_to_end_tests/generic_bodies/nominals.rs)、[构造](../../../compiler/driver/src/request/preflight/end_to_end_tests/generic_bodies/constructors.rs)、[成员](../../../compiler/driver/src/request/preflight/end_to_end_tests/generic_bodies/members.rs)、[抽象派发](../../../compiler/driver/src/request/preflight/end_to_end_tests/generic_bodies/abstracts.rs) |
| calls and defaults | named／default／vararg、receiver／RHS 求值顺序、提供方默认值中的泛型调用、继承默认值、第三个 Cone 再发布 | [共同默认值](../../../compiler/driver/src/request/preflight/end_to_end_tests/generic_bodies/shared_defaults.rs)、[整组推断](../../../compiler/driver/src/request/preflight/end_to_end_tests/generic_bodies/argument_inference.rs)、[数组与 vararg](../../../compiler/driver/src/request/preflight/end_to_end_tests/generic_bodies/arrays.rs) |
| hidden support | 私有普通／泛型 helper、局部递归／多层捕获、内部类型与属性、定义处重载；非法直接导入和 public default 引用保留前端诊断 | [模板支持](../../../compiler/driver/src/request/preflight/end_to_end_tests/generic_bodies/publication.rs)、[共同闭包](../../../compiler/driver/src/request/preflight/end_to_end_tests/generic_bodies/shared_closures.rs)、[原提供方状态](../../../compiler/driver/src/request/preflight/end_to_end_tests/generic_bodies/globals.rs) |
| exact values and ABI | consumer-local 类型、ZST、24-byte 含引用大值、struct／enum／tuple、Option／niche、array／vararg、cast／type test、boxing 与返回值再次传入 | [值布局](../../../compiler/driver/src/request/preflight/end_to_end_tests/generic_bodies/value_layouts.rs)、[Option](../../../compiler/driver/src/request/preflight/end_to_end_tests/generic_bodies/options.rs)、[挂起结果 ABI](../../../compiler/driver/src/request/preflight/end_to_end_tests/generic_bodies/coroutines.rs) |
| generated functions | lambda／capture、local recursion、callable reference、静态／动态函数型变、泛型协程、异常与 finally、强制移动 GC | [函数引用](../../../compiler/driver/src/request/preflight/end_to_end_tests/generic_bodies/references.rs)、[函数适配](../../../compiler/driver/src/request/preflight/end_to_end_tests/generic_bodies/adapters.rs)、[协程](../../../compiler/driver/src/request/preflight/end_to_end_tests/generic_bodies/coroutines.rs) |
| native function addresses | 外来参数自由 `@NoGC` 源函数首次取 C 地址、定义方 storage bridge、精确重载、默认值／模板引用、转导出、大值与指针写回、真实 C callback | [原生函数取址](../../../compiler/driver/src/request/preflight/end_to_end_tests/generic_bodies/native_addresses.rs) |
| delegate read/write | val／var、provide、多 receiver 共享、不同 application 隔离、再导出、setter／复合赋值顺序、ZST／大值／引用 delegate | [委托产物](../../../compiler/driver/src/request/preflight/end_to_end_tests/generic_bodies/delegates.rs)、[委托组合](../../../compiler/driver/src/request/preflight/end_to_end_tests/generic_bodies/delegates/combinations.rs) |
| delegate failures | 第一次失败、跨 Cone 再访问同一失败、初始化副作用恰一次、直接／间接 cycle；既有 runtime coordinator 的并发等待回归 | [sibling 状态共享](../../../compiler/driver/src/request/preflight/end_to_end_tests/generic_bodies/delegates/siblings.rs)、[角色与规则反例](../../../compiler/driver/src/request/preflight/end_to_end_tests/generic_bodies/delegates/negatives.rs) |
| core generic use | 修改并重建 core 后的 Option／array／协程协议与新增泛型声明；core 与普通 provider 使用共同生产和缓存路径 | [重建 core 数组与新增模板](../../../compiler/driver/src/request/preflight/end_to_end_tests/generic_bodies/rebuilt_core.rs)、[实际 Option owner](../../../compiler/driver/src/request/preflight/end_to_end_tests/generic_bodies/options.rs)、[实际协程成员槽](../../../compiler/driver/src/request/preflight/end_to_end_tests/generic_bodies/coroutines/rebuilt_core.rs) |

语言反例覆盖非 identity 递归替换、值布局／继承环、错误 bound、interface 自有 generic method、virtual generic method、非法可见性、从 result／RHS 推导 property binder、delegate suspend／default／vararg role，以及函数 effect／协程协议型变错误。实际诊断 fixture 核对 span 和信息，错误在前端结束。递归与方法声明规则可直接查阅 [泛型方法反例](../../../tests/fixtures/m14-generics/errors/generic-method.scoop)、[非 final 泛型方法反例](../../../tests/fixtures/m6-classes/errors/non-final-generic-method.scoop)及 [接口自有泛型方法反例](../../../tests/fixtures/m6-classes/errors/generic-interface-call.scoop)的配套诊断 golden。

原生函数取址新增 **14 份源码、23 份 golden**；函数适配新增 **16 份 Scoop 源码、34 份 golden**；协程新增 **15 份 Scoop 源码、28 份 golden**。协程的 8 组运行用例包含立即完成、真实挂起、失败恢复、同步／重复恢复、注册失败、值类型 task、函数值型变、闭包及待定异常跨 finally 再次挂起。24-byte 含引用值和泛型 enum payload 在挂起后经历分配，再验证恢复结果及 completion 恰一次。

重建 core 数组专项另有 **4 份源码、6 份 golden**：同时修改 `Array` 与 `MutableArray` 的源 `iterator` 正文，使测试 core 的迭代从索引 1 开始，并新增公开泛型 class／方法／函数与私有泛型 helper。core 源码移走后，独立与组合 consumer 必须观察到新的迭代结果；再次发布后以 consumer-local 类型、Int 和 Unit 运行，覆盖转换构造、Option、函数引用及移动 GC。

## 3. ODR、实际地址、格式与缓存

- [sibling 合并](../../../compiler/driver/src/request/preflight/end_to_end_tests/generic_bodies/siblings.rs)覆盖同一上游泛型函数／类型及字符串。新增 `m23-odr-member-unions` 的两组 fixture 让左右 sibling 从不同源函数签名适配到同一目标签名，各自贡献独立 member；共同 FunctionShape TD/member 保持一致，正反顺序合并通过。组合同时执行泛型异常／finally、捕获、分配和动态适配。
- C 探针从实际 MIR／LIR 表取得符号、interface TD 和槽位，读取两侧对象的实际 TD：相同 `Marker<Int>` 的 TD 与派发目标同址，`Marker<Long>` 的 TD 不同址。探针分配调用在已登记的入口栈帧中执行，跨下一次分配只保存静态 TD 指针。两组都通过普通与 `SCOOP_GC_STRESS_MOVE=1` 运行。
- [委托 sibling](../../../compiler/driver/src/request/preflight/end_to_end_tests/generic_bodies/delegates/siblings.rs)核对共同 cell／storage／failure-root 符号与真实 ODR 候选，并通过跨 Cone 读写、初始化次数和相同失败对象验证运行状态共用。不同 application 分离，已失败 initializer 不重新执行。
- [真实对象检查](../../../compiler/driver/src/request/preflight/end_to_end_tests/generic_bodies/machine/objects.rs)和完整 slib 回归覆盖多对象 plan、range／boundary、typed relocation、writer／patch 宽度、缺失 TD／dispatch／unit member、retired tag／profile 以及完整定义一致性。实际 LSDA、FDE／compact-unwind、stack-map 和 registration 字节变化纳入相应定义摘要；相同 member 的正文／字符串冲突即使 ABI 一致也被拒绝。重型损坏检查留在显式测试中。
- [真实泛型缓存](../../../compiler/scoop/src/snapshot/prepared/tests/process/generic_cache.rs)逐项修改正文、私有泛型 helper、默认值、`ref` 约束和类型实参。前四类只重编译 provider 与 consumer，最后一类只重编译 consumer；core 持续命中。6 轮重复输入的编译调用列表为空，所有节点为 `CacheHit` 且实际产物字节一致。旧 profile 拒绝与重建由固定向量及既有格式／缓存回归覆盖。

## 4. 最终验证

本机目标为 Apple Silicon／`aarch64-apple-darwin`，LLVM **22.1**。最终验证使用 `opt-level=1` 的 Rust 构建，保留 debug assertions 和整数溢出检查，关闭增量编译与调试符号。全部 `SCOOP_UPDATE_*`、自定义 snapshot 目录、`INSTA_UPDATE` 和 `RUST_MIN_STACK` 均清除，真实配套编译器明确启用。

```sh
export LLVM_SYS_221_PREFIX=/opt/homebrew/opt/llvm@22
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_PROFILE_TEST_OPT_LEVEL=1
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
export CARGO_PROFILE_DEV_DEBUG_ASSERTIONS=true CARGO_PROFILE_TEST_DEBUG_ASSERTIONS=true
export CARGO_PROFILE_DEV_OVERFLOW_CHECKS=true CARGO_PROFILE_TEST_OVERFLOW_CHECKS=true
export SCOOP_TEST_PAIRED_SCOOPC="$PWD/target/m23-6a/debug/scoopc"
cargo fmt --all
cargo clippy --workspace --all-targets --target-dir target/m23-6a
cargo build -p scoopc --target-dir target/m23-6a
cargo test -p scoop-slib --lib --target-dir target/m23-6a
cargo test --workspace --no-fail-fast --target-dir target/m23-6a
```

fmt、全仓 all-targets clippy 和配套构建全部通过，无警告。slib 专项 **584 passed、0 failed、0 ignored**；之后完整 workspace **37 个测试组、5333 passed、0 failed、0 ignored**，driver **218 项**全部通过，耗时 **346.62 秒**。完整运行包含编译器、真实子进程构建／缓存、core／provider／consumer、两个产物 view、对象及 C runtime／collector／GC／stack-map／EH 回归。此前未优化的协程全仓回归另有 5330 项全部通过。

测试结束后记录的实际配套编译器 SHA-256：`631e4849b1c3f69c7342eb357c68cfa8f8e679bf2f8a2c58aecc4b79e39256f2`。Cargo 在 workspace 测试准备阶段生成测试 profile 的 `scoopc`，随后才开始执行测试；该文件的摘要与前一步 `cargo build` 的摘要分别保留。

日志与汇总：

- `/tmp/scoop-m23-7-final-r3-fmt.log`、`/tmp/scoop-m23-7-final-r3-clippy.log`、`/tmp/scoop-m23-7-final-r3-build.log`；
- `/tmp/scoop-m23-7-final-r3-slib.log`、`/tmp/scoop-m23-7-final-r3-slib-results.json`；
- `/tmp/scoop-m23-7-final-r3-workspace.log`、`/tmp/scoop-m23-7-final-r3-workspace-results.json`；
- `/tmp/scoop-m23-7-final-r3-compiler.json` 与 `/tmp/scoop-m23-7-final-r3-compiler-post-tests.json`，分别保存构建阶段／测试阶段的编译器摘要、Cargo 准备记录和完整构建环境；
- `/tmp/scoop-m23-7-unions-strict-tests.log`：6 项 sibling／冲突／委托专项全部通过；
- `/tmp/scoop-m23-7-generic-cache-r1-tests.log`：真实泛型缓存五类变化及六轮命中全部通过。

## 5. 纪律与后续交接

实现和验收按功能提交。协程校验按 adapter／frame／控制流／签名职责拆分，大段 MIR golden 移出 Rust 源码；运行辅助代码拆为读取和执行模块。旧 1487 行 link reader 测试按归档、对象、生产记录和拒绝用例拆分，主模块约 260 行、其余模块均低于 400 行；原有 34 个函数／结构体除删除旧 writer 分支外保留正文。各批完成后清理本轮 target，具体记录见 [进度记录](PROGRESS.md)。

最终验证后执行 `cargo clean --target-dir target/m23-6a`，删除 **1679 个文件、2.0 GiB**；日志保留在 `/tmp/scoop-m23-7-final-clean.log`。

本阶段没有新增来源授权、防伪、通用预算、计费或仅服务假想使用者的框架。共有 reader 在相应边界验证类型、格式、引用、ABI 和 GC 契约，随后复用未变化的记录；链接合并检查重复定义及实际引用关系。

M23-8 接收现有六类 Strong／ODR registration 以完成生产多 image 登记和启动；M23-9 在共有 reader 与实际 ODR 合并结果上完成正式 artifact-only program-link。一般 native provider 和历史 CLI 总迁移分别按 M23-10／11 的范围推进，继续复用本阶段真实 fixture。
