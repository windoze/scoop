# 编译器限制审计（2026-04-11）

## 范围与方法

本次审计覆盖以下目录：

- `crates/scoopc/src/llvm/`
- `crates/scoopc/src/hir/`
- `crates/scoopc/src/resolve/`
- `crates/scoopc/src/typecheck/`

搜索信号：

- `LlvmEmitError::UnsupportedMainBody`
- `ExprKind::Todo(...)` / `StmtKind::Todo(...)`
- `ExprTypeError::UnsupportedExpr` / `UnsupportedMemberAccess` / `UnsupportedTypeRef`
- `todo!` / `unimplemented!`
- `HACK` / `FIXME`
- 与降级路径相关的 `Any` fallback、receiver/type param 特判、旧 TODO 注释

原始结果摘要：

- `UnsupportedMainBody`：1325 个原始匹配
- `UnsupportedExpr`：77 个原始匹配
- HIR `Todo(...)`：13 个原始匹配
- `todo!` / `unimplemented!`：0
- `HACK` / `FIXME`：0

说明：

- 这些数字是“原始信号量”，不是“独立功能缺口数”。
- 绝大多数 `UnsupportedMainBody` 属于后端防御性守卫，例如 side table 缺失、HIR 不一致、LLVM builder 状态异常、非法 span/布局假设等；需要按语义分组，而不是按命中次数判断优先级。

## 需要新增短 TODO 的用户可见缺口

### 1. `for (x in iterable)` 的 Custom iterator 路径停在 HIR `Todo`

- 位置：
  - `crates/scoopc/src/typecheck/expr/stmt.rs:706-820`
  - `crates/scoopc/src/ast/mod.rs:1749`
  - `crates/scoopc/src/hir/lower/stmt.rs:86-113`
- 现状：
  - typecheck 已支持通用迭代协议：`xs.iterator(): Iter`、`Iter.next(): Option<Elem>`。
  - 当 iterable 不是 `Array<Int>` / `IntProgression` 时，typecheck 会把 `resolved_for_info.kind` 写成 `Custom`。
  - 但 HIR lowering 对 `Custom` 仍直接产出 `StmtKind::Todo("for_custom_iterator")`，导致正常编译路径卡在 lowering。
- 影响：
  - 用户自定义 iterable 无法进入 LLVM。
  - `for` 语法目前只有两条硬编码快路径，扩展性差。
- 处理：
  - 已新建 `T0151`。
- 优先级：
  - P0。前端已经完成协议检查，剩余缺口集中且可回归。

### 2. safe member access 只覆盖 `Option<struct>.field`

- 位置：
  - `crates/scoopc/src/typecheck/expr/member.rs:14-75`
- 现状：
  - `receiver?.field` 当前只接受 `Option<Struct>` 且字段必须是 struct field。
  - `Option<Class>` / `Option<Object>` 上的字段访问，以及 safe extension property 都会落到 `UnsupportedExpr` / `UnsupportedMemberAccess`。
  - `receiver?.method(...)` 走的是另一条 member-call 路径，不在本缺口范围内。
- 影响：
  - Nullable 访问在“字段/属性”维度与普通 member access 语义不对齐。
  - 直接阻塞 Kotlin 风格的 `obj?.field` / `obj?.prop` 惯用写法。
- 处理：
  - 已新建 `T0152`。
- 优先级：
  - P1。功能面明显，但边界清晰。

### 3. receiver function type 可以声明，但不能作为局部函数值调用

- 位置：
  - `crates/scoopc/src/typecheck/expr/call.rs:789-823`
  - `crates/scoopc/src/llvm/codegen/mod.rs:10068-10084`
- 现状：
  - 类型系统已支持 receiver function type（见历史 T0435 / T1213）。
  - 但局部函数值调用在 typecheck 阶段会拒绝 `fun.receiver.is_some()`，报“函数值调用（暂不支持 receiver function type）”。
  - LLVM 间接 closure call 也显式拒绝 `receiver function value call`。
- 影响：
  - `val f: String.(Int) -> Int = ...` 这类值可以存在，却不能通过 `f(receiver, arg)` 形式调用。
  - scope functions 的声明面已建立，但“把 receiver lambda 当普通值存储/转发/再调用”的 higher-order 场景不完整。
- 处理：
  - 已新建 `T0153`。
- 优先级：
  - P1。Higher-order 能力缺一块显式可见拼图。

### 4. higher-order 间接调用仍不支持 aggregate 返回值

- 位置：
  - `crates/scoopc/src/llvm/codegen/mod.rs:552-559`
  - `crates/scoopc/src/llvm/codegen/mod.rs:9984-10058`
  - `crates/scoopc/src/llvm/codegen/mod.rs:10141-10212`
- 现状：
  - `FunPtr` 间接调用与 closure/function value 间接调用都显式拒绝 `Tuple/Struct/Enum` 返回值。
  - 代码中已有注释说明“正确修复应转为 sret”。
- 影响：
  - higher-order API 只能稳定返回标量 / `Ref` / `String`，复合值返回仍需要源码 workaround。
  - 这类限制会继续向 stdlib API 形状泄漏。
- 处理：
  - 已新建 `T0154`。
- 优先级：
  - P1。属于明确的 LLVM codegen 缺口，且已有实现注释给出方向。

## 已分类但不新增短 TODO 的信号

### A. 已有任务覆盖 / 已完成但保留 fallback

- `RangeInclusive` / `Elvis`：
  - 位置：`crates/scoopc/src/llvm/codegen/mod.rs:13805-13810`
  - 分类：
    - `Elvis` 已由 `T0108` 通过 lowering 展开解决；LLVM 直接分支属于“不应再命中”的守卫。
    - `RangeInclusive` 已有当前短 TODO `T1819` 覆盖，不重复建任务。

- `with_update`：
  - 位置：`crates/scoopc/src/hir/lower/expr.rs:1629-1657`
  - 分类：
    - `T0109` 已完成；这里保留的是 dump-hir / 未经过 typecheck 时的 fallback，不是正常编译路径缺口。

- `array_lit`：
  - 位置：`crates/scoopc/src/hir/lower/expr.rs:80-103`
  - 分类：
    - `T0149` 已完成；这里只在“缺少 lowering hint 且元素类型也无法从局部信息恢复”时退回 `Todo`，本质是未跑 typecheck 的保守兜底。

- interface dispatch：
  - 位置：
    - 旧注释：`crates/scoopc/src/typecheck/expr/call.rs:4373-4377`
    - 实现：`crates/scoopc/src/llvm/codegen/mod.rs:9444-9944`
  - 分类：
    - 功能已由历史 `T1508c` 落地。
    - 本次顺手修正了过期注释，避免误导后续审计。

### B. 刻意保留的阶段性设计

- `class literal`
  - 位置：
    - `crates/scoopc/src/hir/lower/expr.rs:107`
    - `crates/scoopc/src/typecheck/annotations.rs:1151-1162`
  - 分类：
    - 当前仅作为注解 / comptime 可用的“类型名常量”存在。
    - 普通表达式路径未接线，但这是阶段性语义边界，不在本轮新增短 TODO。

- `FunPtr<F>` 的 receiver signature
  - 位置：
    - `crates/scoopc/src/typecheck/lower.rs:146-150`
    - 历史设计记录：`TODO-1.md:5354`
  - 分类：
    - 当前设计明确要求 `FunPtr<F>` 的 `F` 为 non-receiver function type。
    - 因此不把它列为“缺失功能”，只在 `T0153` 中覆盖普通函数值调用。

- function value / `FunPtr` 的命名实参
  - 位置：
    - `crates/scoopc/src/typecheck/expr/call.rs:798-801`
    - `crates/scoopc/src/typecheck/expr/call.rs:935-938`
  - 分类：
    - 函数类型本身不承载稳定形参名；当前拒绝 named arg 属于有意约束，而非遗漏。

### C. 内部守卫 / 非正常编译路径

- HIR `Todo(...)` 13 处中，以下属于“只在非法语境或未跑前置阶段时出现”的内部守卫：
  - `missing_stmt`
  - `spread_arg`
  - `named_arg`
  - `assign`
  - `with_update`（见上）
  - `array_lit`（见上）

- `ExprKind::ClassLit` / `ComptimeBlock` / `ComptimeIf` / `ComptimeFor`
  - 属于阶段性边界或单独主题，不纳入本次 codegen 限制短 TODO。

- 大量 `UnsupportedMainBody` / `UnsupportedExpr`
  - 例如：
    - `builder has no insert block`
    - `missing declared function`
    - `itable call slot ambiguous`
    - `when subject type`
    - `cross-file signature lowering（missing decl source）`
  - 分类：
    - 这类信号用于防御 HIR / side table / lowering 不一致，或用于把“理论上不应发生”的状态转成稳定诊断。
    - 它们不是可单独排期的用户功能。

## Runtime/C 残留依赖结论

本次补看了“仍由 runtime/C 实现但理论上可迁移”的剩余接口形态。结论：

- 已在 `T0122` 中完成大规模迁移的 String 子串/搜索/split 系列，不再是主战场。
- 目前剩余 runtime/C 依赖主要分三类：
  - host/OS 桥接：`env/fs/io/path/process/thread/time/net/channels/sync`
  - GC / unsafe / ABI 边界：`GC.*`、`FunPtr`、thread/task/channel backend
  - 标量格式化或性能敏感 helper：`*_to_string`、`String.hash` 等
- 这些依赖中没有出现“像旧版 String API 那样明显且高收益的整组迁移对象”，因此本轮不新增 runtime 迁移短 TODO。

## 本轮产出

- 新增短 TODO：
  - `T0151`：Custom iterator `for` lowering + codegen
  - `T0152`：safe member access parity（ref receiver / extension property）
  - `T0153`：receiver function value invocation
  - `T0154`：higher-order aggregate returns（closure / function value / `FunPtr`）
- 修正 1 处过期注释：
  - `crates/scoopc/src/typecheck/expr/call.rs`
