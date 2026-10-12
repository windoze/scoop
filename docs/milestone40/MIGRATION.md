# M40 fixture 职责迁移

## 实际基线

实施起点为 `b59294fbcadce0c655cba08258497c5e7d5c28a4`（M37 合入后的 main），
不是设计稿统计的 `22dee6b14`。M38/M39 设计文件保持原样，不假定其实现存在。
按 Git 跟踪的 `tests/fixtures/` 文件逻辑字节统计，不计 Git 历史或运行工作区：

| 项目 | 实施前 |
| --- | ---: |
| 文件 | 32,890 |
| 内容字节 | 8,292,107,894 |
| Scoop 源文件 | 4,939 |
| fixture / 变体 | 3,771 / 4,242 |
| snapshot 声明 | 12,257 |
| Darwin / GNU / musl 适用 fixture | 3,755 / 3,741 / 3,739 |

## 结构代表组

优先复用已有 typed IR 断言，不为本次迁移增加同一规则的第二套 fixture。
以下是阅读和验证入口，不是 runner 注册表，也不限制未来必要的结构测试。

| 职责 | 代表入口 | 实际断言 |
| --- | --- | --- |
| AST 语法树 | `compiler/parser/src/tests.rs`、`tests_m35.rs`、`tests_m37_receivers.rs` | 节点种类、调用参数、声明形状、源码 span；不要求每个运行用例保存 AST |
| HIR 类型与调用 | `compiler/hir-lower/src/tests/m3/generics.rs`、`m17/default_preparation/` | typed 参数、具体化、选中 callable、默认参数映射及求值准备 |
| HIR 跨 Cone 与捕获/分派 | `compiler/hir-lower/src/tests/m23_type_semantics_production/{source_default_captures,source_dispatch,generic_bodies}.rs` | 实际持久实体、捕获类型和引用目标、导出正文及具体化关系 |
| MIR 控制流/清理 | `compiler/mir-lower/src/tests/control_flow.rs` | loop header/backedge、分支目标、return 值先存储再 finally、catch cleanup 次数 |
| MIR 协程 | `compiler/mir-lower/src/tests/coroutines.rs` | hidden ABI、frame live slots、resume point、跨挂起循环目标 |
| LIR 布局/ABI | `compiler/lir-lower/src/tests/{exact_layouts,callable_abi,zst_places}.rs` | exact type 与物理布局、ZST、GC 参数与返回 carrier，不以字面关键词替代 |
| LIR 根与 relocation | `compiler/lir-lower/src/safepoints/`、`compiler/codegen/src/statepoint_tests/` | root scan、位置/对齐、存储与活跃性、优化后 machine roots/relocation |
| 优化结构 | `tests/fixtures/m31-optimization/local-values`、`compiler/mir/src/` 的优化测试 | debug/release 实际 CFG、常量/值传播及副作用；运行输出仍精确检查 |
| ODR/符号绑定 | `compiler/driver/src/request/preflight/end_to_end_tests/generic_bodies/machine.rs`、`compiler/codegen/src/tests/{smoke,linux_codegen/objects}.rs`、`compiler/slib/src/link_object/symbol_verification/tests.rs` | 实际 shared ODR object、弱/强绑定、ELF coalescing、拒绝错误物理符号；CLI provider/consumer 链接及运行保留 |

完整 HIR 调试输出继续可用，但不提交其中递归展开的 CrossCone/LocalConcrete 世界。
因此不改生产 dump 格式、identity 或校验实现，也不对 ID/hash 作额外归一化。
普通功能保留独立、组合、negative、跨 Cone、artifact-only、debug/release、moving/minor GC
原有路径；准确诊断、输出原子性和行为断言保持。

## 分批处理

1. 修订 AGENTS 与两个测试 README，普通用例示例默认不请求 dump。
2. 删除普通用例完整 nm 快照和仅为其存在的步骤；专门的 native 选择、绑定及 ODR 检查按真实合同保留。
3. 移除普通 IR snapshot 引用和无消费者的 emit/dump 参数；观察模式专测保留实际输出与缓存合同。
4. 保留小而明确的结构/优化期望；检查整个发现集合，记录源文件、诊断、target 与执行路径没有丢失。
5. 本机格式/lint、公共 runner 规则测试及针对性验证后，再做 Darwin、GNU、musl 正式验收。

实际保留集合和迁移差异见以下批次说明，最终统计与各目标结果见[验收记录](ACCEPTANCE.md)；不批量刷新旧快照。
临时文件使用仓库 `target/m40/tmp`，定期清理已结束的用例工作目录，保留报告。

### 符号快照批次

删除 2,068 个完整符号文件和 690 个仅为它们存在的 nm 步骤。
`archive-chain` 的 `unused.o` 既有 constructor 引用缺失的 `m23_missing_unused`，
且 Mach-O 带不可用 library option；误选成员会使真实链接失败。
其精确 link plan 继续断言 `unused.o selected=false`，成功链条仍运行并检查 `42\n7\n`，
因此无需重复冻结 executable 的其他符号。
ODR 的 typed 物理定义与强弱绑定测试保持原样，普通多 consumer 的链接/运行路径保持原样。

本机 fmt/workspace clippy、Ruff、44 项公共 runner 测试通过；
`generic-equality-payload` 与 `native-link-archive-chain` 两项 CLI 验证通过
（20 个进程，见 [报告](validation/symbols-darwin.json)）。

### IR 批次与保留范围

普通用例移除 11,508 条 AST/HIR/MIR/LIR snapshot 声明及 1,559 条仅证明 dump
强制重编译的单子进程计数。逐声明比较确认所有 inputs、support、targets、variants、
非观察 argv、源码准备/隔离步骤、退出码、signal、stdout/stderr、完整诊断和其他 checks 保持。
观察模式三项专测保留 dump 请求：`cli-build-warm-observation-link`、
`cli-process-source-observations`、`source-locations-cross-cone-relocation`；
文件存在/集合、同次产物一致性、warm 重编译及重定位 dump 相等性仍被检查。

文件 golden 收敛为 9 项专测，每项只请求一个实际检查的阶段：

- MIR：`legacy-m8-exceptions-return-paths`、`m31-local-values`、
  `m34-inlining-artifacts`、`m34-inlining-decisions`、
  `m34-devirtualization-locals`、`m34-devirtualization-artifacts`。
- LIR：`m31-ssa-roots`、`m34-small-values-native`、`m34-small-values-artifacts`。

保留的 MIR 检查返回/清理顺序、优化分支、真实内联与本地/导入分派目标；
LIR 检查引用活跃位置、聚合 carrier 与 Scoop/C ABI 及跨 Cone 调用。
原来的 debug/release、moving/minor 和三 target 配置不变。
已有 provider/default-template、getter/setter 按需物化及来源位置的局部结构 checks
继续保留，只请求其实际使用的阶段；没有把整份 golden 换成新加的关键词匹配。
库产物发布用例继续编译真实 provider/consumer；它们的布局/分派结构由对应 typed IR
专测负责，不把仅能成功产生 library 的用例记作运行行为验证。

本机四个 stage crate 的单元测试 2,200 项及 parser doctest 1 项通过。
8 项迁移选测通过（11 变体、57 进程、6 golden），见 [报告](validation/ir-darwin.json)。
未批量更新任何 IR 期望，保留的 golden 使用原有字节比较。
六项 ABI/内联/去虚化专测补验也通过（12 变体、66 进程、18 golden），
见 [报告](validation/structures-darwin.json)。

### Native 链接计划批次

79 份 native 专测计划原来约 57 MB，多数内容是 core 的 startup 符号与生成 C。
新增局部 snapshot 规则 `native-link-plan`，完整比较计划中 `link inputs:` 到
`startup object references:` 之前的区段；不按关键字挑选几行，也不改生产 dump。
所选区段保留 native library 身份与输入顺序、对象 slice、archive member 的范围/摘要关系/
选择状态、dynamic provider、load/re-export、rpath 和准确绑定；未知新字段仍导致差异。
缺失/重复边界或不同版本报错，避免错误输入被投影成空字符串而通过。
原有 `native-digests` 只处理 native 内容摘要的相等/不同关系，其他身份与 ABI 未归一化。

期望由原有已验收计划直接截取同一区段，不使用编译器批量刷新；
启动、root、runtime registration 的物理关系由现有 typed 链接测试和真实程序执行覆盖。
测试使用真实比较入口验证 selection、range、digest 关系、动态目标、
weak 属性与未知字段的变化都会使只读验证失败。
Ruff 与 47 项公共规则测试通过；archive-chain、dynamic-reexport、runtime-location
三项 CLI 验证通过（27 进程、4 golden），见 [报告](validation/native-plan-darwin.json)。

ODR 补验：本机 codegen/slib 的 15 项选测和 driver 的真实 provider/consumer
共享 ODR 产物测试通过；后者直接检查实际生成对象和跨 consumer 的实体复用。

### 残余重复

`m31-ssa-roots` 的三 target LIR 字节完全相同，合并为各 profile 一份共享文件，
删除四个副本；三个 target 与 debug/release/minor 的执行矩阵保持。
`native-link-runtime-gc` 和 `native-link-runtime-handles` 没有用户 native 输入，
投影后的计划只剩固定链接顺序标题及 ELF 系统导入计数，故删除六份无关计划和
对应 `--dump-plan` 请求。构建/独立链接指纹相等、普通及 moving GC 运行仍精确检查。
这也闭合了 Darwin 全量发现的两项遗漏投影规则问题，而不是继续保留空泛快照。
本机 SSA roots 三变体以及两个 GC 用例四变体补验通过。

### 损坏产物诊断

Darwin 全量发现截断专测冻结了旧产物长度 `158558`，本次原始产物实际为 `158562`。
复用已有 `await_file` 的字节读取，将诊断中的预计长度精确绑定到本次完整产物长度；
截断位置 `158557`、完整诊断的其他字段、损坏成员与 magic 检查、失败不覆盖旧程序
和最终普通/moving 运行全部保持。本机完整执行该用例通过。
这是唯一修改的既有诊断 JSON；未模糊匹配错误信息，也未新增 runner 规则或逐用例脚本。

### 产物持有测试的边界

`program-link-retained-artifacts` 从 18 步、10 次进程调用收敛为 6 步、4 次调用：
同步构建一次，保存 root/core 产物并删除源码，用隔离的 `scoop link` 和空 sysroot
独立链接，再分别运行构建与独立链接的程序。保留依赖身份、链接计划指纹相等和
两次 `42\n` 的精确输出；删除 FIFO、后台构建、缓存等待、中途删除和重复预热，
同时移除不再使用的显式依赖 manifest。未调整超时。

实现规范 2.7 明确产物持有保证以构建图完成为边界。
已有 `completed_nodes_retain_shared_bytes_without_reopening_paths` 改为实际创建并删除
compiled/cache 路径，继续核对共享字节和闭包对象。新增 86 行 Rust 集成测试通过
真实构建图的 `execute().into_outcome()`，在返回后删除源码及 root/core 缓存文件，
再调用现有 `read_built_program`，精确核对闭包数量、identity 和产物指纹。
不增加生产代码、测试钩子或同步框架，也不承诺构建期间任意删除文件仍成功。

重复的 moving GC 运行由 `native-link-runtime-gc`、`native-link-runtime-handles`
的构建/独立链接双路径以及 `m31-ssa-roots` 的 moving/minor 变体覆盖。
阶段结构沿用上表的 typed IR 和 9 项专测；本用例不再承担阶段 golden 或 GC 压力验证。

两机先完成 `cargo fmt --all` / workspace clippy，再运行上述两个 Rust 测试，均通过。
本机真实构建图测试耗时 38.51 秒，Linux 为 174.01 秒；两者均实际删除文件后成功读取
root 与 core 的 Link 数据。CLI [Darwin](validation/retained-darwin.json) 单项通过
（4 进程）；[GNU](validation/retained-gnu.json) 和 [musl](validation/retained-musl.json)
分别验证本用例及上述三个相关专测，均为 4 项、8 变体、32 进程、3 次 golden 比较通过。
本用例在 GNU/musl 分别耗时 5.74/12.15 秒，复用原验收缓存；Darwin 空缓存耗时 44.97 秒。
本批未重复全量测试；已清理结束的 CLI case 和本机 dev profile，报告留存。
