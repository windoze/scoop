# M5 设计：数组

版本：0.2（实现同步）

对应 `docs/ROADMAP.md` 的 M5。目标：`Array<T>` / `MutableArray<T>`、数组字面量与推导规则、下标访问、`size`、两种数组的互转（spec 第 10 章）。

## 0. 范围说明

- 数组在 spec 中是 `class`（引用类型），但 class/方法体系在 M6 落地。当前两种数组仍是**编译器内建类型**（core 库声明待补齐，见 5.1）；类型标注 `Array<Int>` / `MutableArray<Int>` 直接用 M4 已有的 `Name<T>` 语法。
- spec 10.4 的两组显式互转均已实现：构造函数形式 `Array(m)` / `MutableArray(a)`，以及 `m.toArray()` / `a.toMutableArray()` 方法形式；四者都降为同一个 `ArrayClone`，执行独立的 memcpy 快照。
- spec 10.5 的 `Iterable<T>` / `for` 依赖接口与方法，不在 M5；迭代用 `while` + 下标 + `size`（`for` 与区间待后续里程碑单独立项，见第 6 章）。
- 越界检查：M5 初版为 trap；M8 已改接 `IndexOutOfBoundsException`。

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
- **越界**：每次下标读写做运行时检查（`i` 视作无符号与 `size` 比较）；M8 起失败抛 `IndexOutOfBoundsException`。

子集外诊断：`for`、区间 `a..b`、切片、`String` 下标 → "not supported yet (milestone M5)"。

## 2. 各 stage 设计

### 2.1 AST / parser

- 字面量 `[e1, e2, ...]`（`Expr::ArrayLiteral`）；后缀下标 `expr[expr]`（优先级与 `.` 同级）；赋值目标扩展：`m[i] = v`（`Assign` 的目标从标识符扩展为 `AssignTarget = Local(Ident) | Index { receiver: Expr, index: Expr }`）。
- 语句位表达式后跟 `[` 时的解析注意与字面量区分：`a[i]` 是下标（postfix 紧挨）、行首 `[` 是字面量。

### 2.2 HIR

- `Type` 新增 `Array(TypeId)` / `MutableArray(TypeId)`（types_equal 按元素类型严格相等——不变性）；`TypeRefKind::Generic` 的 `Array`/`MutableArray` 解析为内建。
- **字面量推导**（spec 10.2/10.3）：
  - 有期望类型（标注/实参位）：期望为 `Array<U>` 或 `MutableArray<U>` 时逐元素检查为 `U` 的子类型；引用类型可零成本向上转型，值类型不得在元素位 auto-box；`[]` 仅此时合法；
  - 无期望类型：若包含值类型，所有元素类型必须完全相同；若全为引用类型，则枚举当前 HIR 可表达的共同父类型并选择唯一的最具体者作为 LOB。若有多个互不可比较的最具体共同父类型（HIR 没有交叉类型），退化为 `Any`；
  - 元素在 HIR 中统一重标为推导/期望的元素类型，引用向上转型不产生 `Box`。
- 下标读：接收者必须 `Array<T>`/`MutableArray<T>`，下标必须 `Int`，结果 `T`；下标写：仅 `MutableArray<T>`、值类型 `T`；`.size` 只对数组类型解析（其他类型的 `.size` 照旧 unknown field）。
- 互转构造：callee 名 `Array`/`MutableArray` 优先按转换内建解析（实参恰好 1 个、类型为另一种类且元素类型相同），否则按未知函数诊断。

### 2.3 MIR

- `Type::Array(Box<Type>)` / `MutableArray`；`Expr::ArrayLiteral { mutable, elements }`、`ArrayGet { array, index }`、`ArrayLen(operand)`；语句 `ArraySet { array, index, value }`；`ArrayClone(operand)`（互转）。
- mangling 类型编码：`A<元素>` / `M<元素>`。
- 数组字面量元素按出现顺序求值（无脱糖）。

### 2.4 LIR

- `LirType::Array(Box<LirType>)`（元素内联的变长对象；值本身为 `Ptr`）；
- 指令：`ArrayAlloc { out, elements, element_scan }`（计算总大小 = 16B 对象头 + 8B size + n × 元素布局大小，调 `scoop_rt_alloc` 并写 size 与逐元素 store；M12 加入 over-aligned 元素后，公式修订为 `alignUp(24, element_align) + n × element_size`）、`ArrayLen { out, operand }`（load size 字段）、`ArrayGet { out, array, index }`（**越界检查 + 异常**，然后按元素布局 load）、`ArraySet { array, index, value }`（同检查）；
- `ArrayClone` → runtime 调用 `scoop_rt_array_clone(obj, elem_size)`（见 3.1）；M12 为支持 over-aligned 元素扩展为 `(obj, elem_size, data_offset)`，M14再加入已定型的目标application TypeDescriptor，当前契约为`(obj, target_td, elem_size, data_offset)`；
- **布局 meta**：每个数组实例一条元素布局与完整`element_scan`。该递归扫描可以描述直接引用、含引用的struct / tuple，以及M13修订后使用固定ref偏移的tagged enum；codegen以元素stride包装为数组扫描节点，runtime对每个内联元素重复执行且不读取enum tag。

### 2.5 codegen

- 数组对象内存：`{ ptr td, i64 gc_word, i64 size, [n x elem] }`（td 指向 codegen 发射的数组 TypeDescriptor；TD 的 `size` 记元素 stride，递归扫描描述记 stride 与元素子扫描）；
- 越界检查：MIR 在访问前以 `(u64)index >= (u64)size` 判断并抛 `IndexOutOfBoundsException`；codegen 对裸 LIR `ArrayGet` / `ArraySet` 仍保留共享 trap 块作为结构性防线；负下标自然被无符号比较覆盖；
- 元素为聚合/引用时的 load/store 按 LIR meta 布局。

## 3. runtime 新增

### 3.1 `scoop_rt_array_clone`

M5初版为`const void *scoop_rt_array_clone(const void *obj, uint64_t elem_size)`：从对象头后读`size`，计算总字节数，经GC allocator分配并复制，返回新对象。元素为引用时**浅拷贝**（spec 10.4的“独立快照”指数组本身，元素对象不递归复制）。M12为over-aligned元素加入`data_offset`；M14数组改为普通generic intrinsic class application后，当前入口为`const void *scoop_rt_array_clone(const void *obj, const ScoopTypeDescriptor *target_td, uint64_t elem_size, uint64_t data_offset)`。它以目标descriptor分配并保留新对象头，只复制对象头之后的size/padding/elements，因而`Array<T>`与`MutableArray<T>`转换不会错误沿用来源descriptor。

## 4. 测试计划

- **独立 fixture**（`tests/fixtures/m5-arrays/`）：字面量推导（Array 默认、MutableArray 标注）；下标读写；`size`；互转独立性（改原数组不影响快照）；空字面量带标注；
- **组合 fixture**：嵌套数组；struct/tuple/enum 元素（值类型内联布局 + 聚合 load/store）；`Option<Int>` 元素；数组作为函数参数/返回值；大下标与小下标混合循环（while + size 迭代）；`recursive-reference-scans.scoop` 锁定“聚合元素 + tagged enum + 普通引用”的递归 GC 扫描；
- **未捕获异常 fixture**：`EXPECT-TRAP` 覆盖越界读、越界写、负下标；
- **negative fixture**：`[]` 无上下文；元素类型混合；下标非 Int；对 `Array` 下标写；对非数组下标；互转元素类型不符；`MutableArray` 与 `Array` 类型不兼容（不变性）。

## 5. 临时决策（及退役里程碑）

1. **数组是编译器内建类型**（同 M3 Option 的先例）：class/方法已在 M6 落地，但 core 的数组 class 声明尚未补齐；内建识别点收敛在 HIR 一处，布局与语义不变。
2. **越界 trap（已退役）**：M8 已由 `IndexOutOfBoundsException` 取代。
3. **引用类型元素必须同类型（已退役）**：M6 类型层级落地后已补齐 LOB 推导；无法用单个最具体类型表达时按 spec 退化为 `Any`。
4. **互转只支持构造函数形式（已退役）**：M6 方法调用落地后，`toArray` / `toMutableArray` 已补齐。

## 6. 明确不做

- `for` 循环与区间（`IntRange` 等，spec 11.8）——需要接口/方法支撑，与 `for` 的归属一起在后续里程碑单独立项；
- `String` 下标/切片；切片；数组的 `==` 比较（spec 未定义数组相等语义，待 spec 补充）。
