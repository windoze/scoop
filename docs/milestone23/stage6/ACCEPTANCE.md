# M23-6 实际产物验收

状态：已完成（2026-09-27）。最终实现提交为 `d9bdc4abb`；功能、清理与验收变更均按职责提交。

本记录对应 [M23-6 设计](DESIGN.md) 和 [M23 过度设计清理](CORE-AUTHORITY-CLEANUP.md)。验收范围是参数自由类型的完整产物发布、跨 Cone 消费、单 image 链接运行，以及清理设计中的七组删除项。后续阶段的 generic/ODR、multi-image startup 和 artifact-only program-link 不在此范围内。

## 1. 真实生产路径

正式构建使用 `scoop` 调度和配套 `scoopc build` 子进程。core、普通依赖及当前 Cone 都通过相同的源码快照、依赖发现、缓存和 `.slib` 产出路径。完整 `CrossConeLayoutStrong` 产物携带 HIR 声明与类型关系、MIR 类型与 callable、LIR layout/ABI/dispatch、实际对象及初始化登记。

下游编译通过 `read_cross_cone_layout_artifact_closure` 读取实际归档。Compile 数据读入后直接用于 Link 的对象、符号和 relocation 检查；同一编译结果的发布使用已完成的归档和普通摘要，不重新读取当前产物及全部依赖执行两轮语义检查。

`imported_classes` 等生产测试从源码编译 core、provider、consumer 和 downstream。provider 源码目录随后移走，下游只能消费实际 `.slib`。运行检查提取同一 reader 返回的最终对象、TD、String 和静态根登记，链接真实 runtime，在普通模式和 `SCOOP_GC_STRESS_MOVE=1` 下执行。测试中的单 image 入口负责本阶段运行验收，不构成后续 program-link 功能。

## 2. 功能矩阵

| 能力 | 实际验收入口和内容 |
| --- | --- |
| core 修改、扩展、重建与缓存 | `compiler/scoop/src/snapshot/prepared/tests/process/core_cache.rs`、`core_root.rs`、`core_locator.rs`；真实子进程构建，源码变更失效、再次构建命中缓存、普通目录作为 core 根、显式依赖优先，以及切换为预构建 core 后的下游消费 |
| 完整类型与 canonical ABI | `imported_structs`、`imported_enums`、`imported_constructors`、`layout_exports`；完整字段与 variant、构造与默认值、值返回和再次传入、CLayout、ZST、direct/indirect/sret、typed layout 和实际对象 |
| 成员、属性与 companion | `imported_members`、`imported_classes`；普通方法、getter/setter、复合赋值、自增、限定名与转导出、默认参数、companion 与静态嵌套声明、再次发布后的消费 |
| interface 与 dispatch | `imported_classes/conformance.rs`；class/struct/enum conformance、接口继承与菱形选择、具体及抽象 override、真实 abstract target、默认方法、限定 `super<I>`、继承默认参数、值装箱和大值 ABI |
| class 继承与 protected | `imported_classes/inheritance.rs`；依赖基类构造、虚调用、`super`、属性及受保护嵌套 class/struct/enum/interface/object、有效 owner 访问域、接收者限制和签名泄露错误 |
| 继承的布局与初始化组合 | `inheritance-abi-*`；基类及派生类字段、ZST 字段/参数/结果、24 字节值的构造与虚调用、含 managed 引用的聚合参数和结果、次构造器顺序、初始化中的 inherited backing field、单例继承和第四 Cone 再次派生 |
| String、异常和初始化 | `imported_classes` 中的 `primitive-*`、`cast-*`、`arithmetic-*`、`initialization-*`；修改 core 后的 intrinsic 默认成员、异常默认构造器、初始化循环服务、object 唯一实例、失败缓存、静态根及移动 GC |
| ZST 与 runtime | `heap_zst`、codegen 的 boxing/array/address 测试和 `runtime_collector_tests`；逻辑值保留与 payload 消除、静态 token、数组长度/索引、扫描 DAG 与非法环、装箱根在握手前登记、引用移动后使用当前对象 |
| 错误与完整性 | 各组 negative fixture 断言实际 span 和诊断；共有 reader 的格式、版本、typed 引用、owner、ABI、依赖环和对象损坏测试；各阶段 golden 保留实际完整 HIR/MIR/LIR |

上述 `imported_*`、`heap_zst` 和 `layout_exports` 入口位于 [`compiler/driver/src/request/preflight/end_to_end_tests`](../../../compiler/driver/src/request/preflight/end_to_end_tests)。对应源码和三阶段快照位于 [`tests/fixtures`](../../../tests/fixtures)。

## 3. 七组清理的生产调用链

| 清理项 | 最终职责与复核点 |
| --- | --- |
| canonical ABI | ordinary callable 和 layout callable 均按真实签名、表示、provider 和 target 检查；MIR/LIR/codegen 的生产路径不再按 `ConeIdentity::CORE` 跳过检查，也没有 core 专用空表规则 |
| native boundary | HIR producer 与 reader 查询实际依赖声明、CLayout 和完整字段/variant；删除 `TrustedCore` 外来类型分支、`ImportedCoreNativeBoundaryTypes`、`CoreNativeBoundaryNominal` 及固定身份恢复，保留 C-safe、ABI 和 GC 契约 |
| compiler protocol | 前端识别 intrinsic 和语言角色，完整 typed 声明、签名、effect 和依赖进入 IR；删除 CORE 来源资格、Core/NotCore 外层、重复 operation/String 协议及阶段资格包装 |
| String、初始化与 Link | String descriptor 使用真实 nominal exact；初始化服务进入普通 callable 支持记录；selected、ABI、definition、registration 和 relocation 通过共有 typed target 解析，删除独立服务 ABI、bridge、Link requirement/owner/proof 通道 |
| 通用预算与计费 | compiler、wire、slib、构建调度和 runtime 不再包含 `BudgetMeter`、逻辑 work/byte/node 计费或扫描展开配额；保留实际容量分配失败、整数溢出、格式宽度、对象范围与局部环检测；成本策略不进入 profile、fingerprint 或 runtime ABI |
| 重复证明与完整验证 | 发布直接使用同次完整结果；外部 reader 按边界检查后复用声明、继承图、类型、签名和布局；runtime 热点不重放不可变 scan，重型静态检查仅由显式 `SCOOP_VERIFY_METADATA=1` 或测试调用 |
| 无生产用途的框架 | 删除测试来源工厂、平行 identity-only reader/writer、访问证明表、默认值来源副本、发布凭证状态机和其专用测试；保留完整声明、默认值正文与定义位置、普通缓存摘要及 typed ID 查询 |

主要入口包括：

- [`layout_compile_closure/read.rs`](../../../compiler/slib/src/layout_compile_closure/read.rs)：完整外部产物的共有读取。
- [`artifact_production/layout.rs`](../../../compiler/driver/src/artifact_production/layout.rs)：实际编译结果、对象 requirement 与发布。
- [`profile/descriptor.rs`](../../../compiler/slib/src/profile/descriptor.rs)：只保存 id 和必需 section 清单的 profile。
- [`source_authority.rs`](../../../compiler/hir/src/cross_cone_type_semantics/source_authority.rs)：实际声明与默认参数查询，不保存授权或防伪状态。
- [`runtime/src/value_shape.c`](../../../runtime/src/value_shape.c)、[`value_scan.c`](../../../runtime/src/value_scan.c)、[`boxing.c`](../../../runtime/src/boxing.c) 和 [`arrays.c`](../../../runtime/src/arrays.c)：实际范围、scan、装箱、数组与 GC 合同。

普通名称解析、可见性、全局唯一且类型化的实体 ID、依赖环检测、缓存失效、符号/ABI 一致性与 GC 根检查继续保留。构建和前端中用于默认 core 依赖发现的身份判断只确定实际依赖，不授予后端资格。测试中的 CORE 示例不构成生产特例。

## 4. 产物格式

| 项目 | 当前版本 |
| --- | --- |
| 正式 production profile | `cross-cone-layout-strong/3` |
| 共有 HIR 声明与默认值 | `hir/cross-cone-interface/30` |
| HIR 类型语义 | `hir/cross-cone-type-semantics/8` |
| 前端协议角色 | `hir/core-bootstrap-interface/4` |
| MIR 类型接口 | `mir/cross-cone-type-bridge/1` |
| LIR layout/ABI | `lir/cross-cone-layout-abi/3` |
| layout Link 引用 | `lir/cross-cone-layout-link-closure/2` |
| 完整布局产物 | `lir/strong-production/12` |
| Link identity 引用 | `lir/link-identity-closure/3` |

被退役的字段、tag 和旧 major 不复用；不兼容的产物与缓存需要重建。保留的普通 Strong 表示使用 `/11`，完整布局产物使用 `/12`。实际 runtime C 调用约定和 String 表示保持不变，版本演进不再绑定预算或证明策略。

## 5. 验收命令

在 LLVM 22.1 的 Darwin/AArch64 环境，先格式化和 lint，再构建与验证：

```sh
export LLVM_SYS_221_PREFIX=/opt/homebrew/opt/llvm@22
export PATH="/opt/homebrew/opt/llvm@22/bin:$PATH"
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=0
export CARGO_PROFILE_TEST_DEBUG=0
cargo fmt --all
cargo clippy --workspace --all-targets
cargo build --workspace
SCOOP_TEST_PAIRED_SCOOPC="$PWD/target/debug/scoopc" \
  cargo test --workspace -- --test-threads=4
```

`SCOOP_TEST_PAIRED_SCOOPC` 必须指向本次构建的真实编译器；省略时，真实子进程、core/cache 和 artifact-only 输入消费测试会提前返回，不能据此认定这些场景已验收。正常验收不设置快照更新变量。

runtime 的普通与移动 GC、显式 metadata 检查和必要 C 测试由 workspace 测试执行，详见 [`runtime/README.md`](../../../runtime/README.md)。

## 6. 本次验收结果

2026-09-27，在上述 LLVM 22.1 / Darwin AArch64 环境完成：

| 检查 | 结果 |
| --- | --- |
| `cargo fmt --all` | 完成，最终代码符合格式要求 |
| `cargo clippy --workspace --all-targets` | 通过，无警告 |
| `cargo build --workspace` | 通过，构建真实配套 `scoop` 与 `scoopc` |
| 启用真实配套编译器的 `cargo test --workspace -- --test-threads=4` | **5084 通过，0 失败，0 忽略**；包含 driver 的 114 项测试（涵盖真实产物与组合场景）、runtime C/移动 GC 检查及 doc-tests |
| 最后删除两个无调用的重复 reader 入口后的 `cargo test -p scoop-slib` | **574 通过，0 失败，0 忽略**；全仓 lint 和构建再次通过 |
| 文档与调用链核对 | 三份 spec、ROADMAP、M23 总设计、M23-2～6 相关设计及 core 清理文档采用当前职责；验收文档的本地文件链接有效 |
| 构建目录清理 | 确认没有构建/测试进程且 `lsof` 未发现 target 内打开文件后，`cargo clean` 删除 2892 个文件，释放 6.0 GiB |

全仓测试对应 `2d095040a` 的完整实现，未设置任何快照更新变量。其后 `d9bdc4abb` 只删除两个没有调用者、与保留入口重复执行相同检查的 `self_describing` reader；上述 slib 回归、全仓 lint 和最终构建覆盖该删除，没有修改生产行为。`c4e7bfa82` 清除了旧设计中残留的 core bridge、发布凭证、重复 Compile/Link 验证、runtime 来源包装和缓存禁令。

最近的实际功能与职责提交包括：

- `64631a69e`：通过真实依赖产物消费 protected 嵌套声明。
- `6821e2f8f`：按 typed field identity 初始化继承字段，覆盖 ZST、大值、managed 引用与再次派生。
- `95e5218ef`：setter 可见性域由前端检查，reader 复用完整声明和普通引用规则。
- `92e538376`：限定 `super<I>` 保留真实接口默认方法候选。
- `2d095040a`：同步完整依赖声明、dispatch 和有限 shape-support 的三阶段 golden 与正常错误断言。

本阶段的源码编译、完整产物发布、仅产物下游消费、链接运行和七项清理均已完成。后续 generic/ODR、multi-image startup 与 artifact-only program-link 继续按 M23-7/8/9 实施。
