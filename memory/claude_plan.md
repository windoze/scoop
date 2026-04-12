# 执行计划（公开摘要）

根据上层安全约束，这里记录可公开的执行计划与进度摘要，不写入内部完整推理细节。

## 初始计划

1. 检查最新一次 Git 提交，确认是否提到需要先修复的既有问题。
2. 阅读 `TODO.md`，找到第一个未完成任务。
3. 如首个未完成任务过大，拆分为更小子任务，并同步更新 `PLAN.md` 与 `TODO.md`。
4. 实现当前应执行的首个任务。
5. 运行相关测试与必要的 lint / check，修复出现的问题。
6. 更新 `TODO.md`、`PLAN.md` 与本文件，记录完成状态或阻塞原因。
7. 提交本轮修改并停止，不继续处理下一个任务。

## 当前状态

- 已创建本计划文件。
- 已检查最新提交：提交说明未额外提到需要先修的既有问题。
- 已定位首个未完成任务：`T2003c0c2b3c2-3`（拆分超长 lowering 并收口 handler scaffold/helper）。
- 已完成该任务的实现与验证，准备提交并停止。

## 细化执行步骤

1. 盘点 `effect/` 中当前最重的 lowering 入口，确认优先重构对象：
   - `matrix.rs` 中的 mixed escape site-matrix lowering
   - `mixed.rs` 中 immediate-resume + escape sibling lowering
   - `escape_continuation.rs` 中单 arm escape-continuation lowering
2. 抽取共享 helper：
   - sibling non-resuming arm 分类
   - effect dispatch / catch block 脚手架
   - 可复用的 handler block 组装结构
3. 将超长入口改为“分析/建计划/调用 helper”的编排式结构，避免继续堆积在单个大函数中。
4. 运行格式化、测试、LLVM fixture、clippy。
5. 更新 `TODO.md`、`PLAN.md`、本文件并提交。

## 已完成要点

1. 在 `effect/shared.rs` 新增共享 scaffold helper：
   - sibling non-resuming arm 分类
   - sibling dispatch/catch block 组装
   - escape handle block 组装
   - mixed-escape resume block 组装
2. `mixed.rs`、`matrix.rs` 与 `escape_continuation.rs` 已改为复用这些 helper，减少重复的 `dispatch/finally/catch` 骨架。
3. `escape_continuation.rs` 已把 nested perform site 扫描与相关 frame/state 结构下沉为模块级 helper，并用 `scan_escape_perform_sites` 收口主入口。

## 验证结果

- `cargo fmt --all --check`
- `cargo test --all`
- `cargo run -p scoop -- test`
- `cargo run -p scoop --features llvm -- test`
- `cargo clippy --workspace --all-targets -- -D warnings`

以上命令均已通过。
