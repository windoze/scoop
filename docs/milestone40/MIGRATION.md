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

实际保留集合、迁移差异和最终统计在完成后补齐；不先全量刷新旧快照。
临时文件使用仓库 `target/m40/tmp`，定期清理已结束的用例工作目录，保留报告。
