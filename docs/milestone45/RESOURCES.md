# M45：core 资源协议与 use

状态：资源 API 待实施；第 6 节的推断修复已完成本机验证，见 [实施记录](PROGRESS.md)。所属里程碑与任务见 [M45 设计](DESIGN.md)。本文件中的 core API 是本期新增设计，不表示已落地。

## 1. API 与归属

以下普通 public 声明放入 `scoop.core` 的现有根 package，随普通 prelude binding 可见。接口及 use 不依赖 platform／std，也没有 compiler-known identity。

```scoop
public interface Disposable {
    public fun dispose()
}

public class ResourceUseException public constructor(
    public val primaryFailure: Throwable,
    public val cleanupFailure: Throwable,
) : Throwable()
```

`ResourceUseException` 保存正文失败和清理失败的普通 managed 引用。它直接继承 Throwable，使任意 Throwable（包括断言失败）组合后的结果不会被无意归入仅捕获业务 Exception 的路径；调用方可以检查两个字段。它是 use 的配套 core 类型，不改变整个 Throwable 层级，不增加全局 suppressed 列表或异常注册表。

两个扩展的公共签名为：

```scoop
public fun <T : Disposable, R> T.use(block: (T) -> R): R

public suspend fun <T : Disposable, R> T.use(block: suspend (T) -> R): R
```

这里展示签名，实施时提供完整 Scoop 正文。`T` 保留 receiver 的具体类型，block 内可以调用该资源的具体成员；不把回调参数降为 Disposable。首版只接收实际 Disposable，不增加 Option receiver 的隐式跳过重载。

Disposable 可以由普通类型实现。拥有可变 native 生命周期的 platform 资源使用 final class，使别名共享同一关闭状态；不使用按值复制的 struct 独立拥有同一个 fd 或 native allocation。core 接口不自动赋予所有权、可复制句柄或线程安全。

## 2. dispose 的合同

`dispose()` 表示立即开始释放当前对象拥有、且已不再需要的资源。它是普通同步函数，可以做 managed 操作、分配、调用其他 Disposable，也可能因为 flush、等待或 OS 操作而阻塞／抛异常；“同步释放”不承诺调用耗时上限。

基本规则：

1. 已完成逻辑释放后，重复 dispose 无操作，不再次释放资源，也不重新抛出上一次失败。
2. 只释放本对象拥有的资源。普通引用、借入的 stream／fd／pointer 或标准流不因实现者能访问它们就变为 owned。
3. native owner 开始消费资源时，先取出资源并将共享 owner 状态置为 inert，再操作取出的 native 值；后续 GC hook 或再次 dispose 不处理同一份资源。
4. 即使 flush 等前置收尾失败，也必须尝试释放已拥有的原始资源，保留实际失败；不能因为第一个异常跳过其余必要清理。
5. 前置条件不满足而尚未开始释放时可以拒绝调用并保留资源。例如 buffer 正在借用、mutex 正被持有，抛 IllegalStateException，调用方在前置条件恢复后仍可释放。这不算已经完成的一次 dispose。
6. 并发使用／关闭的安全性由具体类型规定。接口本身不插锁，不允许把数据竞争解释成“幂等关闭已经保证线程安全”。

platform 的惯用 `close()` 是同一逻辑释放操作的等价入口，不保留两套相互独立的关闭状态。`isDisposed` 是具体 owner 可提供的查询，不加入最小 Disposable 接口。

`use` 不检查 receiver 是否已关闭，因为接口没有统一的可用状态查询。具体资源操作按自身状态报错；use 仍保证它进入后调用正文一次并尝试清理。

### 2.1 构造与获取失败

release hook 只保护完整构造成功的对象。获取 fd／native pointer／线程 token 后，包装构造、配置或后续步骤失败，获取方必须显式回滚。

优先先建立不持有 native resource 的内部 owner，再获取和无异常地提交句柄；不能这样组织时，在获取函数的异常路径释放临时所有权。公开 factory 成功返回时才把完整 owner 交给调用者，不暴露半初始化对象。

receiver 与 block 实参的求值发生在进入 use 之前。use 不替尚未进入自身调用的失败补做清理；多步获取仍按构造／factory 的回滚合同处理。

## 3. use 的退出与异常

use 以当前 receiver 调用 block 恰好一次。正文退出后调用 dispose 恰好一次，并按下表决定结果：

| 正文 | dispose | use 的结果 |
| --- | --- | --- |
| 返回 R | 成功 | 返回原 R |
| 抛 primary | 成功 | 重抛原 primary 对象 |
| 返回 R | 抛 cleanup | 抛原 cleanup 对象 |
| 抛 primary | 抛 cleanup | 抛 ResourceUseException(primary, cleanup) |

捕获范围只用于区分正文和清理失败。不能把成功路径中的 dispose 异常再次当成正文异常，然后重复 dispose。两次异常的先后、身份和来源都保留，不用 cleanup 覆盖 primary。

这套语义用普通 try/catch/finally 与泛型代码实现，不增加特殊异常展开机制。嵌套 use 自然从内到外清理；内层获取失败时已经进入的外层 use 仍释放其资源。多层清理失败可以形成嵌套 ResourceUseException，不为本期增加通用异常树平坦化框架。

use 是普通函数调用。它不引入 inline 的非局部 return、不改变 lambda 的 return 规则，也不会自动在变量离开任意词法作用域时 dispose。

### 3.1 挂起重载

回调类型 `(T) -> R` 与 `suspend (T) -> R` 不同，因此可以区分重载。只有函数自身是否 suspend 不同、参数列表相同的两个声明仍属于重复签名。

```scoop
resource.use { value -> inspect(value) }

resource.use(suspend { value -> inspectSuspending(value) })
```

第二种调用必须位于允许挂起的上下文。普通 lambda 不会因调用者为 suspend、期望类型或正文调用了挂起函数而自动转换。

挂起版 use 在整个正文生命周期保活 receiver；真实挂起只保存状态，不执行 dispose。恢复后正常返回或异常展开才清理。dispose 本身仍是同步函数，可能在恢复线程上执行，因此要求特定线程的资源必须另行约束使用场景。

当前没有协程取消合同。放弃且永不恢复 continuation 不会自动执行 finally／dispose；M45 不增加这种保证。NativeByteBuffer 的 owner 可以保存在挂起状态中，但裸 native pointer 的借用回调仍必须同步结束，不能借助挂起版 use 延长其借用。

## 4. 与 release hook 的关系

| 显式 dispose／use | GC-free release block |
| --- | --- |
| 普通 managed 执行，可分配／抛异常／调用其他资源 | 没有 managed this，只读取合格 backing field |
| 负责及时的业务收尾和原始资源释放 | 只对仍拥有的原始 native resource 作尽力清理 |
| 可以 flush、join、按合同等待 | 不能 flush、join、等待停顿的 mutator、调用用户代码 |
| caller 能观察失败 | 不向用户传播 managed 异常，不保证 native 清理成功 |
| 在正常或异常退出时执行 | 不保证触发时机、线程、对象间顺序或进程退出时执行 |

hook 不能调用 dispose，包括通过 helper、接口 dispatch 或藏入 C callback 的间接调用。显式路径先置 inert；hook 以后读取到 inert 就跳过。owner 移动不会再次释放旧副本，完整构造前的失败不能靠 hook 回收。

对于拥有其他 managed resource 的组合对象，显式 dispose 逐项清理自己的 owned 子对象；hook 不遍历该对象图。各子 owner 如有合法 raw cleanup，分别保留自己的 hook。

具体策略在 [平台 API](PLATFORM-API.md) 中固定：

- NativeByteBuffer：free 仍拥有的连续 allocation，通过与显式释放共用的 GC-free pressure 提示入口报告容量减量；不访问 GC owner，也不要求 GC 记录或处理提示。
- File／pipe／socket／目录枚举：仅尝试原始关闭，不 flush；错误和不确定状态不构成自动重试许可。
- Mutex／ConditionVariable：不能以 hook 解锁或唤醒仍在使用的对象；销毁前必须满足具体 native 合同。
- Thread：显式 dispose 等待结束；兜底最多撤销 native join 观察权，不等待 worker，也不操作 managed callback ownership。
- ChildProcess：显式路径负责 wait/reap；hook 只允许非阻塞的已退出状态回收，不提供后台 reaper 或保证仍在运行的子进程会被收走。

这提供清楚的确定性清理惯例和原始资源兜底，不把遗漏 wait、死锁、数据竞争、OS close 失败或异常终止变成已获保证的正确程序。

## 5. 类型、编译与验证职责

新增 core 源码按普通 interface／method／generic template 发布，跨 Cone 通过原实体调用；不新增 Disposable 特判、专用 MIR cleanup node、using lowering 或 release 自动合成。

行为测试由 platform 的 scoop.test 测试目标承载，以普通测试资源记录调用次数、顺序、异常对象和状态。挂起测试通过已有 Continuation 显式恢复；运行期回调失败用 assertFailsWith 检查，编译不合法的挂起／重载情况才使用必要的 compiler negative fixture。

native hook 与 managed owner 的精确保活、关闭后 GC 不二次释放、未完成构造不发布 hook，使用必要的 GC／runtime 专测。不要重复为每一种 platform 类生成完整 IR 快照。

## 6. M45-1：泛型扩展回调参数推断修复

设计时已用源码构建的编译器和 LLVM 22.1 在 Darwin/AArch64 验证：普通／挂起 `f` 的零参数回调重载可以编译运行；只相差外层 suspend 的重复签名、普通函数调用挂起重载、普通 lambda 调用挂起函数均给出相应错误。

另一个组合用例定义两个泛型扩展，签名与第 1 节 use 相同，正文都是调用 block 并在 finally 中 dispose。Resource 是实现 Disposable 的普通 class，具有 `disposed: Boolean` 状态。以下调用在修复前报错：

```scoop
val initial = Resource().use { resource ->
    println(resource.disposed)
    11
}
println(initial)
```

primary 指向 lambda 的 `resource` 参数，诊断为：

```text
lambda parameter requires a type when there is no expected function type
```

把整个 use 嵌入 println 也会失败；先保存结果没有解决。显式写 `resource: Resource` 后，普通／挂起 use、真实挂起／恢复和 finally 释放均可运行。这是定位用对照，不是允许交付给库用户的 workaround。实施时确认 receiver 约束已进入求解器，但共同的 postponed-argument 路径要求整个函数类型可替换，导致未知 R 阻止已知 T 传入 lambda；修复允许完整函数头先参与正文推断。

任务要求：

1. 保留失败现象并缩减成独立回归，分别检查普通与挂起 lambda、构造表达式／已绑定变量 receiver、直接／嵌套调用和显式类型对照。
2. 按语言规范 8.6，把已知 receiver 对 T 的约束带入候选局部推断，给延期 lambda 提供其参数期望类型，再利用正文确定 R；候选间状态隔离，不能在上下文尚可求解时提前要求标注参数。
3. 修复共有调用／泛型推断路径，不按 `use`、Disposable、Resource 的名称或来源特判，不退化成 Any、不修改回调 API、不把实现转入 C。
4. 验证同 Cone 与上游普通 `.slib` 的 generic extension；已有合法显式写法继续成立，不让失败候选污染另一个重载。
5. 继续拒绝普通／挂起函数类型混用、仅外层 suspend 不同的重复签名、缺少真正可用约束的 lambda 以及普通上下文调用挂起函数，并检查诊断位置与原因。
6. 完成最小编译器回归与必要的既有 solver 回归后，再继续依赖该能力的 core use 和 platform 工作。

实施中遇到任何其他编译器或 core 缺口，同样先修复对应层并完成必要回归，再返回 platform；不能将问题换成库侧限制、显式类型要求、native 实现或特殊路径。需要改变语言或 runtime 合同时，先修订相应 spec。
