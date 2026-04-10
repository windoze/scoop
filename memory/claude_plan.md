# T0123: `const fun` 支持 String `+` 和 substring 类操作

## 思路

当前 comptime 求值器（eval.rs）已支持：
- String 字面量 → ConstValue::String
- String `==`/`!=` 比较（line 1143）
- `trimIndent()` 方法调用（硬编码，line 529）

当前 comptime 解释器（interpreter.rs）已支持：
- String 类型的 `const val` 绑定和传参（ConstValue::String 已存在）
- `const fun` 调用（仅限同文件内无 receiver 的函数）

**关键约束**：sysroot/string.scoop 中的 extension functions 使用了 `@Unsafe`、`unsafeSliceBytes`、while 循环、var 等解释器不支持的构造。因此**不能**通过解释器直接执行 sysroot 函数。

**方案**：将常用 String 方法实现为 eval.rs 中的内建 intrinsics（与 trimIndent 同一模式），直接在 Rust 侧完成编译期计算。

## 实现步骤

### Step 1: String `+` 拼接（eval.rs）
- `eval_binary_eager` 中为 `Add` 操作添加 `(String, String)` 分支

### Step 2: String 方法 intrinsics（eval.rs）
在 Call handler（MemberAccess callee）中添加分支，实现常用 String 方法。

### Step 3: 新增 comptime fixtures
- `const_fun_string_ops_basic.scoop` + `.comptime`
- `const_fun_string_methods.scoop` + `.comptime`

### Step 4: 验收
- `cargo test --all` + `cargo clippy --all-targets -- -D warnings`

## 当前进度
- [ ] Step 1: String `+`
- [ ] Step 2: String 方法 intrinsics
- [ ] Step 3: Fixtures
- [ ] Step 4: 验收
