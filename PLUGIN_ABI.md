# Scoop Plugin ABI 设计草案

**Version**: v0 (Draft)
**Status**: Design draft, not yet implemented. Open questions are inlined as `[OPEN]`.
**Scope**: 描述 Scoop 主程序与 Scoop plugin 之间的运行期协议。**不**适用于纯 C ABI 形式的 dynamic library 加载——那个属于 FFI，与本文档无关。

本文档与 `SCOOP_FULL_SPEC.md`、`SCOOP_RUNTIME.md`、`docs/archive/designs/STABLE_ID.md` 配套阅读。

## 0. 概念与目标

### 0.1 什么是 Scoop plugin

一个 plugin 是一个或多个 cone 编译产生的动态库（`.so` / `.dylib` / `.dll`），由 Scoop 主程序在运行时通过 `dlopen` / `LoadLibrary` 加载。Plugin 与主程序之间满足：

1. plugin 的代码与主程序代码使用**同一个 GC heap 与 runtime**；
2. plugin 与主程序对它们共享的类型有一致的 RTTI、layout 与 vtable 布局；
3. plugin 与主程序之间的所有交互通过显式的 ABI 表面进行；
4. plugin 加载后**不可卸载**（v0 不支持 unload）。

Plugin 不是用来做"主程序无关的扩展点"的；它是 separate compilation 在运行期的一个具体形式。

### 0.2 设计目标

1. plugin 与主程序在共享类型上**完全等价**——同一个类型在两边的 RTTI、`is`/`as`/`as?`、虚分派全都正常工作。
2. 初始化顺序明确、不依赖运行期约定（"恰好这样调用就 OK"）。
3. 在 ABI 不匹配时**于任何 plugin 代码运行前**就拒绝加载，避免污染主程序状态。
4. 不引入新的 mangling 协议、不复制 STABLE_ID 已经定义过的概念。
5. 把"跨界 effect"明确排除在 v0 ABI 之外，避免被强迫实现一套低性能、conservative 的跨界 state machine。

### 0.3 非目标

1. **不做 unload**。Plugin 一旦加载就常驻进程。这条是 ABI 决定，不是技术决定。将来要支持 unload 时升 v1。
2. **不做跨编译器版本兼容**。Plugin 与主程序必须由同版本编译器、同版本 runtime、同版本 sysroot 构建。版本不匹配在加载时拒绝。
3. **不做语言级跨界 effect**。Plugin 与主程序之间的 effectful 交互如有需要由用户库通过 token 协议自行实现，不进入语言/ABI 表面。详见第 6 节。
4. **不做"匿名"plugin**。每个 plugin 必须显式声明它依赖哪些主程序 cone、提供哪些 cone、暴露哪些入口。

## 1. 整体架构

```
┌─────── scoop_runtime.{so,dylib,dll} ───────┐
│  GC、thread、safepoint、allocator…          │
│  ── ABI: abi_exports_allowlist              │
└─────────────────────────────────────────────┘
       ▲                          ▲
       │ imports                  │ imports
       │                          │
┌──────┴──── 主程序 exe ───┐  ┌───┴──── plugin.so/.dll ────┐
│  user_main()              │  │  scoop_plugin_v0_describe │
│  plugin 加载逻辑          │  │  scoop_plugin_v0_init     │
│  （EXE 不含任何 cone）    │  │                            │
│                           │  │  plugin 私有 cones（在内）│
└───────────────────────────┘  └────────────────────────────┘
       ▲                          ▲
       │ imports                  │ imports
       │ (按 cone 拓扑序逐个 link) │
┌──────┴──────────────────────────┴──────────┐
│  cone DLLs（每 cone 一个 DLL）             │
│   ── sysroot / stdlib（String/List/...）   │
│   ── 应用私有 cones                        │
│   ── 应用公开给 plugin 的 cones            │
│  类型 descriptor / vtable 在此单实例       │
└─────────────────────────────────────────────┘
```

关键约束：

- **Runtime 是 dynamic library，不是静态链接到 EXE**。所有平台一致（详见 §1.1）。
- **每个 cone 都是独立 DLL，不区分公私**。EXE 自身不内嵌任何 cone（详见 §1.2）。
- **类型 TypeDescriptor 在所属 cone 的 DLL 中定义、由 EXE 与 plugin 共同引用**。Plugin **不重复 emit** 共享类型的 descriptor。`[OPEN: 实现细节见 §7.1]`
- **plugin 私有的类型 descriptor 完全在 plugin .so/.dll 内**，object header 指向 plugin 内部的 descriptor，GC 通过该指针 trace plugin pages（plugin 不卸载，指针永远有效）。

### 1.1 为什么 runtime 必须是 DLL（跨平台一致）

Linux/macOS 上"runtime 静态链接进 EXE，plugin 通过 `-rdynamic` 反向符号解析"是可行的；但 Windows PE 模型要求 import 在链接期就绑定到具体 DLL，不存在等价机制。技术上 EXE 可以生成 import library 让 DLL 反向 link，但维护成本高且非主流做法。

为了避免在 Windows 上单独维护一套链接策略，**v0 ABI 在所有平台上把 runtime 作为 dynamic library**：

- Linux: `libscoop_runtime.so`
- macOS: `libscoop_runtime.dylib`
- Windows: `scoop_runtime.dll`

主程序 EXE 与所有 plugin 都 import 同一个 runtime DLL。代价是每次 runtime 调用走 PLT/GOT 间接（Linux/macOS）或 IAT 间接（Windows），相对静态链接增加 ~1-2 cycles，远低于 GC/safepoint 自身开销，可接受。

收益：

- 单一加载路径，平台间行为一致
- 运行期 runtime 替换／升级路径更清晰
- 测试矩阵显著缩小

### 1.2 唯一部署策略：runtime DLL + 每 cone 一个 DLL

v0 ABI **不**支持"多个 cone 合并为一个 DLL"或"按需选择把哪些 cone DLL 化"等部署粒度选项。**唯一**支持的部署形态是：

- `scoop_runtime` 是一个 DLL
- 每个 cone 编译为**独立**的 DLL，无论是否被 plugin 引用

| 内容 | 形态 |
|---|---|
| `scoop_runtime`（GC/thread/safepoint/...） | 独立 DLL（§1.1） |
| 每个 cone（sysroot / stdlib / 应用 cone / host export 给 plugin 的 cone） | 各自一个 DLL |
| EXE 本体 | 仅含 `user_main` 与 plugin 加载逻辑 |

理由：

1. cone 是 separate compilation 与 ABI hashing 的天然单元；每 cone 一个 DLL 让"DLL = ABI 边界"对齐到"cone = ABI 边界"，不引入新的 grouping 概念
2. cone init 拓扑序直接对应 DLL load 顺序；`abi_hash` 校验粒度直接对应 DLL 校验粒度，没有额外簿记
3. "合并多个 cone 到一个 host_abi.dll"会迫使我们额外定义 ABI bundle 概念、设计 bundle 内 cone 解析协议、维护 bundle 与 plugin 之间的依赖匹配——投入与收益严重不成比例
4. "仅 shared cone DLL 化、私有 cone 留在 EXE 内"看起来省事，实际上要求 codegen 区分两类 cone 的 linkage 策略、要求 RTTI / vtable 合并方案处理两种来源（DLL / EXE 内嵌），增加跨平台测试矩阵

代价是即便 hello world 也会拆出多个 DLL。这个代价在现代部署场景（容器、安装器、应用 bundle）下可以接受。

`[OPEN: 是否提供 release-mode 打包工具把所有 DLL + EXE merge 为单文件？v0 不做。如果未来要做，应作为独立打包工具，不进入 ABI 协议。]`

### 1.3 部署形态

主程序部署是一个 EXE + 一组 DLL（每个 cone 一个），不再是单文件 EXE：

```
myapp/
├── myapp.exe                  # 仅 user_main + plugin 加载逻辑
├── scoop_runtime.dll          # 必有
├── scoop_sysroot.dll          # sysroot 作为单一 cone 的 DLL（[OPEN: sysroot 内部是否多 cone 拆分待定]）
├── myapp_core.dll             # 应用 cone（公开给 plugin）
├── myapp_api.dll              # 应用 cone（公开给 plugin）
├── myapp_internal.dll         # 应用私有 cone —— 同样独立 DLL，不嵌入 EXE
└── plugins/
    ├── plugin_a.dll
    └── plugin_b.dll
```

Linux/macOS 形态同构，只是后缀变 `.so` / `.dylib`。"应用私有 cone 也独立 DLL"是 §1.2 的强制要求，不留豁免。

### 1.4 Cone DLL 的 ABI artifact

每个 cone DLL 编译时**额外**在 DLL 内嵌一个名为 `.scoop_abi`（PE）/ ELF custom section / Mach-O custom section 的元数据节，内容是该 cone 公开声明的 canonical 序列化：

```
.scoop_abi (per cone DLL):
├── cone metadata: (name, version, abi_hash)
├── 编译器 build hash（用于交叉校验）
└── 公开声明的 canonical 序列化:
    ├── 类型（class layout、interface method set、enum variants）
    ├── 函数签名
    ├── object 类型与 singleton ABI 名
    ├── val/var 类型与 ABI 名
    └── 引用的其它 cone 列表
```

#### 1.4.1 用途

- **plugin 编译期**：plugin 作者只需拿到 host 发布的 `cone_a.dll`。`scoopc` 从 `.scoop_abi` 节读取 cone 公开声明，做类型检查与 mangling；不需要 host cone 的源代码、不需要 hand-written stub
- **运行期 plugin 加载校验**：host 计算自己 cone 的 abi_hash，与 plugin descriptor 中 `host_cones[].abi_hash`（来自 plugin 编译期从 `.scoop_abi` 读到的同一份）严格相等校验
- **诊断**：`scoopc abi-dump cone_a.dll` 工具读取并人类可读地输出该节内容

#### 1.4.2 为什么嵌入 DLL 而非独立文件

候选方案是独立的 `cone_a.scoop_abi` 文件与 `cone_a.dll` 并列发布。否决理由：

1. 部署方需要同步管理两个文件，"`.scoop_abi` 与 `.dll` 不一致"是必然会发生的灾难
2. ABI artifact 与 DLL 二进制有强对应（abi_hash 校验绑定），分文件破坏这层契约
3. PE/ELF/Mach-O 都原生支持 custom section；`scoopc` 为 abi_hash 校验本来就需要解析 DLL，多读一节不增加复杂度

#### 1.4.3 工作流示例

```
# 公共 cone 编译
$ scoopc build common_cone --crate-type cone-dll
  → common_cone.dll  (含 .scoop_abi section)

# host 编译，import common_cone
$ scoopc build myapp --crate-type exe --link-cone common_cone.dll
  ↳ 编译器从 common_cone.dll 的 .scoop_abi 读 ABI 做类型检查
  ↳ 链接器把 common_cone.dll 加到 NEEDED / IAT

# plugin 编译，import common_cone
$ scoopc build myplugin --crate-type plugin --link-cone common_cone.dll
  ↳ 同上读 ABI
  ↳ 同上链接 import
```

`[OPEN: .scoop_abi 节的精确二进制格式（schema、版本号、压缩、签名）需要在实现阶段单独写 spec。本节只承诺存在性与用途，不承诺格式细节。]`

## 2. Plugin 入口协议

### 2.1 必须导出的两个符号

每个 plugin 必须以 C ABI 导出且仅导出以下两个固定名字的函数：

```c
// 阶段 A：纯描述。不能执行任何 plugin Scoop 代码、不能分配 GC 对象、
// 不能访问 runtime API。返回一个静态生命周期的 descriptor 指针。
// 返回 0 = 成功；非 0 = 错误码。
extern "C" int32_t scoop_plugin_v0_describe(const ScoopPluginDescriptor** out);

// 阶段 B：完整初始化。在 host 校验 descriptor 后调用。
// host 必须保证当前线程已 thread-init。
// 返回 0 = 成功；非 0 = 错误码（host 应当 dlclose 并报错）。
extern "C" int32_t scoop_plugin_v0_init(const ScoopPluginHost* host);
```

符号名固定为这两个，不参与 STABLE_ID mangling。它们是**协议级**符号，不是 ABI 级符号。

### 2.2 两阶段加载的理由

阶段 A 让主程序在 plugin 任何代码运行前完成兼容性校验：

1. plugin ABI 协议版本（`plugin_abi_version`）
2. runtime 指纹（`runtime_required`）
3. 主程序 cone 列表与 abi_hash 是否匹配（`host_cones`）
4. plugin 提供的 cone 与主程序已加载 cone 是否冲突（`plugin_cones`）

只要任何一项失败，主程序 `dlclose` 走人，plugin 的 cone init 一行没跑。这避免了"plugin 跑了一半 init 才发现 ABI 不匹配，但已经分配了若干 GC 对象、修改了若干全局状态"的状况。

### 2.3 阶段 A 的限制

`scoop_plugin_v0_describe` 实现上**只能返回一个静态生命周期的常量指针**，指向编译期生成的 descriptor 数据段。它**不能**：

- 分配 GC 对象
- 调用 runtime API（包括 `scoop_alloc_typed`、`scoop_thread_*` 等）
- 引用任何主程序符号（除了 plugin 自己 .so 内的全局）

这条限制让 host 可以在 runtime 完全未初始化或处于不安全状态时安全调用 describe。

## 3. ScoopPluginDescriptor

### 3.1 顶层结构（v0）

```c
struct ScoopPluginDescriptor {
    // 协议版本号。v0 = 0。Host 不识别的版本必须拒绝。
    uint32_t plugin_abi_version;

    // 编译器与 runtime 指纹。Host 比较这两个值与自己的指纹是否完全相等。
    // 不相等直接拒绝——v0 不做兼容性范围匹配。
    RuntimeFingerprint runtime_required;

    // plugin 期望主程序提供的 cones。
    // 每个 entry 含 cone 名、版本、abi_hash。
    uint32_t host_cones_count;
    const HostConeReq* host_cones;

    // plugin 自己提供的 cones。仅作元数据/诊断使用，
    // 不直接驱动加载行为。
    uint32_t plugin_cones_count;
    const PluginConeProvision* plugin_cones;

    // plugin 暴露给主程序的入口符号列表。
    // 每个 entry 是一个 effectless 函数的稳定 ABI 名 + 签名描述。
    uint32_t exports_count;
    const PluginExport* exports;
};
```

字段顺序与字段宽度都是 ABI 的一部分，`plugin_abi_version = 0` 之后不允许调整。

### 3.2 RuntimeFingerprint

```c
struct RuntimeFingerprint {
    // GC backend identity。例如 "immix-v1"。
    const char* gc_backend;

    // Effect lowering 协议版本。
    uint32_t effect_protocol_version;

    // Object header 布局版本。变更需要重新编译所有 plugin。
    uint32_t object_header_version;

    // Safepoint 协议版本。
    uint32_t safepoint_protocol_version;

    // 编译器 build hash（git SHA 或等价）。v0 简单粗暴：必须严格相等。
    const char* compiler_build_hash;
};
```

`[OPEN: compiler_build_hash 是否需要弱化为 minor-version-compatible？v0 阶段建议保持严格相等，避免兼容性矩阵爆炸。]`

### 3.3 HostConeReq

```c
struct HostConeReq {
    // Cone 名（如 "myapp.core"）。匹配主程序 cone graph 中的同名 cone。
    const char* name;

    // Cone 版本（来自 Cone.toml）。
    const char* version;

    // 该 cone 在 plugin 编译时对外可见声明的 canonical hash（128 bit）。
    // Host 必须自行计算同一份并对比；不等则拒绝。
    uint8_t abi_hash[16];
};
```

`abi_hash` 的输入是 cone 公开声明（pub 的类型、函数、object、val）经 STABLE_ID canonical encoder 序列化后的 SHA-256，截 128 bit。

`[OPEN: abi_hash 的精确计算规则需要单独写一份附录，至少要明确：是否包含私有但 inline-visible 的内容、是否包含泛型 default body、是否包含 doc 注释。]`

### 3.4 PluginConeProvision

```c
struct PluginConeProvision {
    const char* name;
    const char* version;
    uint8_t abi_hash[16];
};
```

主程序记录这些 cone 的 `(name, version, abi_hash)` 三元组，用于后续诊断与"另一个 plugin 也声称提供同名 cone"的冲突检测。

### 3.5 PluginExport

```c
struct PluginExport {
    // 函数的 readable_path（FQN，仅供调试与诊断）。
    const char* readable_path;

    // @Export 注解指定（或默认推导）的对外名字。
    // Host 通过 dlsym(handle, export_name) 获得函数指针。
    // 详见 §3.7。
    const char* export_name;

    // 函数签名的 canonical hash，用于 host 端编译期类型检查的 sanity check。
    uint8_t signature_hash[16];
};
```

Host 拿到 `export_name` 后通过 `dlsym(handle, export_name)` 拿到函数指针。`export_name` 不是 STABLE_ID 风格的 mangle 名——它是用户在源代码端通过 `@Export` 注解显式控制的稳定名字（详见 §3.7）。这样 host 端 plugin 加载逻辑不需要在运行期参与任何 mangling 计算。

### 3.6 ScoopPluginHost

`scoop_plugin_v0_init` 接收一个 `const ScoopPluginHost*`。v0 阶段这个 struct 主要承担"未来扩展的位置"，当前字段最小化：

```c
struct ScoopPluginHost {
    uint32_t host_abi_version;          // 必须 = plugin 的 plugin_abi_version
    const char* compiler_build_hash;    // host 端的 build hash，做最后一道交叉校验
};
```

不通过 host vtable 注入 runtime API——runtime API 通过 runtime DLL 标准 import 机制提供，详见第 5 节。

### 3.7 `@Export` annotation 协议

> **注**：`@Export` 是一个**跨 crate-type 通用**的语言级 annotation，不是 plugin 专属机制；同样的语义适用于 `--crate-type {staticlib, cdylib}`。本节只描述 plugin 上下文。完整语言规范应进入 `SCOOP_FULL_SPEC.md`，本文档暂为权威参考。

#### 3.7.1 目的

控制函数（v0 仅函数；interface / class / object 暂不支持）在二进制外部符号表中暴露的**名字**。没有这个机制，外部消费者（host 的 plugin loader、C/C++/Rust 调用 staticlib 的代码、其它非 Scoop 工具）只能看到 `__scoop_abi0_fun__<fqn>__h<hash128>` 这种 STABLE_ID mangle 名，无法稳定 dlsym。

#### 3.7.2 语法

```scoop
@Export                          // 默认导出名 = 函数名最后一段
pub fn make_renderer() -> Renderer { ... }
// 在 plugin DLL 的导出符号表中作为 "make_renderer" 出现

@Export("scoop_app_init")        // 显式指定导出名
pub fn init() -> Unit { ... }
// 在 plugin DLL 的导出符号表中作为 "scoop_app_init" 出现
```

`@Export` 仅可标注 `pub fn`。

#### 3.7.3 与 STABLE_ID mangle 名共存：alias

`@Export` 标记的函数有**两个符号**指向同一个函数入口：

- 原始 STABLE_ID mangle 名 `__scoop_abi0_fun__<fqn>__h<hash>`：留给其它 Scoop cone 通过 cone graph 链接调用（消费方是 scoopc）
- 用户指定/默认的 `export_name`：留给非 Scoop 消费方（host 的 plugin loader、C 代码等）

实现层面 ELF/Mach-O 是两个 `T` 类型 symbol 指向同一地址；PE 是两个 export table entry。codegen 上一行 LLVM alias declaration 即可。

#### 3.7.4 名字唯一性约束

**全二进制**内 `@Export` 名字必须唯一——这是 C 全局符号表层面的天然约束，无法回避。编译器需在 link 期前检测同二进制内 `@Export` 名字冲突并报错。

跨二进制（一个 plugin 与一个 host cone DLL 同名 `@Export("foo")`）不构成冲突：plugin 的 `foo` 是 plugin DLL 的导出，host cone DLL 的 `foo` 是该 cone DLL 的导出，dynamic linker 通过 module + symbol 解析。

#### 3.7.5 合法 crate-type

`@Export` 只能出现在 `--crate-type {plugin, staticlib, cdylib}` 的 crate 中。在 `--crate-type cone` 上使用 `@Export` 必须**编译错误**。

理由：cone 是公共库形态，消费方一定是 scoopc 自身（通过 cone graph + STABLE_ID）；公共库作者不应该假定有非 Scoop 消费方在通过 dlsym 调用自己。一旦允许，公共库就开始为想象中的 FFI 用户挑名字、防冲突，而它们根本不是公共 cone 的目标受众。把"是否暴露给非 Scoop 世界"的决策权固定在**最终二进制的拥有者**手里，与公共库作者解耦。

#### 3.7.6 与 PluginExport 的对应

Plugin crate 的编译器扫描所有 `@Export` 标记函数，为每个生成一条 `PluginExport` 条目（§3.5）：

- `readable_path` = 函数的 FQN
- `export_name` = `@Export` 注解指定/默认推导的名字
- `signature_hash` = 函数签名的 STABLE_ID canonical hash

整个 `PluginExport[]` 作为 `ScoopPluginDescriptor.exports` 的内容编入 plugin DLL 的静态数据段。

`[OPEN: @Export 是否需要附加 ABI / calling convention 修饰（如 @Export(c_abi=true)）以支持纯 C ABI 签名？v0 倾向于：plugin–host 契约始终使用 Scoop 调用约定（GC-aware）；纯 C ABI 留给 staticlib / cdylib 场景，等到那两个 crate-type 落地时再设计。]`

## 4. 初始化顺序

### 4.1 主程序自身启动

```
1. C runtime startup（crt0 等）
2. scoop_runtime_init()                        // GC、allocator、thread registry
3. for cone in host.cones (topo sort):
       __scoop_priv0__cone_init__<cone>__h<hash>()
4. scoop_thread_init_current()                 // TLS roots（已存在）
5. user_main()
```

第 3 步对应当前 `MainCodegen::ensure_cone_init_routines_defined`。Plugin 加载发生在 `user_main` 内的某个用户调用点。

### 4.2 Plugin 加载时序

```
[在 user_main 内]
1. handle = dlopen(plugin_path, RTLD_NOW | RTLD_LOCAL)
2. describe = dlsym(handle, "scoop_plugin_v0_describe")
3. err = describe(&desc)
   失败 → dlclose, 报错
4. validate_descriptor(desc):
       - desc->plugin_abi_version 必须 host 已知
       - desc->runtime_required 与 host 当前 runtime 完全相等
       - desc->host_cones 中每条:
           host 必须有同名同版本 cone，且 abi_hash 完全相等
       - desc->plugin_cones 中每条:
           不能与已加载的 cone 冲突（同名异版本拒绝）
   失败 → dlclose, 报错
5. init = dlsym(handle, "scoop_plugin_v0_init")
6. err = init(&host_ctx)
   失败 → 状态可能已被破坏，v0 选择 abort
   成功 → plugin 全部 cone 已 init，可调用 desc->exports 中的符号

[init 内部，由编译器为 plugin 生成]
1. for cone in plugin.cones (topo sort):
       __scoop_priv0__cone_init__<cone>__h<hash>()
2. return 0
```

`[OPEN: 第 6 步失败的语义需要定。abort 简单粗暴；alternative 是要求 plugin init 必须事务化（要么全部成功、要么不留副作用），但这跟"GC 已经分配了对象"很难兼容。倾向于 abort。]`

### 4.3 线程模型

`scoop_plugin_v0_init` 的契约：**调用方（host）必须保证当前线程已经过 `scoop_thread_init_current`**。Plugin init 内部不主动调用它。

Plugin 暴露的 export 函数被 host 调用时，同样的约定继续生效——thread init 是 host/runtime 的责任，不是 plugin 的责任。

`[OPEN: 多线程加载语义。两个线程同时尝试 dlopen 同一个 plugin、或者 plugin init 期间另一个线程已经在跑——v0 简化为"plugin 加载必须串行化，由 host 应用代码保证"。]`

## 5. GC 与 runtime 共享

### 5.1 通过 runtime DLL 导入

`scoop_runtime` 是独立的 dynamic library（见 §1.1），**所有** runtime 符号通过标准 DLL 导入机制提供给 EXE 与 plugin：

- Linux/macOS：plugin .so 与 EXE 都把 `libscoop_runtime.{so,dylib}` 列为 `NEEDED` / load command，dynamic linker 通过常规符号解析绑定
- Windows：plugin .dll 与 EXE 都通过 `scoop_runtime.lib` 的 import library 在链接期完成 IAT 绑定，运行期由 loader 填充

不存在"由 EXE 再导出 runtime 符号给 plugin 反向解析"的路径。EXE 与 plugin 在 runtime 这件事上**地位对等**，都是 runtime DLL 的下游。

可被 plugin 引用的 runtime 符号集合由 `crates/scoop_runtime/src/abi_exports_allowlist.rs` 定义（已存在的机制）。在新模型下它定义的是 runtime DLL **对外导出哪些符号**，而不是 EXE 反向导出哪些符号。集合应至少包括：

- 分配：`scoop_alloc_typed` 等
- 线程：`scoop_thread_init_current`、`scoop_thread_*`
- Safepoint：`scoop_safepoint_poll`
- GC：barrier helper、root scan helper（仅 plugin 必须见的那些）

`[OPEN: allowlist 的具体清单需要在实现时枚举出来，并对每个符号标注稳定性级别（permanent / experimental）。]`

### 5.2 不走 host vtable 注入

不把 runtime API 包成 `host->alloc_typed(...)` 的形式让 plugin 走间接函数指针。理由：

1. runtime 调用频度太高，每次多一次间接调用对性能不利
2. 增加 codegen 复杂度（每个 runtime call 需要不同代码路径）
3. DLL import 已经是 PLT/GOT/IAT 间接，工具链层面成熟，再叠一层 host vtable 是重复开销

### 5.3 GC 共享的运行期约束

- Plugin 分配的对象其 object header 与 host 分配的对象**字段布局完全相同**（由 `runtime_required.object_header_version` 保证）
- Plugin 内的代码与 host 代码使用**同一份** safepoint 实现（由反向符号解析的 `scoop_safepoint_poll` 保证）
- Plugin 内的代码必须由编译器插入与 host 一致的 safepoint poll 与 GC barrier（由编译器版本相同保证）

这三条加起来支持 GC 在 STW 时无差别地暂停 host 与 plugin 代码、扫描全部线程栈、trace 全堆。

## 6. Plugin Boundary Effect Rule

### 6.1 规则

> 任何穿越 plugin–host binary 边界的实体，其静态类型的 effect row 必须为空 `{}`。
>
> "实体"包括：
> 1. plugin 的 `exports` 列表中每一项的函数签名（参数 + 返回值）
> 2. plugin 从 host cone 引用的所有可见符号的签名
> 3. 跨界传递的值上**所有可观察操作**的 effect row
>    （例：跨界传 `() -> Int`，OK；传 `() / IO -> Int`，禁止）
> 4. 跨界传递的对象，其 method 凡可被对侧调用者，effect row 必须为空

这条规则在 plugin 编译期静态拒绝违反者；编译器必须给出明确诊断。

### 6.2 为什么不允许跨界 effect

不是技术上做不到，是性能上不值得。当前 effect lowering 大量依赖"调用方+被调用方+handler 三者静态可见"做 state machine 消除；跨界一旦发生，三者中至少一个变成 opaque，state machine 必须保留完整结构（heap-allocated frame、boundary id、resume entry 等），且无法 inline、无法 fuse、无法 specialize。

效益上看，effectless 边界 + effectful 内部实现已经足够覆盖几乎所有场景：plugin 想暴露 effectful 能力，包成 effectless adapter（例如把 IO effect 在 plugin 内部 handle 掉，对外暴露同步阻塞函数）。这跟 main 边界今天的处理方式天然一致。

### 6.3 用户态实现跨界 effect

如果应用确实需要 RPC 风格的跨界异步交互，可以用 v0 ABI 的现有原语实现：

1. 跨界传递 GC-managed token 对象（effectless 类型）
2. token 对象由发起方注册到自己的 registry，对端持 token 等同于持 opaque handle
3. 完成时对端通过 effectless 回调函数把结果交回，发起方在自己的 registry 中找到对应的 continuation 并 resume

这套流程**完全用户态**，不需要新 ABI、不需要语言特性。如果该模式被验证并广泛使用，再考虑是否进入语言层。在此之前不做。

### 6.4 永远不跨界的形态

即便将来考虑把 boundary effect 内建进语言，**以下永远不允许跨界**：

- multi-shot continuation（多次 resume）
- first-class continuation 作为值跨界
- captured handler reference 跨界

理由：这三者要求对端持有发起方的 frame 布局或 schema 知识，破坏 binary 隔离。

## 7. 共享 TypeDescriptor 与 RTTI

### 7.1 共享类型的 descriptor 必须单实例

`is`、`as`、`as?` 在 class 上目前实现为 descriptor 指针等价比较 + parent 链遍历（见 `crates/scoopc/src/llvm/codegen/main/expr_op.rs:425-439`）。这条路径要求"同一个类型在整个进程内对应同一个 TypeDescriptor 物理实例"。

当前实现中所有 descriptor 都是 `Linkage::Internal`（`crates/scoopc/src/llvm/codegen/gc.rs:1093`），单 LLVM module 内通过 `module.get_global(name)` 去重——但**多 LLVM module / 多二进制的去重未实现**。

Plugin 加载要求做以下改造：

1. 共享 cone（被 host 与 plugin 同时引用的 cone）拥有的类型，其 descriptor 必须 emit 为可在多个 module 间合并的形态（`LinkOnceODR` / `weak_odr` + comdat 是候选方案）。
2. 非定义方（plugin 引用 host 共享 cone 中类型时）只 declare、不 emit。
3. descriptor 内嵌的指针（vtable、itable、bitmap、parent）的 linkage 同步处理。
4. vtable / itable / bitmap 的布局必须严格 canonical，不依赖编译顺序、不依赖"哪些泛型被实例化了"。

`[OPEN: 详细设计见 Task #2（设计共享 TypeDescriptor 合并方案）。该任务依赖 Task #1（审计 RTTI 跨 LLVM module 假设）。本节会在那两个任务完成后回填。]`

### 7.2 Interface 跨界已经免费工作

Interface 的 `is` 走 `runtime_match_type_ids` 数组扫描（i64 type_id 比较，见 `crates/scoopc/src/llvm/codegen/main/numeric.rs`），不依赖 descriptor 指针等价。Plugin 场景下 interface 路径不需要额外工作。

### 7.3 Plugin 私有类型

完全不暴露给 host 的 plugin 内部类型，descriptor 保留 `Linkage::Internal` 即可。Object header 指向 plugin .so 内的 descriptor，GC 通过指针 trace plugin pages。Plugin 不卸载 → 指针永远有效。

## 8. 泛型特化

### 8.1 v0 政策：plugin 自行特化

Plugin 编译期自行生成它需要的所有泛型特化（如 `Vec<HostType>`），即便 host 也用了同样的特化。STABLE_ID 保证两份特化代码的 ABI 符号名一致；这两份代码物理上各占一份空间。

理由：

1. 主程序无法预知 plugin 需要的所有特化集合
2. "host 在编译期发布所有可能特化"不现实
3. 特化代码本身没有跨界 RTTI 问题（function 是 effectless boundary，pure code）

### 8.2 一致性约束

同一个 `(generic, type_args)` 元组在 host 与 plugin 的特化代码必须**语义等价**（行为一致）。这由 STABLE_ID 的 canonical 保证：相同 canonical text → 相同 ABI 符号名 → 相同语义。

`[OPEN: 如果 plugin 特化了一个 host 也特化的泛型，dlopen 后两份代码哪份赢？默认情况下 plugin 用自己的 .so 内副本（RTLD_LOCAL），host 用自己 exe 内副本。两份代码内容在等价语义下功能一致。但 inline cache、function pointer 比较等场景下可能暴露"两个函数指针不等"的现象。需要在 Task #1 审计中明确受影响范围。]`

## 9. 错误处理与失败模式

### 9.1 加载阶段失败

| 失败点 | 处理 |
|---|---|
| `dlopen` 返回 NULL | 返回错误给调用方，不影响主程序状态 |
| `dlsym("scoop_plugin_v0_describe")` 失败 | `dlclose`，报错"非 Scoop plugin 或协议版本不识别" |
| `describe()` 返回非 0 | `dlclose`，报错（plugin 可能想表达内部错误） |
| descriptor 校验失败 | `dlclose`，报错（含具体哪一项不匹配） |
| `dlsym("scoop_plugin_v0_init")` 失败 | `dlclose`，报错（理论上 describe 通过则 init 必存在，但兜底处理） |
| `init()` 返回非 0 | v0：`abort`。状态可能已部分污染。 |

### 9.2 运行期失败

Plugin 加载成功后，调用 plugin export 函数失败的处理与调用同进程任何其它函数失败完全相同——按 Scoop 错误传播规则处理（effect 边界 / 异常 / Result 等，依语言决定）。Plugin 加载这一动作不引入新的运行期失败模式。

`[OPEN: panic 行为。Plugin 内 panic 是否能被 host catch？v0 倾向于"是"——panic 对应 effect-style propagation，跨二进制边界仍按正常 effect 边界处理。但如果 panic 实现走 unwinding，跨二进制 unwind 在某些平台不稳定，需要在实现时核实。]`

## 10. 兼容性与版本演进

### 10.1 v0 的兼容性承诺

**没有**。v0 是设计草案，任何一处都可能在 v1 之前调整。具体来说：

- ScoopPluginDescriptor 字段可增删
- RuntimeFingerprint 可改
- 入口符号名可改
- Plugin Boundary Effect Rule 不会放松，但表达细节可改

### 10.2 v1 升级路径（前瞻）

可能的 v1 改进（不承诺会做）：

1. **跨界 effect**：在收集足够用户态实现经验后，考虑把 token-style RPC 协议提升到语言/ABI 层。
2. **Unload 支持**：需要先解决 GC 持有 plugin 内 descriptor / 函数指针的悬空问题。
3. **跨编译器版本兼容**：建立 ABI 兼容性矩阵。
4. **多 plugin 互操作**：plugin A 提供的 cone 被 plugin B import。需要扩展 cone 解析。

每条都是独立大工程，不与 v0 落地耦合。

## 11. Open Questions 汇总

便于追踪：

1. **`runtime_required.compiler_build_hash` 的弱化策略**（第 3.2 节）
2. **`abi_hash` 的精确计算规则附录**（第 3.3 节）
3. **`init()` 失败的事务化语义**（第 4.2 节）
4. **多线程加载的并发约束**（第 4.3 节）
5. **runtime ABI allowlist 的具体清单与稳定性标注**（第 5.1 节）
6. **共享 TypeDescriptor 合并方案的详细设计**（第 7.1 节，依赖 Task #1 / #2）
7. **泛型特化在 plugin / host 双副本下的 inline cache / 函数指针等价行为**（第 8.2 节）
8. **跨二进制 panic / unwind 行为**（第 9.2 节）
9. **`.scoop_abi` 节的精确二进制格式 spec**（第 1.4 节）
10. **`@Export` 是否需要附加 ABI / calling convention 修饰**（第 3.7 节）
11. **sysroot 内部是否多 cone 拆分**（第 1.3 节）

每一项落实后回填到对应章节，并在本汇总中标记完成。

## 12. 与其他文档的关系

- **`SCOOP_FULL_SPEC.md`**：
  - 本文档不修改语言语义，只新增"plugin 边界"作为一个 ABI 概念。Plugin Boundary Effect Rule（§6）作为一项 ABI 约束被语言规范引用。
  - `@Export` annotation（§3.7）是跨 crate-type 的语言级 annotation，权威 spec 应进入 `SCOOP_FULL_SPEC.md`，本文档暂为 placeholder。
- **`SCOOP_RUNTIME.md`**：runtime 必须新增对 `abi_exports_allowlist` 的稳定性承诺。
- **`docs/archive/designs/STABLE_ID.md`**：本文档完全复用其 mangling 与 hash 协议，不引入新 mangling。`@Export` 提供的 alias 名与 STABLE_ID mangle 名共存（§3.7.3），不替代后者。
- **`AGENTS.md`**：plugin 编译命令、`--crate-type {plugin, cone-dll, staticlib, cdylib}` 等 CLI 选项需要在该文档登记（实现阶段）。
- **`.scoop_abi` 二进制格式 spec**：§1.4 描述了存在性与用途，精确格式应单独成文（v0 落地阶段产出）。

## 13. 实施前置条件

按依赖顺序：

1. **Task #3**（二 LLVM module 复现实验）：钉死当前 RTTI 跨 module 失效现状。
2. **Task #1**（审计 RTTI 跨 LLVM module 假设）：列出所有受影响的 codegen 路径。
3. **Task #2**（设计共享 TypeDescriptor 合并方案）：解决第 7 节 Open Question。
4. 本文档基于 Task #2 结果回填第 7 节，转为 v0 正式版。
5. **Codegen：每 cone 独立 LLVM module → DLL**。当前 codegen 假设单一 LLVM module，需扩展为按 cone 切分、各自产 DLL，linkage 策略相应调整。
6. **`@Export` annotation 落地**（语言前端 + codegen）：解析、验证 crate-type、生成 LLVM alias、检测同二进制名字冲突。Spec 草稿进入 `SCOOP_FULL_SPEC.md`。
7. **`.scoop_abi` 节落地**：codegen 在 cone DLL 中嵌入元数据节；`scoopc` 增加读取与 abi-dump 子命令；abi_hash 校验链路打通。
8. **CLI 与 plugin 入口**：`--crate-type plugin`、plugin 入口 stub codegen（`scoop_plugin_v0_describe` / `scoop_plugin_v0_init`）、descriptor 静态数据生成、plugin cone init 链接策略。
9. **Runtime 侧**：`abi_exports_allowlist.rs` 落实并加 test。
10. **End-to-end fixture**：一个最小 plugin + host 的 fixture，覆盖加载、调用、共享类型 RTTI、GC 跨界 trace、`@Export` alias 解析。

直到第 4 步完成前，本文档保持 Draft 状态。
