# 本轮执行记录

## 目标
- 按用户要求，先检查最新提交是否提到需要先修复的既有问题。
- 然后确认 `TODO.md` 中第一个未完成任务。
- 但基于上一轮已完成的探查结果，当前已确认存在一个必须优先修复的既有缺陷：
  - LLVM 路径下，boxed enum variant 携带 function-type payload 时，若通过 `val Variant(...) = expr` 解构绑定提取函数值并调用，会在 build 阶段因 `value coercion` 失败而报错。
- 因此本轮实际优先事项是修复该既有缺陷，并补充回归覆盖；只有在该缺陷修复完成后，才判断 `TODO.md` 首个任务是否可以一并完成。

## 已知事实
- 已确认 boxed enum 构造本身可用。
- 已确认 boxed enum `when` 解构路径本身可用。
- 已确认失败集中在 boxed enum `val` destructuring binder + callable 调用的组合路径。
- 已确认问题表现为 LLVM codegen 阶段出现 `Ref -> Int` 的错误 coercion。
- 已有未提交调试与半成品修复改动，需先审阅并整理，再继续推进。

## 关键进展
- 已确认 `patterns.rs` 中为 variant pattern 合成 binder 恢复真实字段类型的改动是必要的，需保留：
  - boxed enum function payload 在隐藏 `when` binder 上不能继续退化成 `Any`。
- 已继续把故障从“enum 字段提取”缩到“隐藏 `Raise.raise(...)` 的 HIR 类型错误”：
  - boxed destructuring 生成的字段提取类型与最终 `ValDecl` 目标类型本身都正确；
  - 真正失败的是 pattern lowering 生成的隐藏 `Raise.raise(RuntimeError.NullAssertionFailed)` 一律被标成 `Any`；
  - ordinary callee suspend plan 因此把 `Raise.raise` 的 `resume_slot_ty` 误建模成 `Ref`，在 `n: Int` 这类 boxed multi-field destructuring 路径上被恢复为 `Ref -> Int` coercion 失败。
- 已实施修复：
  - `synth_raise_null_assertion_failed()` 生成的隐藏 `Perform` 节点现改为 `Nothing` 类型；
  - 同时把该隐藏 `Perform` 的 span 改成基于原位置的零宽 span，避免和外层合成 `when` 使用完全相同的 span，降低 resume-tail 重写误命中的风险。
- 已用最小复现验证：
  - `/tmp/probe_drive_pattern_nHKYt8.scoop` 现已成功 build；
  - 运行产物返回值为 `8`，与预期一致。
- 已补正式回归与全量验证：
  - 新增 `tests/fixtures/run-pass/enum_function_payload_boxed_multi_field_basic.scoop`，锁定 boxed multi-field enum function payload 的 ctor、`when` 解构调用与 `val` 解构调用。
  - 已同步 `tests/fixtures/hir/local_val_destructuring_lowering.hir` 与 `tests/fixtures/hir/safe_call_not_null_assert.hir`，反映 hidden `Raise.raise(...)` 现在为 `Nothing` 类型且使用零宽 span。
  - 已通过：
    - `cargo run -p scoop -- build tests/fixtures/run-pass/enum_function_payload_boxed_multi_field_basic.scoop -o /tmp/enum_function_payload_boxed_multi_field_basic.out`
    - `/tmp/enum_function_payload_boxed_multi_field_basic.out`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`
    - `cargo test -p scoopc segment_dump_classifies_ -- --nocapture`
    - `cargo run -p scoop -- test`
    - `cargo test --all`
    - `cargo clippy --all-targets -- -D warnings`
- 当前结论：
  - 这次既有缺陷已收口，且 `T4016T1R` 的 review 条件已满足；
  - `TODO.md` / `PLAN.md` 下一步应切到 `T4016T2`。

## 执行计划
1. 查看最新提交信息，确认是否显式提到需要先修的既有问题。
2. 查看 `TODO.md`、`PLAN.md`、当前 git 状态，确认本轮边界与已有改动。
3. 审阅当前未提交改动，区分：
   - 真实修复候选代码；
   - 临时调试打印；
   - 仍需补充或回退的部分。
4. 继续定位 boxed enum destructuring 的 `Ref -> Int` 类型错配来源，重点检查：
   - HIR lowering 的 pattern binder 类型推断；
   - local pattern val lowering 生成的多个 `ValDecl` 与 initializer 的关联；
   - LLVM codegen 中 `ValDecl` 初始化、局部引用与调用路径。
5. 实施最小且正确的修复，不引入 workaround，不放宽语义。
6. 删除所有临时调试输出，保证最终代码干净。
7. 增加正式回归测试，覆盖 boxed enum function payload 在：
   - `when` 解构调用；
   - `val Variant(...) = expr` 解构调用；
   两条路径上的行为。
8. 运行相关测试与质量检查；若发现其它既有问题，立即转为优先修复。
9. 若回归通过，则将当前 bugfix 视为 `T4016T1R` 的必要完成条件之一，并同步更新 `TODO.md` 与 `PLAN.md`。
10. 提交本轮变更并停止，不继续下一项任务。

## 约束
- 全程使用中文记录。
- 不接受 workaround、fixture-only hack 或规避真实缺陷的变通。
- 所有文件编辑使用 `apply_patch`。
- 不回退不属于本轮的用户改动。
- 在关键进展、计划变化、完成主要步骤时持续更新本文件。
