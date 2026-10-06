# M23-11 设计：公开 CLI、单文件模式与总验收

状态：已完成并验收（2026-10-03）。实际交付、源码覆盖、分批迁移、清理及最终无筛选结果见 [验收记录](ACCEPTANCE.md)。

前置条件：[M23-10 已完成验收](../stage10/ACCEPTANCE.md)，设计核对基线为 `0b180dde6`。沿用 [M23-6a](../stage6a/ACCEPTANCE.md) 的共同 HIR、[Stage 7](../stage7/ACCEPTANCE.md) 的机器定义与 ODR、[Stage 8](../stage8/ACCEPTANCE.md) 的多 image 启动，以及 [Stage 9](../stage9/ACCEPTANCE.md)／Stage 10 的正式产物链接。

权威约束：[语言规范](../../specs/SCOOP-SPEC.md) 12.2～12.6、[实现规范](../../specs/SCOOP-IMPL-SPEC.md) 2.7～2.8、[runtime 规范](../../specs/SCOOP-RUNTIME-SPEC.md) 2.8。本文细化并同步修订 [M23 总设计](../DESIGN.md) 1.3、5.5、8、9.1、9.6、10。CLI 只负责普通编译、构建与进程操作，不增加语言特性、来源授权、通用资源预算或重复语义验证。

## 1. 交付范围与实际基线

### 1.1 用户可用的结果

```sh
scoop build
scoop run -- first "two words"
scoop build --release
scoop run --profile debug --target-dir out
scoop build hello.scoop
scoop build app/Cone.toml -o out/app
scoop build libraries/math -o out/math.slib
scoop run hello.scoop -- first "two words"
scoop build ffi.scoop --library-path native -o out/ffi
scoop link --root-slib out/app.slib --dependency-slib out/core.slib \
  --runtime-objects runtime/index.cbor --target aarch64-apple-darwin -o out/program
```

`build` 完成从源码图到 `.slib`，并在 root 为 executable 时继续完成 runtime 输入和链接。`build/run` 省略 root 时使用当前目录的 `Cone.toml`，默认 profile 为 debug，输出位于 `target/<canonical-target-triple>/<profile>`。`run` 共用构建与稳定输出发布，成功后启动本次程序。`link` 从已经存在的产物完成链接，不要求 Scoop 编译器或源码存在。

正式 `scoopc build` 继续一次产生一个 Cone 的 `.slib`；低层 `scoop-link` 继续接收显式闭合的产物集合。二者的独立进程测试保留。公开 `scoop` 使用既有库入口与配套 compiler 进程，不通过再次启动 `scoop-link` 包装同一库，也不调用 `scoopc` library。

本阶段只支持既有 Darwin/AArch64、完整静态 Cone 图和一个主 executable。final-link cache、并行 Cone 调度、包管理／安装器、watch／daemon、C/C++ 源码编译接口、新的 runtime ABI 及 M24/M26/M27 功能不在范围内。

### 1.2 设计时的基线与缺口

下表保留 `0b180dde6` 设计基线的现状与实施分工；最终实现和迁移结果以验收记录为准。

| 位置 | 基线事实 | 本阶段工作 |
| --- | --- | --- |
| [`scoop/src/lib.rs`](../../../compiler/scoop/src/lib.rs)、[`schedule.rs`](../../../compiler/scoop/src/schedule.rs) | 已有发现、DAG、快照、缓存和串行 child 调度；没有 `scoop` bin | 增加薄 CLI，将既有输出接到物化和执行 |
| [`scoop/src/request.rs`](../../../compiler/scoop/src/request.rs)、[`manifest/src/single_file.rs`](../../../compiler/manifest/src/single_file.rs) | 已有 manifest／single-file 请求和唯一源码身份 | 补省略 root 的当前 manifest、构建 profile 与输出目录配置；复用已有分类与读取 |
| [`scoop/src/program_link.rs`](../../../compiler/scoop/src/program_link.rs)、[`runtime_build.rs`](../../../compiler/scoop/src/runtime_build.rs) | 可从保留的 artifact bytes 链接，也能构建／缓存 runtime | 接通 build/run；显式 link 复用独立 Link reader 和 runtime index |
| [`driver/src/main.rs`](../../../compiler/driver/src/main.rs) | 已是单 Cone `.slib` CLI，旧 executable CLI 已删除 | 保留正式入口，补齐一致的配置、诊断和观察输出 |
| [`driver/src/child_protocol.rs`](../../../compiler/driver/src/child_protocol.rs) | machine 请求只接受 structured + `emit=None`；错误被压成字符串，warning/note 的 origin 被丢弃 | 传递实际诊断来源；一次编译返回多个文件 dump |
| [`scoop/src/snapshot/prepared/model/child_request.rs`](../../../compiler/scoop/src/snapshot/prepared/model/child_request.rs)、[`child.rs`](../../../compiler/scoop/src/child.rs) | 固定请求 `emit=None`，成功响应要求 dump 集合为空 | 把明确观察请求传给指定源码节点，检查返回文件集合 |
| [`driver/src/request/report.rs`](../../../compiler/driver/src/request/report.rs) | 当前诊断上下文仅保存显示路径／源码文本；成功结果至多一个 dump | 保留 canonical source 映射与完整阶段结果，发布后不丢失错误位置 |
| [`linker/src/link.rs`](../../../compiler/linker/src/link.rs) | 一次链接完成检查和原子输出，返回实际 plan fingerprint | 发布到 target/triple/profile 路径，保留 fingerprint 作为内容结果；复用已检查 binary，不重复链接 |
| [`scoop/tests/program_link`](../../../compiler/scoop/tests/program_link) | Stage 9/10 已有真实产物、native、最终检查和运行测试 | 将 fixture 编排与断言迁成统一 Python 数据规则，为公开命令补齐用户入口覆盖 |

旧 `compiler/driver/tests/fixtures.rs`、`warnings_cli.rs`、`inputs.rs`、`linking.rs` 已在 `c02bd245c` 删除。不能将本阶段描述为“只修改旧 runner 的调用函数”，也不能恢复原 core/source 拼接和直链实现。

当前 M1–M22、M25 目录共有 **534 份 `.scoop`**：**480 份**旧合并快照带有 `== ast ==` 或 `== diagnostics ==`（186 成功、294 诊断），另有 1 份同后缀的阶段诊断快照和 53 份没有合并快照的阶段 fixture。另保留 M0 smoke。上述数字是文件盘点，不是通过数；Stage 10 记录的 5437 项全仓通过不能代替已删除 runner 的历史端到端覆盖。

## 2. 公开命令与配置

### 2.1 命令表面

```text
scoop build [<root-input>] [--target <triple>] [--sysroot <dir>]
            [--profile <debug|release> | --release] [--target-dir <dir>]
            [--scoopc <file>] [--cache-dir <dir>] [--cone-path <dir> ...]
            [--runtime-root <dir> | --runtime-objects <index>]
            [--library-path <dir> ...] [-o <output>]
            [--emit <ast|hir|mir|lir|all> --dump-dir <dir>]
            [--dump-scope <root|sources>] [--message-format <human|json>]

scoop run [<root-input>] [同 build 的构建／链接／profile／target-dir 配置]
          [--message-format <human|json>] [-- <program-arg> ...]

scoop link --root-slib <root.slib> [--dependency-slib <dep.slib> ...]
           [--cone-path <dir> ...] [--sysroot <dir>] [--target <triple>]
           --runtime-objects <index> [--library-path <dir> ...]
           -o <output> [--dump-plan] [--message-format <human|json>]
```

`build/run` 的 root-input 可省略，此时等价于显式指定当前目录的 `Cone.toml`。`run` 不接受 `-o` 或 stage dump 选项；自定义最终文件名或观察 IR 使用 `build`，选择输出根目录则两者均可用 `--target-dir`。`--dump-scope` 只能与 `--emit` 一起使用，`--emit` 与 `--dump-dir` 成对出现。`--dump-plan` 是链接后的普通说明输出，不增加另一种计划执行模式。

公开 `link` 使用 `--dependency-slib`；既有低层 `scoop-link --dep-slib` 保持，不增加同义参数。公开 build/run 不接受 `--direct-slib`／`--support-slib`。不添加 raw object、archive、`-l`、任意 linker flags、脚本或自动 native source 发现参数。

所有 operand／配置 path 用 `PathBuf`，program args 用 `OsString`。先按调用者 cwd 解析相对 locator，随后保留原显示拼法与实际 I/O 路径；不能先转 UTF-8 字符串再解析 shell。`--` 后的 `-o`、空串、空格和非 UTF-8 参数均属于程序参数。

### 2.2 默认配置与配套工具

| 配置 | 决定方式 |
| --- | --- |
| root input | 显式 operand；省略时取调用者 cwd 下的 `Cone.toml`，不向父目录搜索 |
| target | 显式 `--target`，省略时由现有 registry 解析当前支持的 host；不支持的 target 在 compiler／runtime／LLVM 工作前报错 |
| 构建 profile | `--profile debug`／`--profile release`，或 release 的简写 `--release`；省略时 debug，两种选择方式不能同时出现 |
| target root | 显式 `--target-dir` 相对调用者 cwd 解析；否则 manifest 为 `<Cone-root>/target`、single-file 为 `<cwd>/target`；均追加 canonical target triple 和构建 profile |
| 配套 compiler | 显式 `--scoopc`，否则取实际 `scoop` executable 同目录的 `scoopc`；随后使用现有 machine capability 与 executable digest 检查 |
| sysroot | `--sysroot` → `SCOOP_SYSROOT` → 当前开发发行的默认 sysroot |
| runtime source root | `--runtime-root` → 当前开发发行的默认 `runtime/`；显式 `--runtime-objects` 时不读取这些源码 |
| cache root | `--cache-dir` → `SCOOP_CACHE_DIR` → 当前 Darwin 用户的 `$HOME/Library/Caches/Scoop`；没有可用 HOME 时要求显式路径 |
| 诊断格式 | 默认 human；json 使用第 5 节的逐行结构化记录，均写 stderr |

当前开发发行默认 sysroot/runtime 与仓库已有 driver 约定一致，由 `compiler/toolchain` 集中提供构建时的工作区路径。该选择只是 locator；移动二进制时可显式提供位置。M23 不顺带实现安装目录探测、包下载或发行安装器。默认 compiler 通过 `current_exe` 所在目录取得，不扫描 cwd、项目父目录或 PATH。`scoopc` 的直接命令应使用同一 sysroot/target 解析规则，允许显式 `--target`、`--sysroot`；它仍不拥有 cache、runtime 或 native library search 配置。

`build/run` 在完整显式依赖发现后仍无 core 节点时，优先使用 sysroot 的 `lib/scoop.core` 源码目录；仅当目录不存在时读取 `artifacts/<canonical-target-id>/scoop.core.slib`。源码目录存在但无效、dangling symlink 或选中产物无效都直接失败，不改选另一来源。选中的来源沿共有 source／prebuilt 路径处理；构建不写 sysroot，开发者修改现有 core 源码仍走普通缓存失效。直接 `scoopc` 与显式 `link` 只消费产物。

`build/run` 使用完整 `ResolvedTargetProfile`，各消费者只取得自己的 projection。显式 `link` 只解析 LIR target 和 final-link projection，不构造完整 build request，不执行 compiler handshake，不调用 `llvm-config`。JSON、dump 与终端显示选择都不改变 target 或产物内容。

环境分工保持：compiler 和链接工具使用既有明确命令／toolchain 环境；被运行程序继承调用者环境。程序的 `SCOOP_GC_STRESS_MOVE` 等 runtime 配置不得被误当作 compiler flags；`CC`、`LDFLAGS`、`LIBRARY_PATH` 不成为额外链接输入。

### 2.3 debug/release 的当前职责

以 `BuildProfile::{Debug,Release}` 表示本次构建选择，进入 umbrella 的请求、输出布局和结果展示；它与已有 `ResolvedTargetProfile` 的 target/ABI/toolchain projection、`.slib` artifact profile 是不同概念。CLI 解析后总有明确 profile，不在后续阶段反复猜默认值。除 debug/release 外的名称报参数错误；`--release` 与任何显式 `--profile` 同时出现也报参数错误。

本阶段两者均使用现有 Scoop codegen、generated-C、runtime-build 和 final-link 设置，只预留 profile 选择及以后映射编译设置的位置。release 目前不代表已开启优化，debug 也不承诺新增调试信息；不因 profile 名称切换 GC、检查或异常语义，不增加 manifest 自定义 profile、继承关系或任意优化参数。

缓存以实际生效的生产配置为依据，当前 debug/release 可以复用同一编译产物和 runtime 缓存，用户输出目录仍彼此独立。将来实施优化时，先定义实际编译设置，再传给对应 producer 并纳入其缓存键／代码内容规则；当前不增加未被消费的 child 字段、IR 字段或另一套编译流程。

## 3. 唯一构建与链接流程

### 3.1 Root 分类和构建

`scoop build/run` 在没有 operand 时，先取 `<调用者 cwd>/Cone.toml`，随后与显式传入该文件共用分类和读取。即使当前目录只有一个 `.scoop`，也不能自动改成 single-file；当前 manifest 缺失、错类型、symlink 损坏或内容无效时正常报错，不向父目录查找。`run -- <args>` 中 root 同样省略，分隔符后的内容全部为程序参数。省略语法只属于 umbrella CLI，进入 `BuildRootInput` 时已经是明确的 manifest 分支；`scoopc build` 仍要求 operand。

显式输入的分类规则复用 `scoop-manifest`：已存在目录对应目录内 `Cone.toml`；basename 精确为 `Cone.toml` 的 regular file 对应 manifest；basename 扩展名精确为 `.scoop`、解析 symlink 后为 regular file 的 operand 对应 single-file。坏类型、缺失、dangling/cycle symlink 或其他扩展名明确失败。抽取 driver 当前分类中的可共享部分到 manifest crate，两个 CLI 使用同一个规则，不引入对 driver implementation 的依赖。

single-file 固定为 `scoop:single-file:0.0.0`、Executable、logical `main.scoop`、唯一指定源码和 core direct edge。相邻 `Cone.toml`、其他 Scoop/C/C++ 源码、archive 和 blob 都不读取。build/run 的 single-file 分支拒绝 `--cone-path` 这类额外 Cone locator 配置；native `--library-path` 仍合法，只由已有 typed requirement 使用。

正常流程为：

1. 补默认 root 并分类，解析明确的 target、构建 profile、target root 和显示映射；`run` 在读到 library root kind 后立即报错。
2. 复用 `load_root → discover → resolve`，在首个编译 child 前完成 exact 图、locator 歧义、版本、kind 和环检查。manifest core root 不加载另一份默认 core；普通 core 显式 locator 继续按已实现规则处理。
3. 复用 `prepare` 的不可变源码、manifest 和 artifact 快照，按 canonical ready-set Kahn 顺序 `execute`。普通 cache miss 每个源码节点只调用一次配套 compiler；direct/support 来自完成的真实依赖。
4. Library root 从完成结果的 artifact bytes／已发布 cache entry 物化 `.slib`，结束。它不构建 runtime 或 native provider，也不要求 `main`。
5. Executable root 先完成自己的 `.slib`，再取得 runtime 输入。直接把保留的 artifact 快照交给 `link_built_program`，不得因临时路径消失而重新读原 source/prebuilt locator。
6. 同一个 program-link 读取机器数据、解析 Stage 10 native 输入、产生正式 startup、执行最终检查；按第 6 节返回或运行 binary。

构建结果以 Library／Executable 封闭分支表示，保存实际 root、完整 dependency artifact 清单、构建 profile 和实际输出位置；Executable 另须携带发布的 binary、runtime index 和 link-plan fingerprint，不能用缺失 binary 的“成功”分支表示 compile-only。新编译产物的 locator 指向已发布 cache entry 或用户物化文件，prebuilt 保留本次实际定位的文件；不能泄露即将随 `PreparedBuildGraph` 销毁的 staging 路径。清单使调用者可以向独立 link 显式提供闭包，不需要反查内部 cache record。

上游失败后立即停止 dependent 和链接，已完成独立节点的 cache entry 可以保留。返回该次失败及此前已产生的 warning；不把后续未执行节点合成为成功，不继续运行已有输出。

### 3.2 Runtime 与 native 输入

runtime 输入为实际封闭选择：

- `BuildSources { root }`：复用 `build_runtime`，按明确 source/header、compiler、flags、ABI 和规则读入／构建／缓存。
- `Objects { index }`：复用 `RuntimeObjectSet::read_index`，检查目标与内容，不读 runtime source。显式损坏 index 报错，不能暗中切到 source build。

`--runtime-root` 与 `--runtime-objects` 互斥。未指定时 build/run 选择默认源码 root；library 不消费 runtime 输入。既有 runtime cache 损坏按其已实现策略 miss 重建，和显式 index 错误区分。

native 输入继续来自 `.slib` 的完整声明合同／实际引用及 `--library-path`。直接 object、archive、dylib/framework、TLS、C/Scoop ABI、callback 和动态绑定全部复用 Stage 10。改变 native locator 或内容只影响实际链接，不能让它进入 `scoopc` 名称决议或改变 single-file 的 Cone 图。

### 3.3 显式 artifact-only link

`scoop link` 必须给出 runtime index 与 `-o`。`build` 的结构化结果提供实际 root／dependency artifact 清单和 runtime index 路径；调用者也可以提供自行保存的同类产物。这一要求使独立链接不依赖“上一次构建”的隐式全局状态。

先读 root 和全部显式 dependency 的不可变 bytes／manifest summary。显式 dependency 按 identity 建索引；相同完整 artifact 的重复 locator 合并，同 identity 不同内容失败。然后只沿 root 的 exact dependency 表补齐闭包：优先已有显式记录，否则按 `--cone-path/<group>/<name>/<version>/cone.slib` 查询；core 仍可使用已有 sysroot target-qualified artifact 位置作为默认 locator。显式候选不匹配时报告错误，不用另一来源掩盖。

此处只复用普通 artifact 候选查找／歧义规则，不调用 source graph discovery，也不跟随 source manifest、core source 或 cache receipt 寻找重建办法。缺失、stale、多个版本、不可达 extra、错误 kind／ABI／target 由原 closure reader 报告。图中只有一个 executable root；single-file artifact 在 dependency 位置一律拒绝。

single-file root 的 core 匹配使用 edge 中实际记录的 HIR/MIR/LIR semantic fingerprints 与 ABI；不能要求一个并未记录的旧 byte-exact core artifact。语义相同而 Code 改变的 core 可以重链，新 Code／artifact 内容进入当前 plan。

完成定位后，从同一批 bytes 构造一次 `ProgramLinkClosure`，与 runtime 对象及 native search roots 调用已有 `link_program`。低层 `scoop-link` 仍要求调用者显式提供闭包，不获得 locator、source build 或 cache 调度能力。

## 4. 缓存和检查边界

真实 native 函数值与泛型再次发布的组合要求最终引用核对沿实际 ODR winner 进行。link map 保留 Scoop 对象的 Cone／member 归属；重复 atom 的唯一保留记录必须属于既有候选且与最终符号地址一致。最终 verifier 使用 winner 的原 relocation，避免把被合并候选的本地 C trampoline 地址套用到保留正文。该修正复用现有 ODR 兼容检查，不修改 callback 身份、产物格式或 runtime ABI（实现规范 2.8）。

### 4.1 继续使用现有编译键与锁

保留 `ConeCompileCacheKeyV1` 的真实输入：manifest／single-file semantic projection、canonical source identity 与内容、全部 direct 依赖三层语义 fingerprint、compiler executable／协议、identity/schema/产物 profile、target/backend 和实际 C bridge toolchain。当前 single-file key 还保留 core Code fingerprint，允许其造成保守重编译；本阶段不混入一次新的增量依赖裁剪设计。

`-o`、`--target-dir`、cwd 的 locator 拼法、临时目录、诊断格式、dump 目录／范围、program argv、mtime 和 native search roots 不进入编译键。省略 root 与显式当前 `Cone.toml` 使用相同的语义输入；debug/release 当前映射到相同的实际生产配置，构建 profile 名称自身也不另造编译键。runtime／native／final-link 内容由各自已有输入和 plan 表达。manifest 依赖 object-only 变化而三层语义保持时，下游编译可命中，最终链接仍使用新对象；single-file core Code 的现有保守规则单独测试。

复用现有 per-key 锁、不可变 entry 和原子目录发布。普通命中返回缓存 warning 与 artifact；显式 dump 请求的例外见第 5.3 节。精确编译 key 下损坏的现有 entry 继续报告现有 corruption 错误，不自动覆盖；不改变为“任意 reader 错误都静默重建”。相同 key 的新生产结果不同仍是 nondeterministic production 错误。

### 4.2 每个边界完成自己的检查

| 边界 | 必需检查 | 直接复用的数据 |
| --- | --- | --- |
| 父进程发现／调度 | manifest、envelope/hash、coordinate、profile、edge、cache key／record | 已读 artifact bytes 与 summary |
| compiler 消费依赖 | 本次实际使用的完整 Compile 数据、类型／引用／ABI／对象 | 读入后未变化的 typed 数据 |
| compiler 生产与发布 | 完整 IR、机器定义、对象及本次写入 bytes | 同次 pipeline 的结果，发布不整图回读 |
| child 完成 | response id／退出状态、输出归档与计划、warning／dump 文件 | compiler 的结构化结果与父进程既有快照 |
| program-link | 完整 Link closure、实际 native 解析、ODR 与链接新事实 | 原对象／canonical ABI／定义与引用检查 |
| runtime | 已加载地址、registration、精确 stackmap、初始化和 GC | 正式 startup 指向的原 image/root |

父进程标为 `CacheHit`／`Compiled` 只描述调度结果，不声称持有完整 Compile/Link world。总设计旧有“每次 cache hit 先重放两个完整 view”的测试条款随本设计修正；损坏 IR／对象仍必须在实际消费者失败。无需为此制造新的验证凭证、第二份元数据或所有阶段通用的验证状态机。

本阶段不建立最终 binary cache。每次 executable build/run/link 都完成一次实际链接；不能仅因默认输出路径已经存在就跳过 native 输入读取或 final verifier。

## 5. 结构化诊断、子进程与 dump

### 5.1 保存诊断产生时的来源

parser/HIR 的现有诊断已经具有 file/span 和 note；driver 应在丢弃源码上下文前把这些索引映射成 `SourceIdentity` 与真实 byte span。拓展当前 report 的 source 映射，不先调用 `error.to_string()` 再解析其中的 `source 0` 或路径文本。

依赖 reader 失败时，driver 在释放已加载输入表之前按实际 artifact slot／provider 关联 locator，并保留原 semantic field／member。父进程借本次依赖快照关系映射回原 locator，结构化通道不把它降级为无位置 tool error；关联只读取已有数据，不重新解码产物。

每条 primary／note 独立使用实际来源：

- 当前源码或依赖定义：已有 `SemanticSourceSpan { cone, logical_path, span }`。依赖来源使用已有 definition-source 记录，不冒充调用者；optional 源码文本缺失时仍能展示 coordinate、logical path 和 byte span。
- manifest／CLI I/O：实际 host path；仅在存在对应文本区间时附带真实 span，不为无法读取的文件伪造 `0..0`；此类 locator 留在当次展示，不写入语义快照／缓存 warning。
- 产物损坏：artifact locator 加原有 semantic field/member 位置，不伪造成 HIR source error。
- 真正没有源码位置的 tool／internal error 才使用 `None`；无效或悬空 source index 是内部错误，不能悄悄降级为无位置诊断。

success warning、编译 failure 和发布 failure 共用这份转换。已完成上游的 warning 不能因下游失败而丢失。稳定排序复用 graph phase/order、Cone coordinate、canonical source/span 或 member；不按本机 staging path 或 HashMap 次序排序。

父进程持有本次 original locator 与快照的映射。cold build 和 cache hit 的同一 canonical warning 内容相同，显示路径按本次调用重新附加。对依赖中的原定义保留原 Cone；不能将所有导入 note 重写成 root 的 `main.scoop`。

### 5.2 面向用户的输出

human 默认将错误、warning 和最终产物位置写 stderr；成功 build/link 的 stdout 为空。`run` 的 stdout 及 stdin 始终属于程序，构建日志不混入；程序运行期间 stderr 同样直接继承。

`--message-format json` 在 stderr 输出逐行 JSON，使用 schema 1 的少量明确记录：diagnostic 和成功的 library/executable/link 结果。diagnostic 保留 severity、code、message、canonical origin、独立 notes，显示 locator 另列；成功分支提供实际 output、root 和 dependency artifact 清单，build/run 结果另显示所选构建 profile，executable 另有 runtime index 与 link-plan fingerprint。清单中的 coordinate／identity 标识产物，path 仅供后续 I/O。无需为每个内部步骤创建 event 总线或保存执行 transcript。

`run` 在启动程序前完成工具 JSON 的输出，此后直接继承程序 stdio；程序自己的 stderr 仍是原始字节，不承诺整个组合 stderr 都是 JSON，也不把它包装成工具记录。run 结果报告与 build 相同的稳定输出路径，运行结束后仍然有效；内部为本次执行保留的私有副本不作为用户产物路径返回。

JSON 是现有 typed result 的展示格式，不是新的 child/build 协议；复用仓库已有 serde/serde_json。语义 golden 只截取 canonical 字段，CLI presentation golden 单独规范化显示路径。文本内容和多字节字符不能被截断来满足固定诊断长度。

逐字段展示格式见 [CLI-JSON.md](CLI-JSON.md)。成功记录中的一次性 `observations` 汇总直接展示现有 scheduler 的节点来源和 child 调用顺序，供 cold/warm fixture 核对；不产生逐步骤事件或持久化 transcript。

### 5.3 一次编译取得全部请求的观察结果

`--emit all` 表示 AST、HIR、MIR、LIR 四项；其余值只请求对应项。`root` scope 只观察 root；`sources` scope 观察完整图中所有源码节点，包括实际从源码构建的 core，不对 prebuilt 请求其未保存的 AST／LocalConcrete。

每项内容来自同次真实 pipeline：AST 为按 source identity 排序的当前源码；HIR 分别显示共同 Export 与 LocalConcrete，保留两者边界；追加的 CrossCone 区域直接显示同次生产的共享接口、source constructor 合同和导入 application，LocalConcrete 显示实际 shape-support roots。canonical 表和 arena 分别沿自身的稳定顺序输出，不为观察重新生产或验证元数据。MIR/LIR 为本次生产的完整模块。跨 Cone 身份改变可以导致受控 golden 更新，不能靠删掉 imports、origin 或 generated identity 让旧输出看起来不变。

LIR 模块中的 `deps` 是本地 arena 引用；含外部初始化依赖的单元还追加 `init-dependencies`，记录同次 production 的 canonical 单元与带 provider 的完整依赖序列。该输出保留跨 Cone 委托初始化的实际关系，不要求测试重读内部产物，也不新增观察命令或持久字段。

父进程为每个观察节点在已有私有 staging 中分配一个空目录，child 把请求的文本写成固定 `ast.txt`／`hir.txt`／`mir.txt`／`lir.txt`。成功后按原阶段顺序返回实际文件 descriptor。父进程检查它们恰好覆盖请求、路径匹配、文件类型／内容 digest 正确，再输出到 `<dump-dir>/<完整 ConeIdentity>/<stage>.txt`；同一次编译中相同 stage 不产生两个文件。

默认缓存命中时没有新 child。显式观察的源码节点即使命中也运行一次完整 compiler，复用同一 source/dependency 快照和正常 cache key，捕获全部所需 stage 后正常生产 artifact。缓存条目相同则复用，差异走现有 nondeterminism 诊断；不能为了四份 dump 编译四次，也不能从 `.slib` 反造没有序列化的 LocalConcrete。dump 不持久化到编译 cache，不成为 `.slib` member，不影响 Code 或 semantic fingerprints。

编译失败只返回真实诊断；未完成阶段不补空 dump。成功节点的 dump 可以作为观察文件保留，但图失败不得发出整个 build 成功结果或执行程序。dump 写入／读回失败属于该观察请求失败，不能报告已满足观察请求。

### 5.4 协议 2 的最小变更

继续一进程、一请求、一响应，length-prefixed canonical CBOR，经 stdin/stdout 传输。machine stdout 只包含 response frame；dump 内容只通过上述文件传输，正常 machine stderr 为空。signal、坏 frame、错误 request id、成功 exit 配失败 response 等仍由现有 transport 检查。

协议 envelope version 升到 **2**，同步 request/response schema digest 与配套 machine capability；不接受旧协议 1 的隐式回退。基本 path、diagnostic origin、digest carrier 和 stage tag 继续复用，stage tag 保持 Ast=1、Hir=2、Mir=3、Lir=4。

请求原八字段结构保持；field 8 的 dump policy 为封闭 sum：

| tag | 字段 | 含义 |
| --- | --- | --- |
| 1 | `{0=1}` | None |
| 3 | `{0=3, 1=stages, 2=directory}` | 非空、按 stage tag 严格递增的集合及明确输出目录 |

旧单 Stage tag 2 退役不复用。该四种 stage 的有限集合是协议实际功能，不是通用计算／内存配额。machine 模式只返回已有 File destination；Stdout destination 不能与 response frame 复用同一流。success 沿原 artifact／fingerprint／warning 字段增加到最多四项唯一 descriptor 的实际支持，failure 继续只携带非空且至少一条 error 的诊断集合，不附成功 artifact。

父进程取消“dump 必为空”的旧断言，按本次请求检查集合；driver 取消 `emit=None` 限制以及以无位置总字符串覆盖语言诊断的逻辑。直接 `scoopc --emit` 的 human 文本输出可继续保留，但与 machine 文件输出都消费同次 stage capture 实现。

协议 2 已进入既有 compile key，所以旧 child 结果和旧 cache key 不命中。缓存 record 的字段结构没有变化时仍用现有 record schema；不要仅因名字中有 V1 就增加第二份平行 receipt。`.slib` outer schema、persistent identity、native contract、runtime metadata ABI 3、runtime index schema 1、link-plan v2 均保持。若实际共同 HIR 修复改变持久数据，再按其真实 section 单独版本化。

## 6. 输出物化与程序执行

### 6.1 默认输出的完整规则

先确定 target root：manifest 模式默认为 `<Cone-root>/target`，single-file 默认为 `<调用者 cwd>/target`；显式 `--target-dir <dir>` 相对调用者 cwd 解析并替换该根。以下 `<output-dir>` 均为 `<target-root>/<canonical-target-triple>/<profile>`，profile 取 debug/release，默认 debug；即使没有 `--target`，也保留 registry 解析出的完整 host triple，例如 `aarch64-apple-darwin`。

| 命令／root | 默认结果 |
| --- | --- |
| `build` manifest library | `<output-dir>/<cone-name>.slib` |
| `build` manifest executable | `<output-dir>/<cone-name>` |
| `build` single-file | `<output-dir>/<sanitized-input-stem>` |
| `run` manifest／single-file | 与对应 build 相同的稳定 executable，成功发布后执行本次构建结果 |
| `link` | 必需的 `-o` |

例如当前 Cone 名为 `app` 时，`scoop build` 产生 `target/aarch64-apple-darwin/debug/app`，`scoop run --release` 产生并运行 `target/aarch64-apple-darwin/release/app`；`scoop build hello.scoop --target-dir out --profile release` 产生 `<cwd>/out/aarch64-apple-darwin/release/hello`。输出路径不再包含 fingerprint；完整 artifact／link-plan fingerprint 继续保存在既有内容记录与内部缓存中，不用用户可变文件充当缓存条目。

single-file stem 按用户 operand 的 basename 去掉 `.scoop`，保留 ASCII 字母／数字／点／下划线／连字符，其他字节替换为下划线；空名、`.`、`..` 使用 `program`。这只是输出文件名，不参与 Cone/source/cache/link identity。

默认目的位置在 root、target 和构建 profile 解析后即可确定。在目的文件系统的私有临时位置完成一次链接，取得 `ProgramLinkOutput` 后将同一已检查 binary 原子发布；返回的 fingerprint 用于结果记录，不再决定输出目录。可抽出既有最小原子发布函数供低层链接与 umbrella 使用；不增加预链接、第二次 final verifier 或仅为发布创建的新证明对象。

build 的显式 `-o` 相对调用者 cwd，完整覆盖最终 root 文件路径，不在其后追加 triple/profile。它可与 `--target-dir` 同用，最终 root 文件以 `-o` 为准；内部缓存仍由 `--cache-dir` 决定，显式 `--dump-dir` 也不受 target root 影响。`run` 不接受 `-o`。默认目录和显式输出都用实际目的位置的锁／临时文件与原子 rename 发布。沿用 fs4、tempfile 和现有普通文件工具。最终路径的锁只覆盖物化，不串行整个全图或程序执行。

library 从不可变 artifact bytes 复制到目的临时文件并同步后 rename，不通过 hardlink 让用户修改 `.slib` 同时损坏共享 cache。可执行权限由 binary 物化保留。输入源码、manifest、当前实际 artifact/runtime/native 文件及 cache 内文件不能被默认或显式输出覆盖；复用真实路径与已打开文件身份进行 I/O alias 检查，父目录 symlink 与已有 hardlink 也覆盖。目录、无法写入和 rename 失败保留旧结果。

不同 target／profile 的输出目录分开；同一目录下同名源码或同名 Cone 的多次成功构建按发布顺序替换稳定文件，失败保留上次成功结果。需要同时保留时使用不同 `--target-dir` 或 `-o`。并发发布只能看到完整文件，不能把“输出已经存在”当作缓存命中或因此跳过本次链接。

`run` 在发布前保留同次已检查 binary 的私有可执行副本，成功发布稳定结果后执行该副本，避免另一次构建覆盖稳定路径而运行错程序。副本放在稳定 binary 的同一目录，保持 `@executable_path` 等相对装载语义，argv[0] 使用稳定 binary 路径；发布不与可修改的稳定文件共享副本 inode。结束后只清理该私有副本，稳定产物保留。

### 6.2 `run` 的进程语义

程序启动时使用第 6.1 节保留的绝对 executable path，argv[0] 为稳定输出的绝对路径；其余参数是 `--` 后原始 `OsString` argv，并保留调用者原 cwd 与完整 environment。语言入口仍是原 `main(): Unit`；本阶段不增加 argv 核心库 API。测试可以用正常 `@Extern`／native companion 读取进程参数与环境。

工具自身的结果和 exit 规则为：成功 build/link 为 0，构建／链接／I/O／启动失败为 1，CLI 参数错误为 2。程序成功启动后，它的正常 exit code 原样传递；程序由 signal 终止时保留 Unix signal 结果，不能统一映射为 1 或仅返回 `128 + signal` 的普通退出。

`run` 的父进程保留私有执行副本直至程序退出，清理该副本及本次其他临时文件后使用平台进程接口传播状态，稳定输出不随清理删除。终端 Ctrl-C／SIGTERM 不得让 compiler、linker 或程序遗留在后台；使用当前前台进程关系和必要的局部 signal 处理／转发，收割已启动子进程，不实现新 job-control 框架。signal 传播以实际 `ExitStatus` 测试，不能只断言“不成功”。

启动失败和运行后失败区分：缺 binary／执行权限等属于 tool error；native `exit(37)`、`raise` 或未捕获 Scoop 异常属于程序结果。一次失败 build 即使 `-o` 已有旧程序也不能运行它。正常 `build/link` 不执行用户程序来决定是否允许发布。

## 7. Python fixture infra、历史覆盖恢复与旧路径清理

### 7.1 测试归属与覆盖盘点

将现有基于 Rust 的文件 fixture 基础设施迁到 `tests/fixture_runner/`，统一入口为 `python3 tests/run_fixtures.py`；不再新增 Rust `fixtures.rs` runner。Python 负责发现用例、解析 acceptance criteria、准备环境、执行工具、比较结果和维护快照。语言端到端用例直接调用实际 `scoop build/run/link`，single-file 走 `scoop build <file>`；低层工具专项使用其既有工具入口。测试准备明确配套 `scoop`／`scoopc`／`scoop-link`，使用私有 sysroot/cache/output；不得通过 `scoopc::compile_file`、Python binding 或内部 pipeline library 回到旧编译单元。

首先盘点第 1.2 节的历史源码，以及现有各 crate 中负责读取文件、构建／执行和比较结果的 Rust fixture harness，包括 Stage 9/10 的 program-link/native 用例。把原有期望、快照、工具入口、native companion 和 stress 要求迁成第 7.2 节的声明；覆盖清单从这些声明和源码扫描结果生成，不在 Python 中维护另一份逐 case 注册表：

- 480 份原合并式端到端 fixture 和 M0 smoke 全部接到公开 SingleFile 构建；成功／诊断／trap 的期望先取原已接受快照与明确 directive，不能由本次结果反推。
- 其余阶段 fixture 的文件加载、工具执行和 golden 比较同样迁到 Python，保留原 parser/HIR/MIR/LIR 的结构断言，并补语言规则的正式 CLI 独立及组合覆盖。能够独立编译的源文件直接执行，缺少入口的正例在 fixture 源文件中明确加入普通 `main` 或配套真实程序，不由 runner 合成隐藏声明／第二份 synthetic manifest。直接构造内存中 typed IR、验证算法或内部不变量的 Rust 单元测试继续留在原 crate，不承担另一套文件 fixture 编排。
- 新 fixture 必须明确声明各步骤的成功、诊断或 trap；未知／冲突 expectation 是测试配置错误。发现规则自动收集声明，每份源码必须属于测试项或明确列出的支持文件，支持文件不单独计作通过；没有声明／归属的文件使完整扫描失败，不能因没有 `.scoop.snap` 或不认识目录就静默略过。

清单只用于管理现有测试，不是编译产物、发布许可或新的通用测试插件系统。通过数必须区分实际 CLI 用例、阶段 golden、Rust 单元测试和 native/runtime 用例，并列出同一 fixture 的运行变体；不能用 workspace 总数取代迁移覆盖。

### 7.2 统一 acceptance criteria 与扩展规则

采用一套版本化 TOML schema 描述测试条件和测试方式，两种载体进入相同的解析、校验与执行流程：

- 简单单文件用例可在源文件开头的 `// fixture:begin`／`// fixture:end` 注释块内写 TOML，每行保留 `//` 前缀。runner 只读取这些注释，不删除或改写交给编译器的源码，诊断位置仍对应实际文件。
- 单文件的较长描述可放在相邻 `<name>.fixture.toml`；多文件／多 Cone 用例用目录中的 `fixture.toml`，显式列出 root、输入与支持文件。每个测试项选择一种载体，禁止内联与附加文件各保存一份可冲突的条件；相对路径统一相对于描述所在目录解析，长诊断／dump／输出可引用独立期望文件。

schema 的首版字段围绕当前实际测试需要组织，不把不同里程碑做成不同 runner：

| 数据 | 统一规则与当前用途 |
| --- | --- |
| 输入与执行条件 | 源码／manifest root、支持文件、工具和 target 要求、tags；缺少必需工具是环境失败，明确不适用的 target 单独报告，不能计作通过 |
| 命名步骤 | 有序的进程执行和必要文件操作；命令使用 argv 数组并声明 cwd、env、stdin，路径／参数／输入可表达文本和必要的原始 bytes；后一步可引用前一步已观测的 JSON 结果或产物路径；同一规则覆盖 build、run、link、native compiler 和低层工具 |
| 变体与状态 | 具名参数／环境变体，例如普通／moving-GC stress；cold→warm→修改输入→重建用有序步骤共享同一私有工作区和 cache，不按 fixture 名实现特殊状态机 |
| 进程结果 | 明确正常 exit code 或 signal，stdout/stderr 的文本或原始 bytes、结构化诊断的 severity/code/message/source/span/notes；negative 必须声明实际失败条件，不能仅断言非零退出 |
| 文件与跨步骤断言 | 文件存在／缺失、类型、内容包含／不包含、摘要或字节相等／不等、JSON 字段比较、阶段 dump／link plan golden；可比较不同 checkout、构建和变体的实际结果 |
| 期望与归一化 | 短值内联、长值引用文件；只使用具名的公共归一化规则处理已知临时路径等非语义差异，不按 case 删除诊断、改写类型身份或吞掉额外输出 |

进程执行、文件准备和断言分别复用公共实现；native companion 的源码、逻辑库名、编译参数与库形态均为数据，复用同一工具执行机制。复制／移动／删除／精确替换或字节损坏等文件步骤只为现有 cache、corruption、源码移走和可重复构建测试服务。原 core 成员／整数相等组合用例将多个原文件组成同一个 single-file 输入，使用 `concat(path,parts)` 按声明顺序拼接文件、文本或十六进制字节；分隔符必须显式给出，先读完各部分再写目标，不插入、删除或解析 Scoop 代码。该动作复用已有字节载体和引用规则，不在 Python 中重写 Scoop 语义、IR reader 或 linker。Rust 内部 typed 数据的精细不变量仍由普通单元测试负责。

每个步骤声明其预期终止结果和需要观察的输出；按声明的完整诊断集合检查额外／缺失诊断，不能把预期 HIR 错误替换成任意 I/O 或 linker 失败。未知字段、嵌套值类型错误、无法解析的步骤引用、互斥条件、缺失期望文件和未覆盖输入均在执行前报告配置错误。引用按当前变体的可用变量检查，后台进程完成前只有 PID 可用；独立诊断 JSON 文件中的引用也在执行前检查。归档／cache 截断测试使用显式 `truncate(path,size)`，只能缩短现有文件，越界不修改文件。过滤、列表和完整运行共用相同的发现规则，完整验收不得通过过滤或隐式 skip 隐藏失败；报告区分配置／环境错误、编译／链接失败、断言失败和明确未运行项。

新增 fixture 原则上只提交源码、TOML 条件和期望文件，自动进入统一发现与执行，不修改 Python 源码、不添加按文件名／目录／里程碑判断的分支，也不为单个 case 附带承担编排或断言的 Python／shell 脚本。已有规则确实无法表达一个新的实际测试能力时，才扩展可复用的公共动作、断言或 schema 字段，并补对应独立／组合测试与格式文档；不得用任意 Python predicate、动态插件或逐 case callback 规避数据规则。普通 C/Scoop 测试程序和被测行为的 native helper 不属于 runner 扩展。

首版以 Python 3.11+ 的 `tomllib`、`subprocess`、`pathlib`、`tempfile` 等标准库完成已有能力；模块按发现／schema、执行、断言／快照和报告划分。交付 fixture 编写说明与成功、negative、多 Cone、native、stress、cold/warm 的数据示例。通过“仅增加 fixture 数据、Python 源码无变更”的新增用例验收扩展能力；infra 自身只针对解析、组合、匹配和进程状态等公共规则测试，不复制语言语义测试。

大量历史用例按 fixture 并行执行，`--jobs` 控制同时运行的用例数，默认最多四个；同一用例的步骤和变体仍严格有序。各用例独立工作目录，只共享本次私有 sysroot/cache，并由正常构建锁协调缓存。报告按发现顺序保存，不受完成顺序影响。用户中断时停止尚未执行的步骤、清理并收割已启动的进程；未完成项明确计为 interrupted，不计作通过。

### 7.3 快照与运行保持

2026-10-07 修订：普通功能 fixture 删除与被测行为无关的整份产物、core 和 native 输入固定摘要，以及失去用途的 golden 和辅助步骤。本目录中旧验收记录的指纹数量只记录当时执行范围，不构成以后继续锁定具体哈希的要求。缓存命中／失效、确定性、读写及源码构建与 artifact-only 消费一致性仍比较同次产生的实际值；只有输入严格受控的编码／指纹专门测试保留固定摘要。结构快照的内容摘要归一化限定明确字段，并以稳定编号保留相等和引用关系；不得隐藏 typed identity、符号、布局、顺序或其他结构差异，也不得掩盖同等条件下的非确定性。

正例通过 `--emit all --dump-dir ...` 从一次当前 root 编译取得四阶段 dump，再执行同次生成的 binary，保留 warning、stdout/stderr、exit 或 trap 覆盖。negative 从结构化诊断核对 canonical source/span、code、message 和 notes；不能让“任意非零退出”满足原语言错误期望。

Python 从正式 CLI 的 JSON 记录、dump 文件和真实进程结果取得这些信息，阶段 golden 只比较当前编译实际产生的阶段输出，不启动第二套前端。原 Rust 代码中的断言和旧 directive 一次迁成相同 schema，不保留另一套长期解释器。快照更新是显式开发命令，只更新被选择的期望文件并生成可审阅 diff，不自动改写成功／失败分类或接受新诊断；正常运行和最终验收始终只读期望。

原快照中的 core AST 拼接、local arena 编号和 host file path 不再是正确结果；按实际共同 IR 更新，并记录变化原因。HIR 显示 Export／LocalConcrete 的边界，外来声明仍是原 typed identity。跨 Cone 暴露出的合法程序失败必须修复共同语义／物化／机器引用，不改为 expected diagnostics，也不删除用例。

Stage 10 已记录参数自由 imported enum 的派生 `==` 候选缺口。总验收补本地／依赖／re-export 的同规则用例并沿共同 HIR 和真实 provider helper 处理；不能保留本地专用比较捷径或以 native 测试绕过。如果发现实际持久表示不足，先改对应 spec，再修 section 与生产／消费。

保留原 moving-GC 清单并扩展实际组合；stress 与普通运行的可观察输出应相同。旧 runner 对 `__LLVM_STACKMAPS` 的物理段名断言按现行 Stage 9 的 `__DATA_CONST,__llvm_stackmaps` 和既有 final verifier 更新，不能为保留旧断言恢复错误段布局。

### 7.4 Native companion 与低层专项

fixture 描述显式列出真实 `native.c`／`native-*.c` 到逻辑库名的映射。Python runner 用明确 C compiler／SDK／deployment 预构建 `fixture_native.o` 或 `libfixture_native.a` 等 Stage 10 可识别候选，再仅传 `--library-path`；不在 Python 中维护 native case 清单或专用构建分支。CLI 不读取相邻 C 文件；某个库没有 typed requirement 时，其存在不能把未声明对象自动加入链接。

包含 C/Scoop ABI、native roots、global/TLS、外来线程、callback、协程／异常和 managed 返回值的旧 companion 按当前 ABI 编译，复用 Stage 10 支持。确有历史测试源码不符合已修订 ABI 时直接修复测试源码和预期，不能注入 raw object 或增加测试专用 runtime 符号豁免。

compiler 单 Cone 生产、reader 损坏、codegen 对象和 runtime 加载边界的普通内部单元测试继续保留；其中依靠文件 fixture 的进程／快照编排迁到 Python，不能以“单元测试”改名保留旧 harness。已有 Stage 6/7 仅为可执行验收服务的临时 link/startup 拼装调用迁到正式 program-link，删除只为该旁路服务的 main/image 合并与默认工具调用。runtime 中定点构造错误 metadata 的 C 单元测试继续测试其真实加载边界，不必改成语言 CLI 用例；需要外部构建和运行的部分共用 Python 的声明式步骤。

### 7.5 迁移完成后的清理

迁移按原用例及断言建立对应关系，先确认 Python 路径覆盖相同的语言规则、诊断位置、阶段 golden、运行结果和 native／stress 变体，再删除对应 Rust fixture infra。迁移期间可用两者临时对照，但 Stage 11 完成时只能保留一套 fixture 执行与验收规则：

- 删除原 Rust fixture runner、用例注册／目录特判、directive／期望解析器、fixture 专用构建／执行／快照 helper，以及仅服务这些代码的测试模块和不再使用的依赖；不能把旧 infra 留作 fallback。
- 删除随迁移失去调用者的临时 link/startup 路径，更新测试命令、脚本及 CI 配置中实际存在的入口。直接调用 Python runner，不增加仅为转调 Python 而保留的 Rust runner。
- 保留原 fixture 源码和仍有价值的期望，将明确的历史语义／ABI 修订逐项记入迁移记录；清理的是重复 infra，不能通过删除正例、缩减断言或把失败改为预期来完成迁移。

最终报告记录旧测试入口到新声明的覆盖对应、用例与变体数量、未运行项及删除清单。`cargo test` 只代表剩余 Rust 测试；完整仓库验收必须同时运行 Python fixture suite，不能继续把 cargo 总数作为历史 fixture 全量通过的证据。

## 8. 新增 fixture 与验收矩阵

新增测试目录按实际职责组织为 `m23-cli`、`m23-single-file`、`m23-cli-diagnostics`、`m23-cli-cache` 和 `m23-cli-combined`；不在每个目录复制一套构建器。manifest 多 Cone fixture 提供真实 `Cone.toml`；single-file 保持原唯一文件。

### 8.1 命令、输入与输出

| 范围 | 必测结果 |
| --- | --- |
| 基本构建 | 无 main 的 library、manifest executable、single-file；默认输出、`-o`、warm cache；手工按序 `scoopc` 与 umbrella 的 `.slib` bytes 相同 |
| 省略 root | build/run 默认当前 `Cone.toml`，与显式 `.`／`Cone.toml` 的图、产物和输出目录一致；仅父目录有 manifest、当前仅有单个 `.scoop`、当前 manifest 缺失／无效时失败；`run -- <args>` 正确分流，library run 报错 |
| Root 分类 | 目录／`Cone.toml`／`.scoop`、相对／绝对／symlink、非 UTF-8 locator；缺失、错误扩展名、非 regular、dangling／cycle；源码 identity 不受拼法影响，显式 source 不受 cwd 的 manifest 影响 |
| 构建 profile | 默认 debug、显式 debug/release、`--release` 等价；未知值、两种选择方式并用时报参数错误；目录隔离，当前 `.slib` bytes／程序结果相同，不增加优化或 runtime 行为差异 |
| 输出目录 | 默认 `target/<canonical-target-triple>/<profile>`，host 默认也含 triple；manifest 相对 Cone root、single-file 相对 cwd；`--target-dir` 的相对／绝对路径、与 `-o` 组合，改变位置不改变编译输入 |
| 单文件隔离 | 同目录放坏 manifest、额外 main、C/C++、archive、blob 仍只读指定源码；非 core import／额外 Cone 参数失败；typed FFI 加 library path 成功 |
| core | 默认 source 与缓存、仅有目标平台产物的 sysroot、源码优先且错误不回退、显式普通 core locator、直接构建修改后的 core、core root 在默认 sysroot 不存在时成功；冲突走普通图诊断 |
| 输出与原子性 | 同名不同内容／并发发布完整文件，后成功者替换；不同 triple/profile 隔离；已有输出、只读目录、rename 失败、source／artifact／cache alias、跨文件系统候选复制；失败保留旧输出 |
| run | library 早失败；argv 的空串／空格／`--`／非 UTF-8、cwd/env、stdin/stdout/stderr；exit 0／37、SIGTERM／SIGABRT／Ctrl-C、启动失败、私有文件清理与无孤儿 child；稳定输出保留，并发覆盖不改变本次执行，argv[0] 和相对动态库装载符合稳定路径 |
| 独立 link | 移走 Scoop/runtime 源码和 compiler，仅保留 slib／runtime index／native；explicit/search-root 闭包、single-file root；缺失／stale／extra／executable dependency 与歧义失败 |

argv/cwd/env 的运行用例使用普通 native helper 观察真实进程数据；signal 用 Python 父测试进程读取 POSIX 子进程状态（`subprocess` 的负 `returncode`），明确区分 signal 与普通 exit code。不以 mock argv 列表代替实际 `scoop run`。

### 8.2 缓存、协议与诊断

| 范围 | 必测结果 |
| --- | --- |
| 调度 | chain／diamond、动态 ready-set、cycle／多版本／歧义在首个 child 前失败，上游失败不启动 dependent/runtime/link |
| Compile key | 源码、语义／默认值／泛型 hidden helper、compiler／协议／target／C bridge 改变的实际失效；未变节点零 child；object-only 与 single-file core Code 的各自规则 |
| 不影响编译键 | 省略 root 与显式当前 manifest、当前等效的 debug/release、locator／mtime／`--target-dir`／`-o`／diagnostic／dump／argv/native search root 改变；普通切换可复用已有编译产物，native 内容仍进入新 link plan，不能沿用旧 binary |
| cache 文件 | 竞争同 key、不同生产结果、截断／错 record／错内容、失败不发布；完整 typed IR／object 损坏在实际消费者拒绝 |
| 协议 | v1/mismatch、坏 frame／trailing bytes／request id／exit、missing output、错误 artifact 摘要；长诊断完整传递，无任意 message quota |
| dump | root／sources 范围、None／单项／四项；cache hit 显式观察每节点仅一次 child；missing／额外／重复／错 stage／路径／digest 失败；不污染 machine 或 program stdout |
| 诊断 | parser 多错误、HIR primary/note、跨 Cone 默认值／hidden 定义、warning 与后续 failure、cold/hit 一致；两 checkout 的 canonical JSON 一致，显示 path 独立 |

普通调度零 child 的 Rust 单元测试复用 `BuildObservations` 和内存中的 recording runner；Python fixture 另执行配套生产进程。前者只检查调度／故障时序，不负责文件 fixture 的发现、构建或快照，不将 fake IR 或 fake binary 算入语言／链接完成门。

### 8.3 多 Cone 组合与总回归

至少一组真实 source graph 经过公开 `build/run` 完成以下组合：facade re-export 上游 generic/default → 下游本地 value/ref type 具体化 → interface dispatch → 两个 sibling 的相同 ODR member 与独立 adapter member 并集 → generic delegated extension → eager/lazy globals → moving GC、closure／coroutine、异常 finally 和 native callback。

同时保留普通数值结果之外的必要断言：相同实例 TD/storage/cell 地址合并、不同 exact type 不同址、lazy/failure 共享、initializer 先于 main 的顺序、跨 FFI 的引用移动／publication 与线程退出。公开 CLI 不能只用 hello world 代表 Stage 7–10 的组合已接通。

6a 的声明位置变化、内存／wire 等价与双向泛型用例通过正式源码／产物入口再覆盖；Rust 内部单元测试和 Python 阶段 golden 继续锁定各自结构。重建 core 后移走源码再消费，确认普通声明和实际正文仍来自该产物。两个不同 checkout/output 根、随机源码创建／依赖输入顺序比较 `.slib` bitwise、canonical diagnostics、dump 和实际 link plan；允许系统 executable 非语义字段按既有规则归一化，不能放宽 slib 比较。

## 9. 实施顺序与完成门

### 9.1 分批实现

1. 以本文和三份 spec 为基线，盘点历史 fixture、Rust harness 与原有断言，建立统一 TOML schema 和 Python 的公共发现／执行／断言入口；补诊断来源转换、一次多阶段 capture 和协议 2 的正负／wire 测试，后续新增文件 fixture 直接使用这套入口。
2. 在现有 scoop 请求／调度中接入观察策略、warning 传播和结果生命周期；复用 cache 与 child，完成 cold/hit 和故障矩阵。
3. 新增薄 `scoop` bin 与命令模块，集中默认 toolchain locator，接通省略 root 的当前 manifest、显式 manifest/single-file、debug/release 选择、target/triple/profile 输出与原子物化；两种 profile 先沿用当前编译设置。
4. 接通 `run` 的稳定输出及同次私有执行副本、stdio/argv/status，以及 artifact-only `link` 的定位／runtime index；用 Python fixture 补无 compiler／无源码的真实进程测试。
5. 把历史及现有 Rust fixture harness 的全部条件、步骤和断言迁成源码注释／附加 TOML，恢复全部历史端到端和阶段 golden 覆盖；审阅诊断与 IR 快照，修复正式产物路径暴露的共同实现缺口。核对覆盖后删除原 Rust fixture infra、专用依赖／入口及旧临时 link/startup helper，同阶段完成恢复和清理。
6. 用仅添加数据的方式补多 Cone、native、corruption、cache、reproducibility 和 moving-GC 组合，验证不需要逐 case 修改 Python；关闭全部更新开关，分别运行 Rust 测试与完整 Python fixture suite，另写实际 `ACCEPTANCE.md`。

代码按 `cli`、build、run、artifact-link、presentation／dump／materialization 的真实职责拆分；不把整个实现塞进 `main.rs`。复用 clap、serde/serde_json、tempfile、fs4 和现有 toolchain／manifest／protocol，不引入无实际用途的工厂、插件或泛化调度层。

### 9.2 必须同时满足的完成条件

- 公开 build/run/link 三条命令完整可用，single-file 只包含唯一 source + core，library 无 main，executable 始终先有完整 `.slib`。
- build/run 无 operand 时正确使用当前 `Cone.toml`；debug/release 两种 profile、默认 debug、`--release`、`--target-dir` 与 `target/<canonical-target-triple>/<profile>` 布局完整可用。当前两种 profile 使用相同实际编译配置，优化仍留待后续实现。
- 每个正常 source miss 一次配套 child，正常 hit 零 child；显式 dump 仅对指定源码节点一次编译取得实际输出。diagnostic primary／note 和 cache warning 保留 canonical 来源。
- 显式 link 不依赖源码、LLVM 或 Scoop compiler；全部入口共用 Stage 9/10 reader、native resolver、startup 和 final verifier，runtime ABI 3 保持。
- 默认路径、`-o`、原子发布、run 稳定产物保留与本次执行隔离、argv/cwd/env/stdio、程序 exit/signal、启动失败和取消清理通过真实进程测试。
- 历史端到端 fixture 覆盖已恢复；阶段 fixture 的归属和 CLI 规则覆盖已逐项记录；没有通过删正例、改期望或临时拼接 core 掩盖错误。
- 所有文件 fixture 共用 Python infra 和一套 acceptance schema；简单内联／复杂附加描述使用相同规则，新增代表性用例只改数据即可运行。原 Rust fixture runner、专用 helper、入口与不再使用的依赖已清理，无 case-by-case 分支或长期双轨。
- 6a 与 Stage 7–10 的适用矩阵、完整 corruption／reproducibility／cache 与 GC/exception/closure/coroutine/FFI 组合通过；旧临时 executable 路径和重复检查条款不再是依赖。
- Rust／Python 的格式化、lint、配套工具构建、infra 公共规则测试和关闭快照更新的全仓测试通过，有逐项覆盖与真实运行证据；本文的文件盘点或旧验收计数不能代替本阶段记录。

每批代码变更先格式化和 lint，再运行对应测试。Python 使用 Ruff 的统一 format/check 配置；实施时确认工具的维护状态并固定开发工具版本。最终命令以明确 LLVM/toolchain 和配套 executable 位置执行，Python runner 的 `--all` 表示不带筛选的完整发现和运行：

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets
ruff format --check tests/fixture_runner tests/run_fixtures.py
ruff check tests/fixture_runner tests/run_fixtures.py
cargo build -p scoop -p scoopc -p scoop-linker --bins
export SCOOP_TEST_PAIRED_SCOOP="$PWD/target/debug/scoop"
export SCOOP_TEST_PAIRED_SCOOPC="$PWD/target/debug/scoopc"
export SCOOP_TEST_PAIRED_SCOOP_LINK="$PWD/target/debug/scoop-link"
python3 -m unittest discover -s tests/fixture_runner/tests
python3 tests/run_fixtures.py --all
cargo test -p scoop --tests
cargo test --workspace --no-fail-fast
```

最终验收先清除全部 `SCOOP_UPDATE_*`、`SCOOP_*SNAPSHOT*`、`INSTA_UPDATE`、`INSTA_FORCE_PASS`、`INSTA_ACCEPT_UNSEEN`、`UPDATE_EXPECT` 与 `RUST_MIN_STACK`，Python 也不得启用快照更新；需要 stress 的程序按 fixture 声明仅对子进程设置。工具缺失或当前必需目标不可用必须在验收入口失败，不能因 `Option`／环境变量缺失提前 return 后仍计作通过。环境准备、工具版本、实际用例／变体数、未运行项、快照变更原因、Rust infra 删除清单与结果写入验收记录，只有全部必需工作完成后才更新里程碑状态。
