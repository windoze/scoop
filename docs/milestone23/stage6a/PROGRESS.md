# M23-6a 实现进度

目标：完成 [设计](DESIGN.md) 第 8 节的全部完成门；每个功能先格式化、lint 和验证，再提交。当前为实现中，已有 Stage 7 功能与 fixture 仅作为迁移基线，不代表共同 HIR 已完成。

## 实施顺序

1. 统一 nominal application 与声明查询，迁移类型关系、成员、继承、conformance 和模式。
2. 统一候选、参数映射和上下文推断，删除导入专用的语言处理调度。
3. 统一普通、默认、泛型和初始化正文，接入正式编解码，删除 imported template 的二次语义构建。
4. 统一具体化请求和实际物化需求，移除 source-only gate 并适配 MIR 输入。
5. 完成内存／wire 等价、声明位置变化、双向泛型、真实产物再次发布和普通／移动 GC 回归，删除被替代路径。

## 接续基线

- 工作区已有 `for` 在 Export HIR 前展开的实现和 `hir/cross-cone-interface/40` 迁移；沿用实际声明、调用、Option 和 while 节点，删除专用 For／binding-plan 运输及重复遍历。
- 三份 spec、M23 总设计和 Stage 7 职责已先行修订，明确声明来源不改变语言语义，普通封闭泛型 application 不能被历史阶段门排除。
- 本次接续已完成 `cargo fmt --all`、LLVM 22.1 下的 `cargo clippy --workspace --all-targets` 和 `cargo build --workspace`，均通过。启用本次实际配套 `scoopc` 的 workspace 回归为 5208 passed、1 failed、0 ignored；唯一失败是 `/40` 迁移遗漏的两份 Link Code 指纹快照。
- 更新这两份快照后，关闭更新开关，以同一配套编译器和原测试二进制复验 `property_initialization_uses_close_source_mir_lir_and_both_artifact_views`，1 passed、0 failed。基线共 5209 个测试均已覆盖通过；原完整回归日志和专项复验日志分别为 `/tmp/scoop-m23-6a-baseline-test.log`、`/tmp/scoop-m23-6a-baseline-code-verify.log`。
- 已清理约 13 GiB 的 `target/debug/incremental` 缓存；阶段验证使用禁用增量编译和调试符号的构建配置。

## 尚未完成

设计中的共同实体、正文与具体化迁移仍须实现。不能把基线回归或某个共同查询通过，作为删除来源专用语义模型的替代验收。

## 共同字段查询与绑定展开

- struct 字段访问、不可失败解构、`when` 模式与穷尽性分析使用同一完整 application 字段查询；模式直接保留完整 owner 类型，适用于当前及依赖声明。迁移期查询内部仍适配两种存储，后续须随 nominal 模型统一而删除。
- val／var、lambda 与 for 绑定直接生成普通声明、字段访问和已选 component 调用，删除只为重放前一步而建立的 `IrrefutableBindingPlan`、shape 与 action。失败按原事务回滚；被解构表达式求值一次，class 的 `_` 仍按顺序调用 component。
- 解构主文件由 829 行降至 194 行，聚合处理 106 行、struct 模式名称解析 69 行。没有新增 wire 节点或 section 版本。
- `m23-shared-bindings` 包含 5 组正例、11 组反例与 HIR／MIR／LIR、诊断快照；覆盖字段／component／扩展、nested／rest／alias、lambda／for 捕获、默认值／when、宽值／引用／Unit。使用真实产物、移走源码、再次发布和普通／移动 GC 运行。
- 验证：全仓 fmt／clippy 通过；HIR、HIR lowering、MIR lowering 与 slib 的 2840 项回归通过；新增两个完整 fixture 测试在关闭更新开关后通过，日志 `/tmp/scoop-m23-6a-bindings-verify.log`。
- 组合探测另确认依赖 Boolean／String 字面量模式的 ordinary `equals` 仍被旧 imported 调用入口限制（例如 `Pair<Boolean>` 中的 `Pair(true, _)`）。该缺口归共同候选／调用及正文迁移，未用阶段限制作为最终合法性规则；本批穷尽性回归使用嵌套 Option 及 guarded struct 模式。
