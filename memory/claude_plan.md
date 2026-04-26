# 执行计划

说明：按要求先写入可审阅的计划与进度记录。我不会在这里写出不可审阅的内部推理细节，但会持续维护完整的执行步骤、判断依据摘要、阻塞项与结果。

## 初始计划

1. 检查最新一次 Git 提交信息，确认是否显式提到需要先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，核对当前计划与任务顺序是否一致。
4. 如果首个未完成任务过大，先把它拆分为可执行子任务，并同步更新 `PLAN.md` 与 `TODO.md`。
5. 实现当前应执行的第一个任务。
6. 运行相关测试、格式化、`clippy`，修复过程中发现的既有问题或回归。
7. 更新 `TODO.md`、`PLAN.md` 与本文件，记录完成情况或新的前置依赖。
8. 提交 Git commit，然后停止，不继续做下一个任务。

## 执行记录

- 状态：已完成最新提交、`TODO.md`、`PLAN.md` 的初步检查。
- 已确认：
  - 最新提交为 `3ffdb1a3c40ff39e198e35a504e544709f0f3c91`，主题是删除待重做的早期 sysroot surface；
  - `TODO.md` 中首个未完成条目是 `T5000e3bR Review：确认 sysroot surface 已实质缩回到仍承诺维护的最小集合`；
  - 当前轮次目标不是推进新功能，而是先复核上一提交是否留下既有问题或边界遗漏。
- 接下来要做的检查：
  1. 核对被移除的 sysroot 模块是否确实已从 `sysroot/`、fixture、文档、编译器可见面中消失。
  2. 核对当前保留的 sysroot surface 是否与文档承诺一致。
  3. 运行与本次 review 直接相关的测试、全量测试与 `clippy`。
  4. 如果发现既有问题，优先修复或按依赖顺序回写 `TODO.md` / `PLAN.md`。

## 完成情况

- 已完成 `T5000e3bR`。
- review 期间发现的既有问题：
  - `PLATFORM_API_SURFACE_AUDIT.md` 仍把已删除的 platform surface 写成现行支持；
  - `STDLIB_COMPLETENESS.md` 仍把已删除 surface 与 `stdlib/test.scoop` 记为已完成；
  - `STDLIB_DESIGN.md` 的目标模块树缺少“future target，不等同当前 shipped surface”的状态说明。
- 已完成修复：
  - 回写上述 3 份文档，使其与 `T5000e3b` 后的实际 sysroot 边界一致。
- 已完成验证：
  - `cargo run -p scoop -- test` -> `fixtures: ok (1197)`
  - `cargo test --all` -> 通过
  - `cargo clippy --all-targets -- -D warnings` -> 通过
- review 结论：
  - 当前 sysroot 已收敛到 `collections/core/delegates/print/process/string/sync/task/thread/unsafe`；
  - 已移除的 `channels/env/fs/io/net/path/test/time` 不再出现在 sysroot、runtime ABI、LLVM codegen dispatch 或 fixture 主路径；
  - 当前唯一保留的过渡 platform surface 是 `scoop.process`，下一条任务即 `T5000e3c`。
