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
- 本次接续已完成 `cargo fmt --all`、LLVM 22.1 下的 `cargo clippy --workspace --all-targets` 和 `cargo build --workspace`，均通过。启用本次实际配套 `scoopc` 的 workspace 回归正在执行，关闭全部快照更新开关。

## 尚未完成

设计中的共同实体、正文与具体化迁移仍须实现。不能把基线回归或某个共同查询通过，作为删除来源专用语义模型的替代验收。
