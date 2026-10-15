# M45：NativeByteBuffer 与 native IO 边界

状态：设计完成，待实施。范围、任务和测试规则见 [M45 设计](DESIGN.md)，资源协议见 [RESOURCES](RESOURCES.md)。

## 1. 类型与所有权

`NativeByteBuffer` 放在 `scoop.platform.memory`，是实现 Disposable 的普通 final class。对象由 GC 管理，字节存储是本对象独占的连续 native allocation。复制 class 引用只产生同一 owner 的别名，不复制 native 所有权或关闭状态。

首版用于文件、pipe、socket 及同步 C API 的字节输入输出；它不是保存 managed reference 的通用 native 容器。不得把任意 T、Scoop object pointer、GcHandle 或 closure 隐藏在字节区并期待 GC 扫描它们。

基本工厂为 `NativeByteBuffer.allocate(capacity: Long = 0L)`。容量为零时允许没有 native allocation。构造出的 buffer 没有可读数据，不承诺 native allocation 已清零。分配失败不能返回半可用对象，临时所有权按 factory 回滚规则处理。

复用 `scoop.bytes.ByteOrder`，不定义第二个 byte-order enum。容量、游标、数量与偏移使用 Long；native size_t／ssize_t 转换在 C 边界检查，M45 不引入 ISize／USize 或修改 core 数组索引类型。

## 2. 游标与初始化不变量

所有处于 open 状态的 buffer 保持：

```text
0 <= readPosition <= writePosition <= capacity

[0, readPosition)                 已消费但仍已初始化
[readPosition, writePosition)     可顺序读取的已初始化字节
[writePosition, capacity)         可写容量，不保证已初始化
```

只公开只读 `capacity`、`readPosition`、`writePosition`、`readableBytes`、`writableBytes` 和 `isDisposed`。没有任意设置 writePosition 的 safe setter；不能通过扩大 writer 游标把未初始化尾部变成可读数据。

safe 顺序读取只访问可读区；绝对 get／set 只访问 `[0, writePosition)` 中已初始化的完整区间。写入只能在成功初始化之后推进 writePosition。所有加法、区间终点和容量增长在地址计算之前检查溢出；先检查实际内存边界，不以指针 wrapping 代替错误处理。

| 操作 | 合同 |
| --- | --- |
| `clear()` | 两个游标归零，保留 allocation；不清零内存，旧内容不再能由 safe 读取暴露 |
| `compact()` | 将未读区移到起点，readPosition=0，writePosition=原 readableBytes；重叠使用 memmove 语义 |
| `skip(count)` | 消费可读前缀，检查 count 位于 0..readableBytes |
| `ensureWritable(count)` | 保证尾部至少有 count 可写字节；空间足够则不分配，不隐式 compact |
| `dispose()`／`close()` | 先置 owner 为 inert，再 free 并报告对应 pressure 减量；重复调用无操作 |

扩容只由显式 ensureWritable 触发，普通 read／write 不隐式扩容或搬移。需要增长时可以采用几何扩容，具体倍率不是公共合同；增长后容量不少于 `writePosition + count`，原 `[0, writePosition)` 内容和两个游标保持不变。失败时地址、容量、游标及已有内容全部保持原状。

首版不提供缩容、共享 view、duplicate 或 slice。输出数组是独立副本；释放 buffer 不使已经复制出的数组失效。

## 3. 受检字节与数值操作

| API 组 | 行为 |
| --- | --- |
| readByte／writeByte | 顺序消费／追加一个 Byte |
| readBytes(count) | 返回独立 Array<Byte>，全部成功后才推进 readPosition |
| readInto(array, offset, count) | 复制到 MutableArray<Byte> 的受检区间，成功后消费 count |
| writeBytes(array, offset, count) | 从 Array<Byte>／MutableArray<Byte> 的受检区间追加，空间不足时报错 |
| getByte(index)／setByte(index, value) | 绝对访问已初始化字节，不改变游标 |
| read／writeInt8、UInt8、Int16、UInt16、Int、UInt、Long、ULong、Float、Double | 定宽读写；除单字节外必须显式传入 ByteOrder；浮点按位型编码 |

readBytes 的结果分配在提交游标之前完成，失败不消费输入；数值操作先检查完整位宽，不留下半个数值或半次游标变化。字节数组批量操作先验证两侧区间；不为首版增加会执行任意 getter 的 List 输入路径。

native 区间不保证每次访问都满足数值类型的自然对齐，不能将任意 byte offset 直接 cast 成未对齐的整数 pointer 后 load/store。实现使用逐字节组合或 memcpy 到对齐局部，再按显式字节序转换；不是依靠当前 CPU 恰好接受未对齐访问。

负数量等非法值抛 IllegalArgumentException，超出可读／可写或数组区间抛 IndexOutOfBoundsException，大小运算溢出抛 ArithmeticException。native allocation 失败使用 platform 的 NoMemory 错误。关闭后，除了 isDisposed、dispose 和 close，其余操作与属性查询均抛 IllegalStateException；不会读旧 pointer。

## 4. 同步 native 借用

以下为 NativeByteBuffer 的公开 unsafe 成员签名，省略类型外层与实现正文：

```scoop
@Unsafe
public fun <R> withReadableBytes(
    block: (Ptr<Byte>?, Long) -> R,
): R

@Unsafe
public fun withWritableBytes(
    block: (Ptr<Byte>?, Long) -> Long,
): Long
```

withReadableBytes 提供当前可读前缀的起始地址与长度，不移动游标。调用方承诺只读该区间；core Ptr 没有 const 限定，不能由类型系统自动保证这一点。

withWritableBytes 提供当前可写尾部；回调返回从该地址开始连续初始化的字节数 n。仅在回调正常返回且 `0 <= n <= writableBytes` 时推进 writePosition 并返回 n。回调必须实际初始化整个前缀，不能只返回一个数量；库检查 n 的范围，不能证明 native 代码已经写过这些字节。

两个 API 在长度为零时传 None 和 0，并仍调用 block 一次；非空范围传 Some(pointer)。不为了制造一个非零裸 Ptr 而分配假字节，也不把零地址伪装成合法 Ptr。调用方须遵守具体 C API 对空范围／null 的要求。

### 4.1 借用期间的保证与责任

- block 是普通非 suspend 函数，可以分配 managed 对象、抛异常和执行同步阻塞 C 调用；借用必须在它返回或抛出时结束。
- owner 在整个 block 与退出清理期间保持 live managed root；finally 清除借用状态。GC 可以移动 wrapper，native allocation 地址在本次借用内保持不变，不需要 pin wrapper。
- 借用期间拒绝 dispose、ensureWritable、compact、clear、顺序读写、绝对写和其他改变字节／游标／allocation 的操作。重复或嵌套借用也拒绝；只读状态查询仍可用。拒绝发生在任何修改之前，抛 IllegalStateException。
- pointer 不得返回、存储、交给稍后完成的 native 操作、捕获到逃出的 closure 或用于挂起之后；callee 在返回前必须停止使用。M45 不提供一般性逃逸分析或静态 borrow checker，这些是 unsafe caller 的义务。
- writable block 只能写借出的尾部，不读取未初始化内容、不改写其他区间。回调抛出或返回无效 n 时不提交游标；尾部可能已被 native 修改，但仍不可被 safe API 读取。
- 不提供并发访问保证。借用标志用于拒绝同一对象的重入冲突，不是跨线程锁；跨线程共享必须由调用者同步，并遵守语言数据竞争规则。

挂起版 use 只延长 owner 的 managed 生命周期，不延长裸 pointer 借用。平台 IO 实现不能把 pointer 保留到方法返回之后。

## 5. 与同步 IO 的连接

ByteSource.readInto 将 native 输入直接写进 buffer 的可写尾部；ByteSink.writeFrom 将可读前缀直接交给 native 输出。公共 IO 不暴露 pointer，内部才使用上述 unsafe 借用。

| 操作 | 成功提交 | 失败／边界 |
| --- | --- | --- |
| 一次 readInto，最多 n 字节 | writePosition 增加实际读到的 k | n=0 返回 0，不据此判断 EOF；正请求返回 0 表示流 EOF |
| 一次 writeFrom，最多 n 字节 | readPosition 增加实际写出的 k | 正请求且 native 无进度不能无限循环；按 IO 错误处理 |
| readFully／writeFully | 每次成功的部分进度立即反映在游标中 | 后续错误不回滚已经发生的外部 IO，保留此前游标变化 |
| UDP receive／send | 按报文合同提交实际字节 | 空报文不表示 EOF；截断状态单独返回，见平台 API |

n 始终受本次 buffer 区间限制；单次 native 调用进一步受实际 ssize_t／平台最大传输范围限制。流 IO 允许短传输，不能把不足 n 自动当成 EOF 或异常。读取不会为“也许还有数据”自动扩容；调用方或缓冲包装显式准备容量。

一次 syscall 报错且没有交付字节时，位置保持不变。EINTR 重试必须区分是否已经发生进度；复合操作抛错后，游标就是已完成进度的事实，不声称整个操作失败原子。

核心数组直接与同步 C IO 交互时，仍可使用已有 withDataPointer，不要求先复制进 NativeByteBuffer。选择 off-heap 是为了可复用的稳定 native 字节存储，不是替换所有 managed byte array。

## 6. GC pressure factor 提示接口

NativeByteBuffer 只向 GC 报告其压力贡献的变化，不负责选择收集时机、minor／full、阈值、收集基线或回收重试策略。GC 可以使用、合并、加权或完全忽略这个提示。首版 GC 允许把入口实现为 no-op，这是明确允许的完整行为，不是未实现占位。

### 6.1 一个增减入口

在 core 现有 GC API 中增加一个操作，逻辑签名为：

```scoop
public fun gcAdjustPressure(delta: Long)
```

pressure factor 采用可加减的整数贡献，不采用由 buffer 计算的 GC 触发倍率。NativeByteBuffer 以已提交的 native 容量字节数作为自己的贡献：正 delta 增加压力，负 delta 撤销压力，零不产生变化。GC 如何解释这份贡献属于实现策略。

接口不携带 owner、pointer、分配 id 或 collector 类型，不返回是否应收集、配额或错误，不要求 GC 维护可查询的精确内存总账。它不改变资源所有权、根集合、地址有效期或 native 分配是否成功的判断。

| buffer 事件 | 反馈 |
| --- | --- |
| 成功分配并提交容量 c | `+c` |
| 成功扩容，已提交容量从 old 变为 new | `+(new - old)`，不再重复报告同一变化 |
| 分配／扩容失败，保留原存储 | 不报告变化 |
| clear、compact、普通读写或借用 | 容量未变，不报告变化 |
| 显式 dispose 或 release hook 实际释放容量 c | `-c`，同一份所有权只撤销一次 |
| 已报告的分配随后回滚 | 撤销此前报告的贡献；未报告的临时分配不产生虚假的负贡献 |

先提交成功的存储状态，再报告其变化；释放时先取出资源并将 owner 置为 inert，实际 free 后报告减量。增长过程的瞬时峰值、allocator metadata 和整个进程 RSS 不属于该 buffer 贡献的精确统计承诺。容量与差值继续受原有 Long／地址边界检查约束。

### 6.2 始终 GC-free，显式路径与 hook 共用

通知入口的效果合同不随当前 GC 是否采用该提示而变化：

- 不分配 managed 对象，不抛异常，不执行 safepoint／poll，不在调用栈内触发 GC。
- 不等待 mutator，不访问 owner、managed 引用、线程 TLS、root、handle 或 pin；允许从正常 native 资源操作和 collector 的 release hook 调用。
- 多线程调用安全；GC 若保留信息，应自行采用满足这个 leaf 合同的轻量记录方式。接口不规定必须有计数器、队列或锁，也不负责用户数据的同步。
- GC 以后决定利用压力时，在自己已有的合法收集决策位置处理；不能把通知入口改成可能同步收集的函数，从而破坏 hook 的调用合同。

源码声明沿 core 的现有 GC API 组织，以直接 C leaf 绑定同一个 runtime 操作；调用遵守现有显式 Unsafe 规则。不再拆成增加／减少入口、managed 压力检查入口和专用 hook 扣减入口，不增加新 intrinsic、collector 插件或 buffer 专用 GC 角色。

当前 release 合同禁止一般 GC runtime 操作，实施前仍须在语言规范 9.1.6／14.1 和 runtime 相关章节明确允许这个不触发收集的提示操作。不能因为首版恰为 no-op，就绕过效果检查或将任意 GC API 视为 release-safe；普通 NoGC helper 的既有推导规则保持。

### 6.3 首版 no-op 与策略边界

首版 GC 可以完全不保存 pressure 信息，M45 不要求实现外部字节计数、pressure 驱动的 full collection、收集基线或压力触发调优。其实现经 M44 的 GC 操作／适配边界接入，NativeByteBuffer 不依赖 Immix 私有结构。

NativeByteBuffer 不主动调用 collect，也不在 malloc／realloc 失败后要求 GC 收集并重试。失败按原有合同保留旧状态并报告 NoMemory。GC 将来增加压力策略时，不应要求 buffer 改变分配／释放 API。

忽略 pressure 不影响边界检查、保活、显式释放或正常 GC 回收时的 hook 合同；它意味着外部内存增长不会单独推动更早的收集，遗漏显式 dispose 的 buffer 可能更晚释放。这是允许的收集时机取舍，不是 native 资源释放正确性的替代机制。

M45 不建立逐读写／遍历／复制计费、资源配额、通用资源账本或产物成本合同。GC 的 no-op pressure 消费是本设计允许的特例，不允许以此绕过任何其他编译器／core 缺口。

## 7. 验收与明确后续

主要通过 scoop.test 验证所有公开状态、内容、边界和运行期失败；必要的 C／runtime 专测验证分配失败、pressure 通知的增减／回滚配平、no-op 合同、GC owner 保活、显式释放后再 GC、跨 moving 的借用和 hook 不重入收集。通知测试只在窄测试接缝观察 delta，不要求生产 GC 为测试保留计数器或增加可替换 collector。验收不要求 pressure 导致某次收集，也不以 RSS 是否立即下降证明 free 正确，不靠读取未初始化内存来测试禁止访问的规则。

首版不加入共享 slice／持久 view、池化、引用计数、分段存储、arena、mmap、O_DIRECT 对齐、公开任意 native T 存储或异步持有 pointer。需要这些能力时单独设计其实际所有权和生命周期，不预置空类型或无调用者的框架。

实施发现编译器或 core 无法正确支持本设计需要的普通接口、异常、数组、FFI 或根语义时，先修复对应层并回归，再继续 buffer；不得在库里绕过。
