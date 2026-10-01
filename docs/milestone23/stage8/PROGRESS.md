# M23-8 实施记录

状态：实施中。M23-7 验收提交为 `d548a07a1`；设计基线提交为 `3cd6c95a4`。本记录只报告已经实施和运行的内容，完整完成门仍见 [设计](DESIGN.md)。

## 1. root gateway 异常生命周期

- root gateway 在 `BeginCatch` 后通过现有 `MaterializeException` 生成托管对象，写入专用 failure root 后才 `EndCatch`。新增 managed call 使用完整 safepoint identity 与 root plan，native payload 不再离开 catch。
- root lowering 按职责拆入 `production/root.rs`；专门检查物化输入的根、独立输出、failure slot、入口 poll 与封闭失败返回值。同步 29 处既有 LIR golden，保留原函数语义。
- `cargo fmt --all`、`cargo clippy --workspace --all-targets` 通过。LIR lowering 143 项、codegen/runtime 302 项、slib 584 项全部通过；driver 的实际重建 core 与产物消费组合测试通过（包含 9 份受影响 golden）。测试时未启用 snapshot 自动更新。
- 首次 `cargo clean` 删除 1184 个文件，释放 2.8 GiB。验证使用 `target/m23-8`，opt-level 1、无 debug symbols、关闭增量，保留 debug assertions 与溢出检查。
- 多 image startup 尚未接通；root/eager gateway 完整结构拒绝检查和实际 startup 异常报告随后续功能验收。

## 待完成

1. 统一初始化 registration、删除 coordinator 副本及退役 tag，原子迁移 reader/publisher/runtime 格式。
2. 多 image 地址范围、六类记录、类型/scan、静态 roots、immortal 与初始化关系登记。
3. 完整 stackmap 规范化、registration 关联、合法 ODR 合并及 GC 接入。
4. EntryPending、逐次 gateway 线程握手、canonical eager 顺序、异常报告与 shutdown。
5. 真实 3+ Cone 产物运行、损坏输入和受控线程竞争矩阵，以及历史运行辅助迁移和最终全仓回归。
