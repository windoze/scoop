# M5 设计：数组

版本：0.1（草案）

对应 `docs/ROADMAP.md` 的 M5。目标：`Array<T>` / `MutableArray<T>`、数组字面量与推导规则、下标访问、`size`、两种数组的互转（spec 第 10 章）。

## 0. 范围说明

- 数组在 spec 中是 `class`（引用类型），但 class/方法体系属 M7。与 M3 的 Option 一样，M5 的两种数组是**编译器内建类型**（core 库声明等 class 落地后补齐，见 5.1）；类型标注 `Array<Int>` / `MutableArray<Int>` 直接用 M4 已有的 `Name<T>` 语法。
- spec 10.4 的互转有"以对方为参数的构造函数"与 `toArray`/`toMutableArray` 方法两种形式；M5 只实现**构造函数形式**（`Array(m)` / `MutableArray(a)`，方法形式需要 M7 的方法调用）。
- spec 10.5 的 `Iterable<T>` / `for` 依赖接口与方法，不在 M5；迭代用 `while` + 下标 + `size`（`for` 与区间的归属在 M12 前的某个里程碑单独立项，见第 6 章）。
- 越界检查：M5 为 trap（沿用 `!!` 的 `scoop_rt_trap` 机制），M8 改接异常（见 5.2）。

## 1. 语言子集

```
fun main() {
    val a = [1, 2, 3]                       // Array<Int>
    val m: MutableArray<Int> = [4, 5]       // 上下文推导为 MutableArray
    println(a[0] + a.size)                  // 1 + 3
    m[0] = 40                               // MutableArray 下标赋值
    println(m[0])
    val b = Array(m)                        // 互转（memcpy，独立快照）
    m[0] = 99
    println(b[0])                           // 仍是 40
    val nested = [[1, 2], [3]]              // Array<Array<Int>>
    println(nested[0][1])                   // 2
    val ps = [Point(1, 2), Point(3, 4)]     // 值类型元素：内联连续布局
    println(ps[1].x)                        // 4
}
```

新增（相对 M4）：

- **类型**：`Array<T>` / `MutableArray<T>`（内建泛型类型；对 `T` 不变，spec 10.4）。
- **字面量** `[e1, e2, ...]`：上下文推导种类（`MutableArray<T>` 标注 → 可变；其余一律 `Array`）；空字面量 `[]` 必须有显式类型上下文。
- **下标**：读 `a[i]`（两种数组都可，返回元素类型 `T`）；写 `m[i] = v`（仅 `MutableArray`，值类型必须匹配）；`a.size`（伪属性，返回 `Int`）。
- **互转**：`Array(m)` / `MutableArray(a)`（实参必须是另一种类、元素类型相同；语义为 memcpy 独立快照，spec 10.4）。
- **越界**：每次下标读写做运行时检查（`i` 视作无符号与 `size` 比较），失败 trap。

子集外诊断：`for`、区间 `a..b`、切片、`String` 下标 → "not supported yet (milestone M5)"。

## 2. 各 stage 设计

### 2.1 AST / parser

- 字面量 `[e1, e2, ...]`（`Expr::ArrayLiteral`）；后缀下标 `expr[expr]`（优先级与 `.` 同级）；赋值目标扩展：`m[i] = v`（`Assign` 的目标从标识符扩展为 `AssignTarget = Local(Ident) | Index { receiver: Expr, index: Expr }`）。
- 语句位表达式后跟 `[` 时的解析注意与字面量区分：`a[i]` 是下标（postfix 紧挨）、行首 `[` 是字面量。

### 2.2 HIR

- `Type` 新增 `Array(TypeId)` / `MutableArray(TypeId)`（types_equal 按元素类型严格相等——不变性）；`TypeRefKind::Generic` 的 `Array`/`MutableArray` 解析为内建。
- **字面量推导**（spec 10.2/10.3 的 M5 形态）：
  - 有期望类型（标注/实参位）：期望为 `Array<U>` 或 `MutableArray<U>` 时逐元素检查类型 = `U`；`[]` 仅此时合法；
  - 无期望类型：M5 简化——**所有元素类型必须完全相同**（值类型规则与 spec 10.3 一致；spec 对引用类型的 LOB 规则暂缓——M5 只有 String 与数组自身两种引用类型，无混合场景；混合一律报 "array literal elements must have the same type, found X and Y"），推导为 `Array<T>`；
  - spec 10.3 的"值类型元素不 auto-box"在 M5 自然成立（没有 auto-box 场景）。
- 下标读：接收者必须 `Array<T>`/`MutableArray<T>`，下标必须 `Int`，结果 `T`；下标写：仅 `MutableArray<T>`、值类型 `T`；`.size` 只对数组类型解析（其他类型的 `.size` 照旧 unknown field）。
- 互转构造：callee 名 `Array`/`MutableArray` 优先按转换内建解析（实参恰好 1 个、类型为另一种类且元素类型相同），否则按未知函数诊断。

### 2.3 MIR

- `Type::Array(Box<Type>)` / `MutableArray`；`Expr::ArrayLiteral { mutable, elements }`、`ArrayGet { array, index }`、`ArrayLen(operand)`；语句 `ArraySet { array, index, value }`；`ArrayClone(operand)`（互转）。
- mangling 类型编码：`A<元素>` / `M<元素>`。
- 数组字面量元素按出现顺序求值（无脱糖）。

### 2.4 LIR

- `LirType::Array(Box<LirType>)`（元素内联的变长对象；值本身为 `Ptr`）；
- 指令：`ArrayAlloc { out, element_ty, elements: Vec<Value> }`（计算总大小 = 头(8) + size 字段(8) + n × 元素布局大小，调 `scoop_rt_alloc` 带数组 TypeDescriptor——见 2.5，写 size 与逐元素 store）、`ArrayLen { out, operand }`（load size 字段）、`ArrayGet { out, array, index, element_ty }`（**越界检查 + trap**，然后按元素布局 load）、`ArraySet { array, index, value }`（同检查）；
- `ArrayClone` → runtime 调用 `scoop_rt_array_clone(obj, elem_size)`（见 3.1）；
- **布局 meta**：每个数组实例一条：元素布局 + `element_is_ref`（M9 GC 扫描用：元素为引用时整个元素区按引用扫描）；tagged enum 元素暂按 M4 已记录的边界处理。

### 2.5 codegen

- 数组对象内存：`{ ptr td, i64 size, [n x elem] }`（td 指向 codegen 发射的数组 TypeDescriptor `scoop_td_array_<enc>`，结构沿用 runtime spec 2.2，`size` 字段记元素大小）；
- 越界检查：`(u64)index >= (u64)size` → 复用共享 trap 块（消息 "array index out of bounds"）；负下标自然被无符号比较覆盖；
- 元素为聚合/引用时的 load/store 按 LIR meta 布局。

## 3. runtime 新增

### 3.1 `scoop_rt_array_clone`

`const void *scoop_rt_array_clone(const void *obj, uint64_t elem_size)`：从对象头后读 `size`，计算总字节数，`scoop_rt_alloc` 风格分配（malloc，always-leak）+ 整体 memcpy，返回新对象。元素为引用时**浅拷贝**（spec 10.4 的"独立快照"指数组本身，元素对象不递归复制——与 memcpy 语义一致）。

## 4. 测试计划

- **独立 fixture**（`tests/fixtures/m5-arrays/`）：字面量推导（Array 默认、MutableArray 标注）；下标读写；`size`；互转独立性（改原数组不影响快照）；空字面量带标注；
- **组合 fixture**：嵌套数组；struct/tuple/enum 元素（值类型内联布局 + 聚合 load/store）；`Option<Int>` 元素；数组作为函数参数/返回值；大下标与小下标混合循环（while + size 迭代）；
- **trap fixture**：`EXPECT-TRAP` 越界读、越界写、负下标；
- **negative fixture**：`[]` 无上下文；元素类型混合；下标非 Int；对 `Array` 下标写；对非数组下标；互转元素类型不符；`MutableArray` 与 `Array` 类型不兼容（不变性）。

## 5. 临时决策（及退役里程碑）

1. **数组是编译器内建类型**（同 M3 Option 的先例）：class/方法落地（M7）后在 core 库补齐声明，内建识别点收敛在 HIR 一处；布局与语义不变。
2. **越界 trap**：M8 由异常取代（spec 11.7 的异常族；下标越界的异常类型随 M8 定）。
3. **引用类型元素必须同类型**（无 LOB 推导）：spec 10.3 的 LOB 规则在出现混合引用类型场景（M7 之后）时实现。
4. **互转只支持构造函数形式**：`toArray`/`toMutableArray` 方法随 M7 补。

## 6. 明确不做

- `for` 循环与区间（`IntRange` 等，spec 11.8）——需要接口/方法支撑，与 `for` 的归属一起在后续里程碑单独立项；
- `String` 下标/切片；切片；数组的 `==` 比较（spec 未定义数组相等语义，待 spec 补充）；元素为 tagged enum 时 Plain 布局的引用偏移压平（M4 已记录的边界）。
