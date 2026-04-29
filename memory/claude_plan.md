# 当前执行计划

1. 检查最新一次 Git 提交的提交信息与变更说明，确认是否提到了尚未修复的既有问题；如果提到，优先修复该问题。
2. 阅读 `TODO.md`，定位第一个未完成任务，并阅读 `PLAN.md` 了解当前计划与依赖关系。
3. 评估该任务是否可在本轮完整完成；如果范围过大或存在前置缺口，则先把任务拆分并更新 `TODO.md` 与 `PLAN.md`，本轮只执行新的首个子任务。
4. 实现本轮目标任务，尽量做最小且正确的修改，不引入规避问题的临时方案。
5. 运行与改动直接相关的测试；如过程中发现已有缺陷、回归、规范不匹配或实现边界缺失，立即优先修复，或将其作为前置任务插入 `TODO.md` 并停止继续后续任务。
6. 完成后更新 `TODO.md`、`PLAN.md` 和本文件，记录完成情况、阻塞关系与验证结果。
7. 按仓库既有风格创建一次 Git 提交，然后停止，等待下一轮调用。

## 当前进度

- 已检查最新提交 `60bff1b`，提交信息未引入需要先修复的既有 issue 说明。
- 已确认首个未完成任务是 `T5001aR`（baseline review）。
- 已复核 `ROOT_FRAME_REFACTOR.md` 第 4.4 节与对应代码入口：
  - runtime 侧确实覆盖了 stackmap roots 主路径、`native_roots`、runtime init 对 stackmap registry 的默认依赖，以及 heap/global/pinned/handle roots 的非 stackmap visitor 入口；
  - 编译器侧确实覆盖了 ordinary safepoint、`@Extern` native 边界、`extra_gc_root_slots` / hidden sret / indirect aggregate spill、以及 ordinary resume / effect state-machine 继续执行路径。
- 当前判断：baseline 足以支撑后续“先抽 root map，再上 runtime substrate，再接编译器”的顺序；未发现需要先插入到 `T5001b` 之前的新前置任务。

## 校验结果

- 已更新 `TODO.md`：将 `T5001aR` 标记为完成，并写入 review 结论。
- 已更新 `PLAN.md`：补充 `T5001aR` 的复核结论，明确 baseline 已覆盖 `T5001b` 所需的关键热点与推进顺序。
- 已运行：
  - `cargo test --all` 通过。
  - `cargo clippy --all-targets -- -D warnings` 通过。

## 本轮收尾

- 下一步仅剩整理 diff 并创建一次 `T5001aR` 提交；提交后停止，不继续执行 `T5001b`。

## 约束

- 不跳过既有问题，不以变通方案绕过规范缺口。
- 仅完成一个任务或一个新拆出的首个子任务。
- 在执行过程中若计划发生变化或完成关键步骤，及时更新本文件。
