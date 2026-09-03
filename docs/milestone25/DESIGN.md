# M25 设计：自有异常 ABI 与 libc++abi 退役

版本：0.1（设计草案）

本里程碑把 Scoop 进程内的原生异常实现从 libc++abi 迁移到自有异常记录、catch 状态和 personality；底层只调用 Itanium unwind Level I 的 `_Unwind_*` 接口。目标是让生成的 Scoop 程序不再链接或导入 libc++abi，同时保持 M8–M10 已确定的 `throw` / `try` / `catch` / `finally` / rethrow、moving GC root 与 suspend handler 物化语义不变。

M25 取代 `docs/milestone8/DESIGN.md` 3.4、3.5、4 中关于 `__cxa_*` / `__gxx_personality_v0` 的实现选择，并收束 `docs/milestone9/DESIGN.md` 的异常 external root 与 `docs/milestone10/DESIGN.md` 的异常物化实现。旧设计仍作为历史里程碑记录，不再作为当前异常 ABI 的实现依据。

---

## 0. 关键决策与范围

- Scoop 拥有自己的 `ScoopExceptionRecord`、`exception_class`、personality、caught-exception 栈以及 begin/end/rethrow 协议；不复用 C++ exception header、RTTI、catch bookkeeping 或 terminate handler。
- runtime 只消费 `<unwind.h>` 暴露的 `_Unwind_RaiseException`、`_Unwind_Resume`、`_Unwind_DeleteException`、context getter/setter 等 Level I 接口。生产产物不得导入 `__cxa_*`、`__gxx_personality_v0`、`__gcc_personality_v0`或 C++ terminate 符号。
- 当前实现 target 仍固定为 LLVM 22.1 + macOS/AArch64。Darwin 上这些 `_Unwind_*` 符号由默认链接的 `libSystem` 提供，因此最终链接删除 `-lc++abi`，也不添加当前 SDK 不接受的独立 `-lunwind`。
- 继续使用 LLVM `invoke` / `landingpad` / `resume`。codegen 只生成 catch-all 和 cleanup 两类封闭 LSDA action；Scoop 的具体异常类型匹配仍由 landing pad 后的 `scoop_rt_is_instance` 普通代码完成。
- 新抛出的异常仍按值复制。record 内的 payload 是稳定的 heap 外 Scoop 对象，并在 record 生命周期内登记为 stable external object root；moving GC 可以更新其中的出站引用，但不能移动 record 或 payload。
- caught 状态归当前 `ScoopThreadState` 所有，按栈维护。native exception record 不可跨线程、不可跨 suspend point，也不形成用户可见的 `exception_ptr` 或引用计数协议。
- HIR/MIR 的异常语义和现有 normal/unwind/cleanup CFG 不变；LIR 的 `LandingPad`、`CleanupPad`、`BeginCatch`、`EndCatch`、`Throw`、rethrow 与 `Resume` 抽象不变。M25 只替换其 codegen/runtime 落点并补强验证。
- C ABI 与 Scoop ABI FFI 的展开边界不因本里程碑扩大。foreign exception 进入生成的 Scoop EH 区域属于 fatal ABI error，不能被 Scoop catch 当作 `Throwable`。
- 验收以最终可执行文件和 object 的依赖、导入符号与行为共同判定；“源码里不再写 libc++abi”但产物仍带该依赖不算完成。

---

## 1. 现状、依赖规模与迁移边界

### 1.1 当前直接依赖

M25 设计前，生成程序的异常路径直接使用以下 libc++abi 能力：

| 能力 | 当前符号 | 当前调用方 | M25 替代 |
|---|---|---|---|
| 分配 ABI exception buffer | `__cxa_allocate_exception` | runtime `scoop_rt_throw` | runtime 自行分配 `ScoopExceptionRecord` |
| 首次抛出 | `__cxa_throw` | runtime `scoop_rt_throw` | `_Unwind_RaiseException` |
| 源码 rethrow | `__cxa_rethrow` | runtime `scoop_rt_rethrow` | 同一 record 再次 `_Unwind_RaiseException` |
| 开始 catch | `__cxa_begin_catch` | codegen `BeginCatch` | `scoop_rt_begin_catch` |
| 结束 catch | `__cxa_end_catch` | codegen `EndCatch` | `scoop_rt_end_catch` |
| 当前 primary exception | `__cxa_current_primary_exception` | uncaught 诊断 | runtime 自有 caught 栈 / throw 参数 |
| personality | `__gxx_personality_v0` | 每个含 landing pad 的函数 | `scoop_eh_personality` |
| terminate handler | `std::set_terminate` | runtime 初始化 | runtime 直接稳定诊断并 `abort` |

LLVM `resume` 已经落到 `_Unwind_Resume`，因此现状并不是“整个异常系统都由 libc++abi 提供”，而是 Level I unwinder 上还叠着一层 C++ language runtime。M25 删除的是后一层，并保留前一层。

M25 在当前 target 上允许的直接 unwind symbol 集合固定为：

| 符号 | 用途 |
|---|---|
| `_Unwind_RaiseException` | 首次 throw 与同record rethrow的两阶段展开 |
| `_Unwind_Resume` | LLVM cleanup landing pad继续 phase 2 |
| `_Unwind_DeleteException` | 结束record并统一触发cleanup callback |
| `_Unwind_GetLanguageSpecificData` | personality取得LSDA |
| `_Unwind_GetRegionStart` | LSDA相对地址基址 |
| `_Unwind_GetIP` | 选择当前call-site range |
| `_Unwind_SetGR` | 安装landing pad的exception/selector参数 |
| `_Unwind_SetIP` | 跳转到选定landing pad |

`_Unwind_Resume_or_Rethrow`不是必需依赖：Scoop record不支持forced unwind，rethrow可以明确开始新的普通phase 1。任何新增`_Unwind_*`导入都必须先进入target EH profile和本表，再修改实现。

### 1.2 需要修改的责任面

- `runtime/src/rt.c`：当前 exception buffer、`__cxa_throw` / rethrow、personality wrapper 和 terminate handler；
- `compiler/codegen/src/function/instruction/exceptions.rs`：`BeginCatch` / `EndCatch` 的 libc++abi symbol；
- `compiler/codegen/src/function.rs`：函数 personality symbol；
- `compiler/codegen/src/target.rs`：Darwin 最终链接参数中的 `-lc++abi`；
- `runtime/tests/rt_eh_test.cpp`：测试本身借助 C++ catch 验证 Scoop buffer，必须由不依赖 C++ runtime 的测试替换；
- codegen、driver 与 fixture 的 artifact 检查：增加 LSDA 形态、导入符号与动态库依赖断言。

这些修改不要求改变 parser、HIR 类型检查、MIR catch 分派或用户可观察的语言规则。

### 1.3 不属于本里程碑的 libc++ 依赖

`scoopc` 编译器进程通过 LLVM/inkwell 依赖宿主工具链中的 C++ runtime，不属于“生成的 Scoop 程序只使用 libunwind”的目标。M25 的验收对象是 Scoop runtime archive、生成的 object 和最终 Scoop executable；不要求重建 LLVM 或把编译器本身改成无 libc++。

### 1.4 为什么不能改用现成 C personality

`__gcc_personality_v0` 不是 `__gxx_personality_v0` 的无 RTTI替代品。Scoop catch 需要 phase 1 报告 `_URC_HANDLER_FOUND`，而通用 cleanup personality 不提供 Scoop handler search 语义。把 personality symbol 直接替换为 `__gcc_personality_v0` 会令 cleanup 可能运行，但 catch 无法成为选中的 handler。

因此可行方案必须实现一个 Scoop personality；这也是去掉 libc++abi 的主要复杂度，而不是改一条 linker flag。

---

## 2. Scoop exception record

### 2.1 稳定 identity

Scoop exception class 固定为：

```text
SCOOP_EXCEPTION_CLASS = 0x53434f4f50000000  // "SCOOP\0\0\0"
```

personality、begin-catch 与 cleanup callback 都必须精确比较完整 64 位值，不能只比较 vendor 前缀。该值一旦有已发布产物便视为 runtime ABI 常量；不同 target 的字节序只影响内存表示，不影响数值比较。

### 2.2 私有记录布局

逻辑布局如下，具体字段顺序与 offset 只属于 runtime 私有实现：

```text
ScoopExceptionRecord
├── magic / lifecycle state
├── allocation base、size 与 payload offset
├── caught_previous
├── _Unwind_Exception unwind
└── aligned Scoop object payload
```

约束如下：

- 每条 record 恰好含一个 `_Unwind_Exception`，并满足 `<unwind.h>` 对其大小和对齐的要求；
- `unwind.exception_class` 写入 `SCOOP_EXCEPTION_CLASS`；
- `unwind.exception_cleanup` 指向 Scoop cleanup callback；
- `unwind.private_1` / `private_2` 完全归 unwinder 所有，Scoop 不在其中保存 caught 状态；
- payload offset 按 `TypeDescriptor.align` 向上对齐，并对所有 size、alignment 与加法做 overflow 检查；
- payload 包含完整 Scoop 对象头和正文，因此可由普通 TypeDescriptor / `RefScan` visitor 扫描；
- raw `_Unwind_Exception *` 到 record、record 到 payload 的转换只存在于 runtime 私有 helper；生成代码只传递 opaque raw pointer。

需要用 `_Static_assert` 锁定 `_Unwind_Exception` 对齐、container conversion 与 target 基本整数宽度。runtime public header 不暴露该结构。

### 2.3 生命周期与所有权

一条 record 的状态机为：

```text
Allocated
  └─ register external root → InFlight
       ├─ selected handler → Caught
       │    ├─ EndCatch → Deleted
       │    └─ Rethrow → InFlight → Caught ...
       └─ no handler / unwind error → Deleted + abort
```

核心不变量：

- `InFlight` 与 `Caught` 状态都恰好持有一份 external object root；
- 只有 `_Unwind_DeleteException` 触发的 cleanup callback 可以撤销 root 和释放整条 allocation；
- `EndCatch` 不直接 free，只在生命周期确实结束时调用 `_Unwind_DeleteException`；
- rethrow 复用同一 `_Unwind_Exception` 和 payload，不新增 root、不复制对象；
- cleanup callback 用 state/magic 防止 double delete，但发现重复删除必须作为 runtime ABI error 终止，不能静默吞掉；
- thread detach、runtime shutdown 和测试 teardown 时 caught 栈、in-flight record 计数与 external root 计数必须为零。

### 2.4 首次抛出

`scoop_rt_throw(obj)` 的顺序固定为：

1. 验证当前线程已 attach 且 `obj` 是非空、已发布的 Scoop 对象；
2. 从对象头取得 TypeDescriptor，验证 size 至少覆盖对象头、alignment 合法；
3. 以 checked arithmetic 计算 record 总大小和 payload offset，用 native allocator 一次分配；
4. 初始化 record 元数据与 `_Unwind_Exception`，复制完整对象到 aligned payload；
5. 把 payload 登记为 stable external object root；登记成功之前不得调用任何会展开的操作；
6. 调 `_Unwind_RaiseException(&record->unwind)`。

`scoop_rt_throw` 保持 NoGC、`noreturn`。native allocation 或 root registry 同步不等于 managed allocation，不能触发 collector 或执行 Scoop 代码。

`_Unwind_RaiseException` 返回表示没有 handler或 phase failure。runtime 此时仍拥有 record，按 reason code 输出稳定诊断：正常走到栈底使用 `uncaught exception: <type name>`，协议/展开失败使用 `fatal unwind error: <reason>`；然后调用 `_Unwind_DeleteException` 并 `abort`。不经过 `std::terminate`，也不尝试恢复执行。

### 2.5 cleanup callback

cleanup callback 只做可界定的 native 工作：

1. 由 `_Unwind_Exception *` 找回 record；
2. 验证 magic、状态以及record已不在caught栈中；
3. 从 external root registry 撤销 payload；
4. poison/清零 debug 状态并释放 allocation。

它不得分配 managed 对象、触发 GC、抛异常或调用用户代码。`_Unwind_Reason_Code` 参数只用于一致性诊断，不改变 root/free 的 exactly-once 语义。

---

## 3. Personality 与封闭 LSDA 子集

### 3.1 生成侧契约

M25 不设计一套新的异常表格式。LLVM 22.1 仍从以下 IR 生成 Itanium LSDA：

- catch handler：`landingpad { ptr, i32 } catch ptr null`；
- cleanup-only pad：`landingpad { ptr, i32 } cleanup`；
- 同一 pad 不混合 Scoop typed catch、C++ RTTI或 filter clause；
- 源码的多个 `catch (T)` 共享 catch-all pad，随后执行现有的 `is_instance` 决策链；
- 未处理异常继续使用 LLVM `resume { ptr, i32 }`。

LIR 的 `ExceptionRecord` 名称仍指 LLVM landingpad 的 `{ raw exception pointer, selector }` aggregate，不等于 runtime 私有的 `ScoopExceptionRecord` 布局。M25 实现时应补充注释避免二者混淆，但不需要改变该 LIR type。

### 3.2 当前 Darwin/AArch64 encoding profile

codegen artifact qualification 必须锁定 LLVM 22.1 当前实际输出的子集：

| LSDA 项 | 允许值 |
|---|---|
| LPStart encoding | `DW_EH_PE_omit` |
| TType encoding | `DW_EH_PE_pcrel | DW_EH_PE_sdata4 | DW_EH_PE_indirect`（`0x9b`） |
| call-site encoding | `DW_EH_PE_uleb128`（`0x01`） |
| action | cleanup 的 `0`，或指向单一 catch-all type entry 的正值 |
| type entry | LLVM `catch ptr null` 对应的 null entry |

personality 不能实现“足够通用”的猜测式 DWARF parser，再对未知 encoding 尽量继续。target profile 声明哪组 codegen 输出，runtime 就只接受哪组；LLVM 升级或新增 target 必须先用 object qualification 观察实际 LSDA，再显式扩展 profile 与测试。

### 3.3 LSDA decoder

personality 内部使用一个不做 unwind side effect 的 decoder，输入 LSDA pointer、function region start 和当前 IP，输出以下封闭 sum：

```text
NoAction
Cleanup { landing_pad }
CatchAll { landing_pad, selector = 1 }
Malformed { reason }
Unsupported { feature }
```

decoder 必须：

- 使用 `_Unwind_GetRegionStart`、`_Unwind_GetLanguageSpecificData` 与 `_Unwind_GetIP`；普通 call frame 以 `IP - 1` 选 call-site range，并对零值做检查；
- 正确解码 ULEB128 / SLEB128，检测位移、指针和 range 加法溢出；
- 以 call-site table 的显式长度限制表扫描，要求 `[start, start + length)` 不重叠，并以checked arithmetic计算 `region start + landing-pad offset`；
- 验证 action offset、type index、单一 catch-all链与 selector；拒绝 typed catch、negative filter、循环action链和当前 profile 外 encoding；
- 区分“当前 IP 没有 action”与“表损坏”，后者始终 fatal；
- 不解引用 null type entry，也不把 type table address 当作 Scoop metadata。

LSDA API 不提供整个 section 的映射长度或function end，因此 runtime 无法单独证明任意恶意 pointer 后方都可读，也无法验证landing pad的function上界。生产安全由两层共同保证：codegen object verifier借助object section/symbol range证明所有Scoop-produced LSDA属于封闭形态且landing pad位于所属function，runtime decoder再对格式携带的长度、算术和局部索引做防御性检查。Scoop 不承诺装载手工伪造object后仍可恢复。

### 3.4 phase 1：handler search

`scoop_eh_personality` 首先验证：

- `version == 1`；
- action flag 只含当前阶段允许的位；
- exception class 精确等于 `SCOOP_EXCEPTION_CLASS`；
- 不是 forced unwind；
- 当前 frame 的 LSDA 可被封闭 decoder 接受。

在 `_UA_SEARCH_PHASE`：

- `CatchAll` 返回 `_URC_HANDLER_FOUND`；
- `Cleanup` 或 `NoAction` 返回 `_URC_CONTINUE_UNWIND`；
- foreign exception、forced unwind、malformed/unsupported LSDA 进入 fatal unwind 诊断，不冒充未匹配异常继续传播。

cleanup-only action 绝不能在 phase 1 返回 handler found，否则 unwinder 会错误地把 cleanup frame 当作终点。

### 3.5 phase 2：安装 landing pad

在 `_UA_CLEANUP_PHASE`：

- 普通中间 frame 的 `Cleanup` 安装 cleanup landing pad；
- 带 `_UA_HANDLER_FRAME` 的已选 frame 必须解码为 `CatchAll`，安装 catch landing pad；
- 普通中间 frame 即使含 `CatchAll` 也不能再次截获；没有 cleanup 时继续展开；
- phase 1/phase 2 对同一 handler frame 得到不同 action 是 fatal 表一致性错误。

安装上下文前通过 `_Unwind_SetGR` 写入 ABI 两个 exception data register：第一个为 `_Unwind_Exception *`，第二个为 selector（catch-all 为 `1`，cleanup 为 `0`）；再以 `_Unwind_SetIP` 写入 landing pad address，返回 `_URC_INSTALL_CONTEXT`。寄存器编号使用 target 可用的 `__builtin_eh_return_data_regno(0/1)`，不能在跨 target 代码中硬编码 AArch64 寄存器号。

### 3.6 `resume` 与 rethrow 不同

- `resume` 表示继续当前 phase 2，必须沿用 LLVM `resume` → `_Unwind_Resume`；它不重新执行 handler search。
- Scoop `throw` 表示创建新 record并开始新的 phase 1。
- Scoop rethrow 表示对当前 caught record开始新的 phase 1，以便外层 handler重新参与匹配；M25 的 record只支持普通非-forced unwind，因此清除本次 caught 状态后直接再次调用 `_Unwind_RaiseException`。LLVM libunwind 会重新初始化其 private unwind 字段。

三者不能共享一个 runtime entry，也不能靠当前 CFG 形状猜测语义。

---

## 4. Catch 状态协议

### 4.1 线程状态

`ScoopThreadState` 增加 private `caught_exception_top`。caught链不做额外分配，每条active record以自身的private字段记录：

- 进入前的 previous top；
- `InFlight` / `Caught` / `Rethrowing` / `Deleting` 生命周期状态。

该状态只由当前 OS thread读写，不使用全局“当前异常”。同一active record不得重复压栈；外层重新捕获rethrow record前，原handler的`EndCatch`已将它弹栈并恢复为`InFlight`。foreign-thread callback attach时从空栈开始；detach要求栈为空。异常物化只改变payload的存储位置，不允许把native caught frame转移到另一个线程。

### 4.2 `scoop_rt_begin_catch`

生成代码在 catch dispatch 块调用：

```text
scoop_rt_begin_catch(raw_unwind_exception) -> managed payload ref
```

入口执行：

1. 验证 thread state、raw pointer、exception class、record magic 和生命周期；
2. 要求record处于`InFlight`且不在当前caught链，把其`caught_previous`设为旧栈顶并以`Caught`状态压栈；
3. 同一 record再次被外层 handler捕获时复用 record，不能复制 payload；
4. 返回 payload 的 AS1 managed ref。

payload 已在首次 throw 前登记为 external root，因此 `BeginCatch` 只发布已有稳定引用，不做 raw-to-managed 地址空间 cast。codegen 应以正确 AS1 signature 声明该 runtime entry。

### 4.3 `scoop_rt_end_catch`

入口只允许结束栈顶 caught frame：

- 验证 LIFO、record identity 与当前生命周期状态；
- 弹出当前层，恢复 previous top；
- 若record处于`Rethrowing`，只结束本层caught ownership并恢复为`InFlight`，不删除record；
- 若record处于`Caught`，切到`Deleting`并调用`_Unwind_DeleteException`，由cleanup callback撤销root并释放；
- 其他状态一律是fatal exception state error。

所有正常离开 catch 的 CFG edge必须恰好执行一次 `EndCatch`。return、break/continue、finally 覆盖和handler内新异常沿用 MIR 已显式生成的 cleanup edge；M25 不在 codegen 里补猜配对。

### 4.4 `scoop_rt_rethrow`

rethrow 的顺序固定为：

1. 验证存在 current caught frame；
2. 在栈顶 frame / record 上标记 rethrow；
3. 对同一个 `_Unwind_Exception` 调 `_Unwind_RaiseException`，从当前调用点开始新的 search；
4. 展开原 handler 时，其 cleanup path调用 `EndCatch`，后者弹出旧 catch 但保留 record；
5. 外层 catch选中后再次 `BeginCatch`，观察同一 payload identity。

如果 raise 返回，说明外层无 handler或发生 unwind错误；runtime 输出 uncaught/fatal诊断，先以内部 helper解除仍残留的 caught frame，再 `_Unwind_DeleteException` 并终止。不能让 `_Noreturn` 入口返回到生成代码。

handler 内抛出另一个对象不是 rethrow：新对象形成新 record；展开旧 handler时 `EndCatch` 正常删除旧 record，然后 LLVM `resume` 继续传播新 record。caught 栈必须正确覆盖 `catch A { throw B }`、嵌套 catch 和 finally 再抛三种组合。

### 4.5 NoGC 与 safepoint

personality、`scoop_rt_begin_catch`、`scoop_rt_end_catch` 和 `scoop_rt_rethrow` 都是 NoGC native runtime path：

- 不分配 managed对象；
- 不等待或发起 GC safepoint；
- 不执行用户代码；
- 不保留未登记的 movable GC pointer；
- 不允许从 native caught stack直接扫描 payload，root来源仍只有 external root registry。

`scoop_rt_materialize_exception` 是 generated managed entry，可分配和触发GC；调用它时 payload root仍有效，分配返回后必须重新读取可能已由 collector 更新的 payload 出站引用，再完成正文复制。

---

## 5. 编译器与 runtime 落地

### 5.1 HIR 与 MIR

HIR/MIR 不新增语义节点：

- catch类型决议、遮蔽、exhaustiveness与finally覆盖仍由现有 HIR/MIR 负责；
- MIR 保留 `LandingPad`、`BeginCatch`、`EndCatch`、`Throw`、`Rethrow`及显式 unwind edge；
- `CaughtException` 仍必须被 `BeginCatch` 支配；
- suspend transform继续保证 native catch状态不跨挂起点。

对应 golden dump原则上只因注释或 symbol dump变化而变化，不能出现为了适配personality而把 source type匹配下沉到 LIR/runtime 的新结构。

### 5.2 LIR 与 codegen

LIR 抽象保持不变，codegen 做以下机械替换：

| LIR / CFG | M25 LLVM/runtime 落点 |
|---|---|
| function personality | `scoop_eh_personality` |
| `LandingPad` | `landingpad ... catch ptr null` |
| `CleanupPad` | `landingpad ... cleanup` |
| `BeginCatch` | `scoop_rt_begin_catch(raw)` |
| `EndCatch` | `scoop_rt_end_catch()` |
| `Throw` | `scoop_rt_throw(obj)` |
| rethrow runtime target | `scoop_rt_rethrow()` |
| `Resume` | LLVM `resume` |

`scoop_rt_throw`、`scoop_rt_rethrow`、`scoop_rt_begin_catch`、`scoop_rt_end_catch`和`scoop_eh_personality`全部属于generated/runtime private ABI；throw/rethrow声明从`runtime/include/scoop_rt.h`移入private header，begin/end从一开始只在private header声明，personality只在runtime EH内部header声明。普通 C FFI public surface不暴露建立Scoop EH控制流的入口。

codegen 单元测试必须同时断言 IR 出现新 symbol、旧 symbol完全缺席，以及 landingpad clause / cleanup bit未变化。

### 5.3 runtime 模块划分

异常实现从已较重的 `runtime/src/rt.c` 拆为独立模块，建议责任如下：

- `runtime/src/eh.c`：record、throw/begin/end/rethrow、cleanup与诊断；
- `runtime/src/eh_personality.c`：LSDA decoder和personality；
- `runtime/src/eh_internal.h`：仅runtime内部共享的record与decoder类型；
- `runtime/src/generated_entries.h`：生成代码可见、源码不可见的 throw/rethrow/begin/end/materialize 声明。

模块命名可随现有 runtime 构建结构微调，但 personality parser、record lifecycle 与通用 GC/runtime 逻辑不能继续全部堆入 `rt.c`。每个新 C source 必须由 target profile 的明确 runtime source set选择，不能靠 host wildcard 隐式加入。

### 5.4 target 与链接

`DarwinAarch64` target profile 的最终 linker args 从 `-pthread -lc++abi` 改为仅保留非异常所需参数；system C driver默认加入 `libSystem`，由其解析 `_Unwind_*`。不得为通过本机测试而硬编码 Xcode 内部的 `libunwind.tbd` 路径。

target capability 增加不可缺失的 EH profile，至少声明：

- unwind model：Itanium/DWARF；
- personality ABI：Scoop LSDA subset v1；
- exception data register count：2；
- LSDA encoding profile；
- unwind provider/link策略：Darwin libSystem re-export；
- artifact inspection strategy：Mach-O。

未来 target 若没有完全对应的 provider、encoding与寄存器ABI，必须在 target selection/build阶段诊断，不可回退到 libc++abi。

### 5.5 uncaught 与 fatal 诊断

删除 `std::set_terminate` 后，诊断入口按来源区分：

- `_URC_END_OF_STACK`：`uncaught exception: <qualified type name>`；
- phase 1/2 reason code错误：`fatal unwind error: <phase>: <reason>`；
- foreign exception：`fatal unwind error: foreign exception entered Scoop EH`；
- malformed/unsupported LSDA：包含稳定的分类和 function address，不输出随机内存；
- begin/end/rethrow状态破坏：`fatal exception state error: <reason>`。

诊断过程中只读取已验证仍存活的 TypeDescriptor/name，不分配 managed对象，不走 Scoop `toString`，最后统一 `abort`。

---

## 6. GC、协程、线程与 FFI

### 6.1 moving GC

exception payload 复用 runtime spec 3.3 的 stable external object root能力：

- collector把 payload当作稳定对象起点，按其 TypeDescriptor扫描并原地改写所有出站managed ref；
- payload自身不是GC heap object，不参与mark/evacuation，也不出现在object-start map；
- record address、payload address和 `_Unwind_Exception *` 在整个生命周期保持不变；
- root登记发生在 raise前，撤销只发生在 cleanup callback；
- catch local取得的 payload ref可参与普通AS1 liveness，但跨safepoint时其稳定性来自 external object契约，不得被误当作可evacuate heap allocation。

moving stress至少覆盖 payload 内含 direct ref、嵌套aggregate ref、数组/enum ref，以及 throw 后到 catch 前和catch内分配时发生compaction。

### 6.2 suspend handler

M10协议保持不变：选中的 catch 或可能挂起的 finally在首次挂起前调用 `scoop_rt_materialize_exception(payload)`，得到普通 managed heap副本，然后立即 `scoop_rt_end_catch()`。

- native record、caught frame与 external payload都不能进入 coroutine frame；
- 恢复后继续传播时，对 managed副本重新 `scoop_rt_throw`，建立新 native record；
- 同一次未物化 rethrow 保持 record/payload identity，物化后的新 throw不保证 identity；
- verifier继续拒绝任何 `CoroutineStep.Suspended` 可达路径携带未配对 `BeginCatch`。

### 6.3 多线程

- caught 栈位于 `ScoopThreadState`，同一 record只在抛出/处理它的线程上使用；
- runtime不提供把 `_Unwind_Exception *` 放入 `GcHandle`、callback token或共享队列的API；
- 想跨线程传递失败时必须先物化为普通 managed `Throwable`，再使用语言/标准库的同步机制；
- foreign callback gateway返回 C 前必须物化并结束全部 native catch，token只保存 managed exception handle；
- shutdown统计中加入 active exception record / caught frame，非零即拒绝销毁 runtime。

### 6.4 FFI 边界

M25 不尝试让自有personality与任意 C/C++/Rust exception互操作：

- Scoop exception不得穿越 C ABI frame；
- 初版 Scoop ABI FFI 仍按 runtime spec 5.5 禁止异常越过外部实现；
- foreign exception class进入 Scoop personality立即fatal；
- 不提供把 C++ exception翻译为 Scoop `Throwable` 的adapter；
- 不承诺 Scoop exception可被 C++ `catch (...)` 安全消费。

这使“只依赖libunwind”保持为Scoop内部闭合协议，而不是宣称与所有Itanium语言runtime ABI兼容。

---

## 7. 验证计划

### 7.1 LSDA decoder 单元测试

以纯 byte fixture 覆盖：

- `NoAction`、cleanup、catch-all；
- 多个call-site range与边界 IP；
- 最短/多字节 ULEB128、SLEB128；
- 截断header/table、overflow、非法encoding；
- action offset越界、cycle、typed catch、negative filter；
- null/错误landing pad与phase不一致。

测试不调用真正 unwinder，便于对每个decoder分支做确定断言。fuzz/属性测试可补充，但不能替代上述命名case。

### 7.2 runtime 生命周期测试

替换 `runtime/tests/rt_eh_test.cpp`，测试驱动本身不得用 C++ throw/catch或链接 C++ runtime。主要行为通过生成的 Scoop fixture验证，C单元测试只直接检查record helper/decoder的封闭内部契约。

至少覆盖：

- throw → catch → end，payload copy、TypeDescriptor与字段完整；
- 内层catch rethrow，外层观察同一payload地址；
- handler内 throw新异常，旧record先释放，新record被外层捕获；
- nested catch、catch未匹配、finally正常/异常/覆盖路径；
- 未捕获异常的稳定诊断和非零终止；
- begin/end/rethrow非法状态的fatal诊断；
- 每个进程结束时 external root、record与caught frame计数归零。

### 7.3 GC 与协程组合

- 异常payload携带对象图，raise后、catch前及catch内触发moving compaction；
- rethrow跨多层frame后引用仍指向relocated对象；
- catch物化后结束native catch，再挂起/恢复/重新throw；
- callback捕获异常到token handle，返回C时caught栈为空；
- 多mutator并发抛出，各线程caught栈隔离。

### 7.4 IR 与 object qualification

codegen测试检查：

- 每个含landingpad函数的personality为 `scoop_eh_personality`；
- catch pad仍是 `catch ptr null`，cleanup pad仍带 `cleanup`；
- `BeginCatch` / `EndCatch` 只调用 Scoop runtime entry；
- `resume` 仍生成 LLVM `resume`；
- object中的 `.gcc_except_tab`/对应Mach-O section满足3.2封闭profile；
- personality symbol、call-site range、action和type entry可由artifact verifier逐项解释。

### 7.5 最终产物依赖验收

Darwin executable使用 `nm -u` / `dyld_info -imports` / `otool -L` 等价检查，必须满足：

- 不导入任何 `__cxa_*`；
- 不导入 `__gxx_personality_v0`、`__gcc_personality_v0`、`std::set_terminate`或其他 C++ terminate symbol；
- 不加载 `libc++abi.dylib`；
- 只出现设计允许的 `_Unwind_*` symbol，并由默认 `libSystem` 解析；
- link command不含 `-lc++abi`，也不含显式 `-lunwind`；
- runtime测试和fixture harness本身不通过 C++ driver偷偷重新引入 libc++abi。

应把禁止symbol集合做成集中常量供测试复用，不能在多个测试中各自维护不一致列表。

### 7.6 全量回归

完成迁移后按项目准则先执行 formatting / lint，再执行 runtime tests、codegen/driver tests、exception fixtures、moving-GC stress和完整 `cargo test`。M1–M24既有异常与协程fixture必须无语义变化通过。

---

## 8. 实现顺序与提交门禁

### 8.1 文档与基线锁定

- 合入本设计、runtime spec、implementation spec与roadmap；
- 把当前八类libc++abi导入和Darwin依赖固化成“迁移前基线”测试说明；
- 以当前LLVM 22.1 object锁定3.2的实际LSDA bytes/profile。

### 8.2 可独立验证的 decoder

- 实现封闭LSDA decoder及纯C单元测试；
- 增加object qualification工具/测试，证明codegen实际只产生decoder接受的形态；
- 此阶段不得提前切换生产personality，也不得对未知encoding静默接受。

### 8.3 原子切换异常 runtime

在同一批可构建变更中：

- 加入record lifecycle、Scoop personality和thread-local caught stack；
- 把throw/rethrow/begin/end全部切到新runtime；
- 删除所有`__cxa_*`声明、gxx personality wrapper和terminate handler；
- 替换 C++ EH runtime test；
- 更新generated private header与target runtime source set。

不能长期保留“新旧两套runtime按开关选择”的双轨，也不能在新personality失败时fallback到C++ personality。

### 8.4 链接与产物门禁

- 删除 `-lc++abi`；
- 通过完整异常、GC、协程和callback回归；
- 加入最终Mach-O依赖/符号否定断言；
- 验证clean环境中没有测试harness间接拉入libc++abi。

### 8.5 收尾

- 删除遗留C++测试source/build rule和过时注释；
- 确认public/private runtime header分层；
- 更新所有历史设计的“已被M25取代”指向，但不改写历史决策正文；
- 运行 `cargo fmt --all`、`cargo clippy --workspace` 和完整测试矩阵。

每一阶段都必须保持主分支可构建、相关测试可运行；迁移完成前不把M25标记为完成。

---

## 9. 验收标准

M25 完成必须同时满足：

1. Scoop runtime拥有record、personality和caught栈的唯一实现，源代码中无生产用`__cxa_*`/gxx/gcc personality/terminate调用；
2. throw/catch/finally/rethrow、handler内替代异常与uncaught诊断行为通过独立及组合fixture；
3. moving GC能扫描并更新external payload中的出站引用，root在所有终止路径exactly once撤销；
4. suspend handler先物化再挂起，native caught状态不进入coroutine frame；
5. 多线程/callback场景caught状态隔离，detach/shutdown不存在残留record；
6. LSDA decoder和artifact verifier只接受LLVM 22.1 Darwin/AArch64封闭子集；
7. 最终程序不链接libc++abi，不导入任何禁止symbol，`_Unwind_*`由libSystem提供；
8. HIR/MIR/LIR的语言级异常语义未发生未写入spec的变化；
9. M1–M24回归、lint和完整测试通过。

---

## 10. 明确不做

- Windows SEH / `catchpad` / `cleanuppad`、ARM EHABI或SJLJ；
- macOS/AArch64以外target的unwind provider与LSDA profile；
- C++ typed catch、RTTI、exception specification/filter或C++ exception互操作；
- 异常穿越 C ABI / 任意 Scoop ABI FFI frame；
- foreign exception到Scoop `Throwable` 的转换；
- 跨线程native exception record、`exception_ptr`、公开引用计数；
- 让native catch状态跨suspend point；
- 用backtrace或demangler扩展uncaught诊断；
- 重建LLVM、移除`scoopc`编译器进程自身的宿主libc++依赖；
- 更改Scoop现有throw-by-value语言语义；
- 在personality中接受任意LLVM/DWARF LSDA形态或提供libc++abi fallback。

---

## 11. 参考边界

- Itanium C++ ABI，Exception Handling：Level I Base ABI；
- LLVM Language Reference：Exception Handling Instructions；
- LLVM libunwind `UnwindLevel1.c`：`_Unwind_RaiseException`、`_Unwind_Resume`、`_Unwind_DeleteException` 的实现契约；
- `docs/specs/SCOOP-RUNTIME-SPEC.md` 第3.3、5章；
- `docs/specs/SCOOP-IMPL-SPEC.md` 第2.4、2.5节；
- `docs/milestone8/DESIGN.md`、`docs/milestone9/DESIGN.md`、`docs/milestone10/DESIGN.md` 的历史异常/GC/协程设计。
