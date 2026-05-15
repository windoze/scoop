# 当前执行计划

## 约束

- 以 `TODO.md` 为唯一任务顺序和完成状态来源。
- 只完成第一个标题未带 `[DONE]` 的任务，然后停止。
- 如遇到阻塞当前任务的规格不匹配、缺失语言特性、回归或实现边界，先修复；若无法在当前任务中正确修复，则在 `TODO.md` 中插入最小必要前置任务并停止。
- 不使用 workaround、fixture-only hack 或弱化测试形状来绕过问题。
- 完成后必须更新 `TODO.md`，运行相关验证，提交 Git commit。

## 步骤计划

1. 阅读 `TODO.md`，按标题 `[DONE]` 前缀识别第一个未完成任务。
2. 查看该任务的依赖、验收标准、完成记录和相关说明。
3. 根据任务范围检查最小必要代码上下文，不进行开放式历史问题扫描。
4. 实现当前任务或处理直接阻塞该任务的前置问题。
5. 添加或更新最小相关测试、fixture 或文档。
6. 运行任务指定验证和必要的相关测试；如失败，定位并修复。
7. 将当前任务标题标记为 `[DONE]`，更新 completion record。
8. 更新本文件记录关键进展、验证结果和最终状态。
9. 检查 Git 状态和 diff，提交本次任务全部相关改动。
10. 停止，不继续处理下一个任务。

## 进度

- 已创建初始执行计划。
- 已读取 `TODO.md`，第一个未完成任务为 `P4-T02：迁移 remaining string helper，明确 substrate 边界并收缩 runtime surface`。
- 最新提交为 `[P4-T01] Migrate scalar toString to sysroot methods`，与当前任务顺序相邻但未明确留下未完成 blocker；继续执行 `P4-T02`。
- 已将 public `String.length/toInt/concat/hash/isEmpty/replace/charAt/repeat/compareTo/trimIndent` 接入 sysroot body；`String.concat` 通过 audited named intrinsic runtime bridge 触达 allocation/copy substrate。
- 已清理 resolver/typecheck/HIR/LLVM/effect-lowered 中上述 public helper 的按名 intercept；保留 `byteLength/getByte/unsafeSliceBytes` 作为 byte-level substrate。
- 已从 runtime direct API/ABI 中移除 `length/toInt/hash/isEmpty/replace/charAt/repeat/compareTo/trimIndent` 等 public helper；`scoop_string_concat` 暂作为 concat bridge 的 substrate symbol 保留。
- 已完成主要验证：`cargo check -p scoopc`、`cargo test -p scoopc llvm::tests -- --nocapture`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`、`cargo run -p scoop -- test --fixtures tests/fixtures/build`、`SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/managed_abi_string_helpers_basic.scoop`、`cargo clippy --all-targets -- -D warnings` 均通过。

## 当前任务执行细化

1. 检查 string helper 当前声明、typecheck/resolver/HIR/LLVM/runtime intercept 分布。
2. 判断 `length` / `byteLength` / `getByte` / `unsafeSliceBytes` 是否为 runtime substrate，并把结论写入 sysroot/runtime 文档注释。
3. 将非 substrate helper 迁移到 `sysroot/string.scoop` 或 `sysroot/core.scoop` 的普通/managed ABI helper 实现。
4. 删除对应 resolver/typecheck/HIR/LLVM/runtime 的名字特判与 runtime symbol 暴露。
5. 增加覆盖 migrated helper 的 build/run-pass/GC-stress fixture。
6. 运行任务要求验证与 clippy；修复失败。
7. 更新 `TODO.md` 的 `[DONE]` 标记和完成记录，提交改动。
