# 本轮执行计划

## 约束说明

根据任务要求，我会先记录高层执行计划、关键判断点和进度更新；这里不会写逐字内部思维，而是写可审计的执行步骤、决策依据和状态变化。

## 初始步骤

1. 检查最新一次 Git 提交，确认提交信息或相关变更里是否提到已有问题。
2. 如果发现“需先修复的既有问题”，优先修复并验证。
3. 读取 `TODO.md`，定位第一个未完成任务。
4. 读取 `PLAN.md`，确认当前计划与该任务是否一致。
5. 判断该任务是否足够小且可在本轮完整完成。
6. 如果任务过大，则把任务拆成更小子任务，并同步更新 `PLAN.md` 与 `TODO.md`，本轮只执行拆分后的第一个子任务。
7. 实现本轮目标任务。
8. 运行相关测试、格式化、lint，至少覆盖：
   - 相关最小测试集
   - `cargo fmt --check`
   - `cargo clippy --all-targets -- -D warnings`
   - 必要时运行更大范围测试
9. 更新文档与计划：
   - 在 `TODO.md` 标记本轮任务完成
   - 在 `PLAN.md` 记录状态变化
   - 按需要继续更新本文件
10. 提交 Git commit，然后停止。

## 预设决策规则

- 不会跳过“最新提交中提到的既有问题”检查。
- 不会同时推进多个 TODO 任务；只完成当前第一项未完成任务。
- 如果遇到阻塞，会保持任务为 TODO，并按依赖顺序重排 `TODO.md` 与更新 `PLAN.md`，然后提交并停止。
- 任何代码编辑前，会先补充本文件中的执行进度说明。

## 当前状态

- 状态：已完成仓库检查，正在拆分 T0148。

## 已完成检查

1. 已查看最新提交 `08be83bced3efe656daf1f5c7c82dd14d70e255f`，提交信息仅为 `[T0147c] 补齐 Float sysroot API 与 builtin 路由`，未在提交说明中声明需先修复的遗留问题。
2. 已读取 `TODO.md` 与 `PLAN.md`。
3. 已确认当前第一个未完成任务为 `T0148`（Float 字面量完整管线）。

## 关键判断

- `T0148` 同时跨越 lexer / parser / AST / HIR / typecheck / LLVM codegen / comptime / fixtures，单轮完整落地的改动面过大，且回归路径横跨多个阶段，不适合作为一次提交直接完成。
- 因此需要先拆为更小子任务，并在本轮只执行第一个子任务。

## 拆分方案（拟定）

1. `T0148a`：Float 字面量前端打通（lexer / token / AST / parser / HIR lowering / parse+hIR fixtures）。
2. `T0148b`：Float 字面量静态语义（默认类型、Float32 后缀、absorption、基础类型推断）。
3. `T0148c`：Float 算术/比较/转换的 LLVM codegen 与 run-pass fixtures。
4. `T0148d`：comptime、多文件与剩余字面量审计收尾。

## 本轮执行目标

- 更新 `TODO.md` / `PLAN.md`，将 `T0148` 替换为可独立验收的子任务。
- 实现并完成 `T0148a`。
- 运行针对性测试与全量回归。
- 更新文档状态并提交 commit。

## 已完成实现

1. 已把 `T0148` 在 `TODO.md` / `PLAN.md` 中拆分为 `T0148a ~ T0148d`。
2. 已完成 `T0148a`：
   - `TokenKind::FloatLiteral`
   - `syntax/float_literal.rs`
   - lexer 数字扫描扩展（小数 / 科学计数法 / `f` / `f32` 后缀）
   - AST `ExprKind::FloatLit`
   - parser Float literal 解析与 `1.toString()` / `1..2` 保护
   - HIR `LiteralKind::Float64(f64)` / `LiteralKind::Float32(f32)`
   - HIR lowering 到 builtin `Float64` / `Float32`
3. 为保持全仓编译通过，已同步补齐少量辅助路径：
   - resolver/property walker 对 `FloatLit` 的无副作用遍历
   - MIR `ConstValue` 新增 Float kind
   - LLVM `codegen_literal` 新增 Float literal 常量发射

## 已完成验证

- 定向单测：
  - `cargo test -p scoopc --lib float -- --nocapture`
- 全量验证：
  - `cargo fmt --check`
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - `cargo clippy --workspace --all-targets --message-format short -- -D warnings`

## 当前状态

- 状态：`T0148a` 实现与验证完成，正在回写计划与待办状态，然后提交。
