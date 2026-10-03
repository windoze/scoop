# M23-10 验收记录：一般 native 输入与链接闭包

状态：已完成并验收（2026-10-02）。最终代码、测试基线为 `06e347a5c`；前置为已验收的 [M23-9](../stage9/ACCEPTANCE.md)，完成门见 [阶段设计](DESIGN.md)。本记录之后的阶段状态提交只更新文档。

## 1. 正式入口与实际交付

`ArtifactLinkRequest.library_paths`、`scoop::link_built_program`、`scoop_linker::link_program` 与独立 `scoop-link --library-path` 使用同一组显式搜索根和同一个 native resolver。链接输入为完整 `.slib` 闭包、runtime 对象索引、目标 toolchain，以及逻辑 requirement 对应的已有 native 文件。program-link 只编译本次生成的 startup C；不会发现或编译 native companion 源码。

```sh
scoop-link --root-slib app.slib --dep-slib core.slib \
  --runtime-objects runtime/index.cbor \
  --library-path native --target aarch64-apple-darwin -o program
```

实际实现包括：

- direct Mach-O object、normal archive、dylib／标准 v4 `.tbd`／C 接口 framework 的显式定位、去重、target slice 和格式检查。静态输入保留不可变 bytes；archive member 使用 parent、physical ordinal 与 offset，名称只作诊断。
- 从真实 undefined 引用出发的有限工作集，支持 archive 成员链和跨归档回边；选中成员独立物化，ld 不再二次抽取 archive。未选成员只承担必要格式与符号索引检查。后续成员新增定义时，只重新检查受影响的既有动态绑定。
- 实际 export、普通 load、re-export、改名 re-export 和版本关系。按本次 binding 为各直接 provider 投影标准 stub，固定 libSystem 使用同一投影；最终 ordinal 与真实 owner、版本和必要 RPATH 一致。
- provider-owned extern 入口，以及跨 Cone 的 native global/TLS 读取、写入、更新和取址。导入的 GC intrinsic、callback 协议、泛型 `FunPtr` 和 native storage 默认表达式沿共有 HIR／具体化／MIR 路径消费原有语义。
- 精确 map／trace 输入集合、普通原生重定位表、最终静态／动态引用、EH／TLV 结构、保留段和原子输出。原生全局引用按对象精确路径及其保留符号定位，复用现有 ARM64 指令／fixup 检查；Darwin 的 synthesized TLV descriptor 通过原 TLS symbol 的唯一归属关联。

局部原生引用在对象读取时核对格式、目标和边界；ld 重建的 unwind 与调试段不按原位置重放字节校验。最终检查处理链接产生的新地址与绑定，不重新做语言类型推导、完整 ODR 内容计算或 runtime registry 验证。

## 2. 真实产物与功能组合

[原生输入运行器](../../../compiler/scoop/tests/program_link/native_inputs.rs) 调用真实配套 `scoopc`，产生 core、provider、facade 与 root `.slib`；在下游编译或链接前删除相应源树。native C 使用明确 compiler／SDK／deployment 预先构建。独立链接进程清空继承环境，使用受限 PATH 和不可用的 LLVM／Scoop sysroot locator。所有 native 文件通过 `--library-path` 定位，没有 fixture 专用 raw object 注入。

| 验收能力 | 实际检查 | 测试入口 |
| --- | --- | --- |
| Direct object | native 返回值、源码移除、默认值／泛型／re-export；相同 bytes 去重、不同内容歧义、坏 target／symbol kind／common／constructor | [direct](../../../compiler/scoop/tests/program_link/native_inputs/direct.rs) |
| Archive | 成员链、跨 archive 回边、多个 symbol、同名／相同 bytes 的物理成员、未选成员；TOC、thin／nested／坏范围、已选 weak／strong 冲突与缺 helper 引用链 | [archive](../../../compiler/scoop/tests/program_link/native_inputs/archive.rs) |
| 空工作集与去重 | 从真实产物取得 library requirement，用实际 archive 检查零抽取、后续单次抽取和重复需求；未选 constructor／autolink 不触发效果拒绝 | [选择器](../../../compiler/linker/src/program/native/selection/tests.rs) |
| Dynamic／framework | 重名 export 的独立显式选库、普通 load 与 re-export 区别、改名 re-export、framework、绝对 install name、`@rpath`、`@loader_path`、标准 stub | [dynamic](../../../compiler/scoop/tests/program_link/native_inputs/dynamic.rs)、[边界](../../../compiler/scoop/tests/program_link/native_inputs/dynamic/cases.rs) |
| Global／TLS | provider→facade→root 的读写、更新、取址、默认值与泛型；C 线程验证 TLS 独立、普通 global 共享，结束前 join／detach | [storage](../../../compiler/scoop/tests/program_link/native_inputs/storage.rs) |
| C ABI | 八种定宽整数边界、嵌套 packed/aligned CLayout、原始／nullable pointer、nullable `FunPtr`、静态 NoGC callback 的真实 C 布局与返回值 | [cabi](../../../compiler/scoop/tests/program_link/native_inputs/cabi.rs) |
| Scoop ABI | Managed 引用参数／返回、native roots、24 字节间接返回值的 publication、ZST；NoGc 的 GC-free 标量／ZST 路径，object／archive／dynamic 组合 | [scoopabi](../../../compiler/scoop/tests/program_link/native_inputs/scoopabi.rs) |
| Managed callback | 捕获引用、同线程重入、Reusable／OneShot、retain／release、外来线程 attach、实际 failure payload、两线程 TLS 与 GC | [callbacks](../../../compiler/scoop/tests/program_link/native_inputs/callbacks.rs) |
| GC intrinsic | 导入的 collect／stats，以及泛型 provider 内 pin／handle 的真实调用与回收期间存活 | [runtime](../../../compiler/scoop/tests/program_link/native_inputs/runtime.rs) |
| 四 Cone 混合 | archive helper→dylib、两个 Cone 共享同一泛型实例、root 本地类型、eager／lazy 各执行一次；输出 `100, 0, 200, 56, 96, 121, mixed-alive` | [mixed](../../../compiler/scoop/tests/program_link/native_inputs/mixed.rs) |

适用功能分别在普通模式和 `SCOOP_GC_STRESS_MOVE=1` 下运行。Managed Scoop ABI 的 C 实现检查引用确实移动、native root 更新，以及间接返回槽在再次 GC 后仍含更新的引用；callback／TLS 组合跨 FFI 保留 live String，并在 callback 内实际触发 GC。测试不是只观察某次整数返回或只比较 metadata。

新增 native fixture HIR／MIR／LIR golden **87 份**，来自 29 组实际 stage 输出。源码负例核对实际 source span 与具体诊断；包括 unsafe、只读存储、类型／取址、CLayout、静态 callback NoGC，以及 managed callback 签名、context、mode 和显式函数类型。

## 3. 负例、输入一致性与最终结果

| 边界 | 实际检查 | 测试入口 |
| --- | --- | --- |
| 全部声明合同 | library、C/Scoop ABI、GC effect、参数、结果、function/data、TLS、mutability 八类冲突；三个未调用 extern 的 Cone 均保留 origin，且在缺库检查前失败 | [contracts](../../../compiler/scoop/tests/program_link/native_inputs/contracts.rs) |
| 候选／工作集 | 默认命名空间不扫描搜索目录；后续 archive 定义不能接管显式动态绑定；物理 RPATH 与 install name 改变计划 | [resolution](../../../compiler/scoop/tests/program_link/native_inputs/resolution.rs) |
| 已读快照 | 读入后替换原对象并改指 symlink，仍消费原 bytes 并得到同一 fingerprint／结果；重新读取得到不同计划与结果，坏候选保持旧输出 | [输入一致性](../../../compiler/linker/src/link/tests.rs) |
| 原生格式 | 截断／越界／未知或重复 relocation、坏配对／target、指向调试符号、未知 load command、坏 compact unwind、错误 TLV bootstrap／key／template；普通和 `-O2 -g` 对象运行 | [formats](../../../compiler/scoop/tests/program_link/native_inputs/formats.rs) |
| EH 记录 | 从实际 LLVM 生成的 Mach-O 取得 EH 记录，检查正常读取，以及截断、坏 CIE 与 FDE 引用 | [EH](../../../compiler/linker/src/native_object/sections/tests.rs) |
| Map／trace | 额外、重复、遗漏 object／trace 输入，重复 object index、错误 symbol owner、越界 symbol range | [map／trace](../../../compiler/linker/src/link/map/tests.rs) |
| 原生静态结果 | 基于真实 executable 修改 branch target、函数指针或 map 地址，拒绝错误目标且保留原输出 | [静态引用](../../../compiler/linker/src/final_image/tests/native.rs) |
| 原生动态结果 | 基于真实 dylib executable 改错实际 library ordinal、版本或 RPATH，拒绝不匹配 | [动态引用](../../../compiler/linker/src/final_image/tests/dynamic.rs) |

交换依赖顺序、搜索根顺序与目录 symlink 别名后，四 Cone 混合程序的完整计划相同。native bytes、正式装载目录或 install name 变化则改变计划。已有 entry／provider／signature／alias／ODR／stackmap／fixup 损坏与原子输出回归继续保留。

## 4. 格式和范围

`.slib` outer schema、persistent identity、native contract schema、mangler、runtime metadata ABI **3** 与 runtime index schema **1** 保持。plan 使用 `scoop-resolved-link-plan-v2`；final-link profile 使用 `scoop-final-link-profile-v2`，第 9 字段为 native 输入规则 revision `1`。物理 native 输入／provider ID 仅用于本次 linker 输入与计划，不新增分发容器、来源凭证、资格状态机或预算体系。

本阶段仍只交付 Darwin/AArch64、单个主 executable 与静态完整 Cone 图。普通 native dylib/framework 不增加 Scoop image；不包含 `dlopen`／卸载、通用 blob handler、Cone 内 C/C++ producer、whole-archive 或 final-link cache。

M23-11 继续把同一个请求／resolver 接到公开 umbrella `scoop build/run/link`，迁移 single-file 和历史 fixture，删除旧 source/core 拼接与直链旁路。Stage 10 的供应能力已经在正式低层库与独立 `scoop-link` 交付，不留另一套 fixture linker。

另行记录一项共有 HIR 问题：参数自由导入 enum 的派生 `==` 候选仍有缺口，本阶段 callback 状态通过规范支持的 `when` 匹配验证。此次没有修改该比较候选导入问题；其后续修复应沿共有 HIR／provider helper 引用进行，不另建 native 比较路径。

## 5. 最终验证

本机为 Apple Silicon，Rust **1.99.0**、LLVM **22.1.8**。每批变更先 `cargo fmt --all` 与 `cargo clippy --workspace --all-targets`，再运行测试。最终回归启动前移除全部 `SCOOP_UPDATE_*`、含 `SNAPSHOT` 的 `SCOOP_*`、`INSTA_UPDATE`、`INSTA_FORCE_PASS`、`INSTA_ACCEPT_UNSEEN`、`UPDATE_EXPECT` 和 `RUST_MIN_STACK`。

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

最终 fmt、Clippy、配套构建和完整 workspace 的退出码均为 **0**，Clippy 无 warning。完整回归 **41 个测试组、5437 passed、0 failed、0 ignored**，耗时 **707.42 秒**。全部 snapshot 更新开关关闭，测试结束后没有源码或快照变更。

| 测试组 | 通过项数 |
| --- | --- |
| scoop 编排／runtime 构建 | 98 |
| stage 依赖边界 | 3 |
| program-link（含 39 项 native 输入测试） | 60 |
| codegen／runtime | 310 |
| HIR 数据／验证 | 865 |
| HIR lowering | 1340 |
| linker 输入／选择／最终文件 | 8 |
| LIR 数据／验证 | 473 |
| MIR lowering | 114 |
| `.slib` 产物读取／验证 | 586 |
| driver 端到端 | 227 |

此前关闭快照更新的各功能专项、选择器／快照单元、静态及动态 final mutation 测试也均通过；最终完成判定采用上面的同一次完整回归。

## 6. 功能提交与文件组织

| 提交 | 内容 |
| --- | --- |
| `9a7c724dd` | Stage 10 spec／design，明确范围与普通 native 输入合同 |
| `f16d13dcc` | 显式 object 定位、快照与独立进程链接 |
| `d9ed3ecf0` | archive 物理索引与有限符号工作集 |
| `dcf016f14` | dynamic export／依赖与标准 stub 投影 |
| `7eeb90403` | 跨 Cone native global／TLS |
| `4468ff654` | 共有导入 GC intrinsic 消费 |
| `ffe858747` | imported managed callback、foreign threads 与 moving GC |
| `d793adfa6` | 完整 C ABI 真实产物矩阵 |
| `6343a94c5` | Scoop ABI roots／结果 publication／ZST |
| `80abaa763` | 四 Cone archive／dylib／ODR／初始化组合 |
| `761ed771a` | 原生 relocation 与精确最终对象绑定 |
| `b7561635a` | 完整合同冲突及全部来源诊断 |
| `610b7dfaa` | 输入替换／symlink 快照、空工作集与后续归档冲突 |
| `07e189d99` | 原生 relocation／EH／TLS 必要格式检查 |
| `06e347a5c` | 最终 dynamic ordinal／版本／RPATH 负例 |

相对 `afa3cebe5` 新增 **74** 个 Rust/C 源文件，最长 **236** 行。对象定位、archive、dynamic graph／export、selection、原始 relocation、最终引用及 map／测试支持按真实职责拆分；复用已有 ARM64 解码与指令核验，不增加平行语义 verifier。

实施过程中分批执行 `cargo clean`。完整回归前最近一次清理删除 **2860** 个文件、**3.1 GiB**；最终全仓通过后再次清理 **3039** 个文件、**3.5 GiB**，仓库 `target` 目录已不存在。随后只修改验收文档与阶段状态。
