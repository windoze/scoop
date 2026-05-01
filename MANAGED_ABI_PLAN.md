# Scoop：Managed ABI 与 native callable ABI 收口计划

> 生成时间：2026-04-30  
> 历史归档：暂无（本轮首次建档）  
> 本轮主题：按 [`MANAGED_ABI.md`](./MANAGED_ABI.md) 的设计基线，把 external callable surface 从“按入口和历史特判拼接 lowering”收口为“按 ABI identity 分流 lowering”；同时落地 `ExternAbi::Scoop`，并把 native `@Extern` / `FunPtr` 统一到单一 native ABI classifier。

## 0. 工作原则

- 本轮严格按 `MANAGED_ABI_TODO.md` 的顺序推进，不跨条目并行实现。
- [`MANAGED_ABI.md`](./MANAGED_ABI.md) 是本轮唯一设计基线；若实现过程中改变主张，必须先回写该文档，再继续实现。
- 本轮不是为某个单独 fixture 打补丁，而是要把 callable ABI identity 变成一等 contract。
- `FunctionType` 只表达源码级签名，不再默认承担 ABI 身份。
- direct `@Extern` 与 indirect native `FunPtr` 必须共享同一套 native ABI classifier。
- `GC-free` 不等于 `C ABI-safe`。
  - native surface 的门禁必须以 ABI-safe 规则为准；
  - tuple、普通 nominal value type、enum、以及依赖 hidden sret 的 aggregate，不能仅因 GC-free 就自动穿过 C ABI。
- 禁止“参数按 native lowering、返回值按 ordinary lowering”这类分裂实现。
- 禁止 direct symbol call 与 indirect funptr call 对同一 native callable 使用不同的 aggregate return 规则。
- 优先建立 ABI 边界和验证矩阵，再迁 helper。
  - `ExternAbi::Scoop` 的落地是本轮主线之一；
  - 但在引入新 ABI 前，必须先把 native surface 的 contract 收紧为自洽状态。
- correctness-first，兼容 second。
  - 若现有 fixture / helper 建立在错误 ABI 假设上，应修 contract 与测试，而不是保留错误模型。
- 跨平台是本轮验收的一部分。
  - 至少要求 `linux/amd64` 与 `macos/aarch64` 对 native surface 的 direct / indirect parity 有明确验证。

## 1. 顺序总览

1. 先把当前 external callable surface 的现状盘清，形成“签名、ABI 身份、参数 lowering、返回值 lowering、callconv、native/managed 边界”的统一 baseline。
2. 再把 ABI identity 引入前端/HIR/LLVM lowering 设计，让编译器能区分 ordinary callable、native callable 与 managed external callable。
3. 随后收紧 `ExternAbi::C` 的 typecheck 合同，从“GC-free”升级为“ABI-safe native value surface”。
4. 在门禁和表示稳定后，统一 direct `@Extern` 与 native `FunPtr` 的 declaration/call lowering，建立单一 native ABI classifier。
5. 然后清理现有建立在错误 native aggregate-return 假设上的测试与 helper，补齐 direct/indirect parity 回归。
6. native surface 收口后，再引入 `ExternAbi::Scoop` 的声明与调用主线，让 external linkage 下的 ordinary managed helper 有正式出口。
7. 最后以 `string cone` 为试点，验证“编译器已按 ABI identity，而不是按 helper 名字工作”。

## 2. 分阶段目标

### P0. 基线收口与 native surface 审计固化

- 盘清当前 external callable surface 的所有主路径：
  - direct `@Extern` declaration / call；
  - `FunPtr<F>` direct call / `.invoke(...)`；
  - `funPtrToUIntPtr` / `uintPtrToFunPtr` token round-trip；
  - aggregate param / return、receiver、calling convention、`gc-leaf-function`、`enter_native/leave_native` 等决策点。
- 目标不是再写一版泛泛设计，而是形成可执行 baseline：
  - ABI 身份 today 丢失在哪里；
  - 哪些规则仅在 direct extern 路径存在；
  - 哪些规则在 `FunPtr` 路径被局部重建；
  - 哪些现有 fixtures / helper 建立在错误 ABI 假设上。

### P1. ABI identity 模型落地

- 在设计与实现表示上明确切开两层信息：
  - 源码级函数签名；
  - callable ABI identity。
- 至少能稳定区分：
  - ordinary managed callable；
  - native C ABI callable；
  - managed external callable（`ExternAbi::Scoop`）。
- 这一阶段不要求立刻全量改 codegen，但要求后续任务不再依赖“只看 `FunctionType` 推 ABI”。

### P2. native surface typecheck 合同收紧

- 把 `ExternAbi::C` 的 surface 从“GC-free 即可”收紧为“ABI-safe native value surface”。
- 明确 v1 允许集与禁止集：
  - 允许：标量、`UIntPtr`、`Ptr<T>`、`@CLayout` 且已证明 ABI-safe 的值类型；
  - 禁止：tuple、普通 nominal value type、enum，以及需要 hidden sret 的 aggregate。
- `FunPtr<F>` 也要纳入同一规则。
  - 若 `F` 表示 native callable，参数/返回值必须满足 native ABI-safe 约束；
  - 不允许通过 `FunPtr` 侧门绕过 `@Extern` 的 ABI-safe 限制。

### P3. 单一 native ABI classifier

- 在 declaration path 与 callsite path 建立统一的 native ABI classifier。
- 这个 classifier 必须作为一个整体决定：
  - 参数 lowering；
  - receiver lowering；
  - 返回值 lowering；
  - aggregate return 是 direct 还是 indirect；
  - calling convention；
  - `enter_native/leave_native`；
  - `gc-leaf-function`。
- 一旦 classifier 成立，direct `@Extern` 与 native `FunPtr` 只能复用它，不能各自另写局部规则。

### P4. native surface 测试与 helper 收口

- 清理当前用 ordinary hidden sret 冒充 native aggregate return 的测试/辅助 helper。
- 把 native surface 的回归改成按 ABI family 验证：
  - direct symbol call；
  - indirect funptr call；
  - token round-trip；
  - 多平台 parity。
- 若某些既有 fixture 与 v1 `ExternAbi::C` contract 冲突，应明确迁移为：
  - `@CLayout` / `Ptr<T>` / `UIntPtr` 形式；
  - 或转为未来 `ExternAbi::Scoop` / managed external surface 覆盖。

### P5. `ExternAbi::Scoop` 声明路径

- 扩展前端/HIR side table 与 symbol declaration path，支持 `ExternAbi::Scoop`。
- 要求：
  - external linkage；
  - ordinary param ABI；
  - ordinary return / hidden sret；
  - 无 `enter_native/leave_native`；
  - 不标记 `gc-leaf-function`。
- 初版范围继续保持收窄：
  - 只支持顶层函数；
  - 只支持 `Pure`；
  - 暂不支持 generics / closure crossing / outward suspend。

### P6. `ExternAbi::Scoop` 调用路径与 `string cone` 试点

- 把 imported `ExternAbi::Scoop` 调用接到 ordinary managed call 主线。
- 以 `string cone` 为 tracer bullet：
  - 先接 `Int.toString`；
  - 再接 `Bool/Char/Float.toString`；
  - 再接 `String.concat/replace/repeat/...`。
- 验证编译器已经按 ABI 分流工作：
  - 不再依赖 helper FQN special-case；
  - 调用点仍保留 statepoint / root spill / hidden sret correctness。

### P7. 稳定化与后续扩展入口

- 全量回归要覆盖：
  - `cargo test --all`；
  - `cargo run -p scoop -- test`；
  - native surface direct/indirect parity；
  - `linux/amd64` 与 `macos/aarch64` 平台矩阵；
  - `ExternAbi::Scoop` 的 IR 与语义回归。
- 在 v1 稳定后，再评估：
  - managed external function pointer；
  - 泛型导入/导出；
  - outward effect / continuation ABI；
  - ABI version / feature bitmap。

## 3. 本轮关键判断

- 本轮的核心不是“给 `@Extern` 多一个枚举值”，而是把 callable ABI identity 正式建模。
- `ExternAbi::Scoop` 与 native ABI 清理必须一起设计，但实现顺序应先 native、自洽后 managed。
- `FunPtr` 不是 ABI 逃生舱。
  - 它承载的不是“随便一个地址 + 随便一个 `F`”；
  - 而是有 ABI family 的 callable token。
- native surface 的 v1 重点是“正确、可审计、跨平台一致”，不是“尽量多放行 aggregate”。
- 若现有 fixture 与 v1 ABI contract 冲突，优先修 contract 与测试，不为了保留旧 fixture 继续扩大不自洽的 native surface。

## 4. 主要风险与应对

### 4.1 把 ABI identity 只做成文档名词，没有真正进入 lowering 决策

- 若 declaration / callsite / typecheck 仍各自局部推 ABI，本轮会再次退化成名字驱动特判。
- 应对：要求单一 classifier 成为验收项，而不是实现细节。

### 4.2 native ABI-safe 约束过松，继续把错误 aggregate surface 放进 v1

- 若继续默认放行 tuple / enum / 普通 nominal value type，x86_64 上“看起来能跑”的错误仍会被保留。
- 应对：先收紧为可审计子集，再讨论扩展支持。

### 4.3 `FunPtr` round-trip 丢失 ABI 元信息

- 仅靠 `UIntPtr <-> FunPtr` 传地址，无法在 callsite 末端恢复完整 ABI contract。
- 应对：内部 lowering contract 必须保留 ABI family，不能把“值是地址”误当成“ABI 已知”。

### 4.4 `ExternAbi::Scoop` 过早扩面

- 若在 native surface 仍分裂时就引入 managed external callable，容易把两条线一起做坏。
- 应对：先 native classifier，再 `ExternAbi::Scoop` declaration/call path。

### 4.5 跨平台验证被降级为“事后补测”

- 这条线本来就是被 `macos/aarch64` 暴露出来的；如果不把跨平台矩阵当成本轮验收，问题会再次回流。
- 应对：把 `linux/amd64` 与 `macos/aarch64` parity 明确写进 TODO 验收。

## 5. 预期收口状态

- callable ABI identity 已成为一等信息，不再只剩源码级 `FunctionType`。
- `ExternAbi::C` 的 surface 已收紧为 ABI-safe native value surface，而不是 GC-free 的宽泛代理。
- direct `@Extern` 与 native `FunPtr` 已共享同一套 declaration / call classifier。
- native surface 的 direct/indirect parity 已在 `linux/amd64` 与 `macos/aarch64` 上被回归锁定。
- `ExternAbi::Scoop` 已为 external linkage 下的 ordinary managed helper 提供正式出口。
- `string cone` 已成为首个不依赖 helper 名字 special-case 的 managed external 试点。
