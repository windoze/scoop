# 当前执行计划

## 思路摘要
- 目标是严格按 `TODO.md` 顺序完成第一个标题未带 `[DONE]` 的任务，然后停止。
- `TODO.md` 是任务状态、依赖、验证要求和完成记录的唯一依据；`PLAN.md` 仅在阶段级计划变化时更新。
- 如果当前任务被具体实现缺口或规格不匹配阻塞，将只添加最小必要的前置任务并提交，不用变通方案继续推进。
- 本文件记录可公开的执行计划和进度，不包含隐藏推理细节。

## 步骤计划
1. 读取 `TODO.md`，识别第一个未完成任务及其要求、依赖和验证项。
2. 检查最近提交信息是否明确提到与该任务直接相关的未完成问题；如相关，将其纳入当前任务或作为前置任务记录。
3. 读取当前任务涉及的代码、测试、规格或 fixture，确认现有实现边界。
4. 按任务要求做最小且完整的实现，不采用 fixture-only hack、特例绕过或规格偏离。
5. 添加或更新相关测试与 fixture，覆盖任务要求和发现的同类问题。
6. 运行任务要求的验证命令及必要的相关测试；如失败，定位并修复。
7. 更新 `TODO.md`：在任务标题前加 `[DONE]`，填写完成记录；仅在阶段计划确实变化时更新 `PLAN.md`。
8. 运行提交前检查，审查待提交改动，确保不包含无关或敏感文件。
9. 使用符合仓库风格的提交信息提交本次任务的全部相关改动。
10. 停止，不继续处理下一个任务。

## 进度记录
- 已写入初始执行计划，下一步读取 `TODO.md` 选择第一个未完成任务。
- 已确认当前任务为 `P6-T02：删除 LLVM 阶段 f-string codegen 后门 + sysroot 文件 f-string 使用 lint`。
- 最新提交 `d9997865 [P6-T01] Desugar f-strings through StringBuilder` 直接对应前置任务完成状态，未声明需要插入的未完成阻塞项。
- 下一步将定位 f-string codegen / MIR lowering 残留路径与 source/sysroot 解析入口，随后删除后端 fallback 并加入 sysroot f-string lint 与 owner 测试。
- 已删除 LLVM/HIR direct f-string codegen 函数、MIR f-string codegen 函数、MIR lowering 的 `lower_interpolated_string_expr` 入口，以及 f-string-only runtime ABI declarations；保留 HIR 层 `StringBuilder` desugar，但重命名为 `desugar_f_string_expr` 以避免旧后门命名残留。
- 已在 parser 阶段根据 `SourceFile::is_sysroot()` 拒绝 sysroot f-string，并添加 `sysroot_files_cannot_contain_fstring` owner 测试。
- 下一步运行格式化、P6-T02 指定 grep、owner 测试和 fixture 验证。
- 已完成验证：指定旧后门命名 grep 无命中；sysroot f-string owner 测试通过；`cargo test -p scoopc fstring_desugar -- --nocapture` 通过；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/fstring_*.scoop` 通过；`cargo test --all --all-targets` 通过；`cargo clippy --all-targets -- -D warnings` 通过。
- 全量 `cargo run -p scoop -- test` 在最终代码状态下仍为既有 7 个失败、1335 个通过、1372 checks 通过；失败项为 `mutable_array_ops_basic`、5 个 runtime GC/native-root stdout mismatch、`run_pass_cone/cross_file_ctor_named_default_basic`，均未涉及 f-string owner path。
- 已更新 `TODO.md` / `TODO-3.md` 的 P6-T02 状态与完成记录；下一步提交本任务改动。
