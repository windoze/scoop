# M23-8 实施记录

状态：实施中。M23-7 验收提交为 `d548a07a1`；设计基线提交为 `3cd6c95a4`。本记录只报告已经实施和运行的内容，完整完成门仍见 [设计](DESIGN.md)。

## 1. root gateway 异常生命周期

- root gateway 在 `BeginCatch` 后通过现有 `MaterializeException` 生成托管对象，写入专用 failure root 后才 `EndCatch`。新增 managed call 使用完整 safepoint identity 与 root plan，native payload 不再离开 catch。
- root lowering 按职责拆入 `production/root.rs`；专门检查物化输入的根、独立输出、failure slot、入口 poll 与封闭失败返回值。同步 29 处既有 LIR golden，保留原函数语义。
- `cargo fmt --all`、`cargo clippy --workspace --all-targets` 通过。LIR lowering 143 项、codegen/runtime 302 项、slib 584 项全部通过；driver 的实际重建 core 与产物消费组合测试通过（包含 9 份受影响 golden）。测试时未启用 snapshot 自动更新。
- 首次 `cargo clean` 删除 1184 个文件，释放 2.8 GiB。验证使用 `target/m23-8`，opt-level 1、无 debug symbols、关闭增量，保留 debug assertions 与溢出检查。
- 多 image startup 尚未接通；root/eager gateway 完整结构拒绝检查和实际 startup 异常报告随后续功能验收。

## 2. 单一初始化 registration 与格式迁移

- 删除旧 88-byte coordinator 的 struct、symbol、Strong/ODR role、definition atom、digest node、外来物理引用。初始化 runtime API、生成的 ensure 和跨 Cone 引用统一使用 352-byte `ScoopInitializationUnitDescriptorV1` registration，cell 仍为 16 bytes。
- 诊断 atom 归属 registration，物理 reader 与 definition/registration fingerprints 同步；循环诊断按 byte span 长度读取，测试覆盖带尾随数据的非 NUL 结束路径。
- metadata prefix ABI 升至 3；generic profile `/2`、cone production `/4`、layout link closure `/4`、link identity closure `/8`、Scoop object verifier `/4`。registration wire 保留 24 个字段及原编号；退休 tag/field、旧 profile 与旧 ABI 有拒绝用例。同步实际格式向量，未改变 source/exact/unit/body 身份公式。
- 将初始化发射的声明检查与测试模块构造拆为子模块，删除只服务旧 coordinator 的 target 推导分支。
- 格式化与全 workspace/all-targets Clippy 通过。identity 335、LIR 468、slib 584、codegen/runtime 302 项通过；最后清理后的 11 项初始化 reader 测试通过；实际 core 重建与产物消费组合测试通过（40.40 秒），未开启自动更新 golden。
- 已清理闲置的默认 target 构建缓存；本功能提交后清理专用 `target/m23-8`。旧单 image 启动调度仍待新 registry/startup 接通后删除；生产中只保留一种初始化 record 格式。

## 3. 已加载映像地址范围与只读元数据

- Mach-O adapter 在读取 header/load commands 前检查 VM 可读区间，收集实际映射的权限、检查 section 完整范围并提供显式释放接口。范围查询覆盖相邻区间、权限变化、空洞、对齐和整数溢出。
- 编译器将含地址修正的不可变元数据放入 `__DATA_CONST`；最终链接将 LLVM stackmap section 归入同一段。dyld 完成地址修正后，runtime 按实际 VM 权限要求两者只读。实机用含函数地址重定位的 stackmap 验证该约定；直接改变原段权限会触发 linker text-relocation 错误，因此设计和链接参数已同步采用 section 迁移。
- 格式化与全 workspace/all-targets Clippy 通过；codegen/runtime 303 项、toolchain 9 项通过；实际 core 重建与产物消费组合测试通过（39.71 秒），未开启自动更新 golden。

## 4. 多 image 收集、身份与依赖闭包

- 增加按六种 record kind 分开的 ID/地址索引；所有 prefix、表和 byte span 先检查完整只读范围，再读取内容。ODR 只接受同址合并，Strong 重复 producer 拒绝；同组独立 helper member 取并集，并检查跨 kind 的 group/member 冲突。
- 在完整 image 集合上检查 root 可达闭包、重复坐标/版本、缺失或重复 dependency、自环和环。Kahn 每次重新选取当前 ready set 的最小坐标，返回唯一 image 顺序。
- C 测试使用带不可读 guard page 的只读映射，覆盖六类相同 ID bytes 的独立命名空间、ODR helper 并集、反向输入枚举和 23 种损坏情况。动态 ready-order 用例明确区分逐次 Kahn 和按层排序。
- 格式化与全 workspace/all-targets Clippy 通过；codegen/runtime 304 项、toolchain 9 项通过。收集接口尚未发布给 GC；类型、静态根和初始化关系将基于此完整集合继续解析。

## 5. 类型、callable、safepoint 与 scan 地址关系

- 在完整 typed 表上建立 TD、callable entry 和 64-bit site ID 的反向索引，拒绝不同身份共址、runtime type/site ID 碰撞、错误 owner producer 和缺失 fingerprint。
- TD 先检查 144-byte 固定部分及完整参数尾部，再解析 parent、interface、function type 和已知 itable 范围。继承图单独检查环，函数参数/结果中的 Any 与非继承循环不被误拒绝。
- scan 先按地址验证只读节点和 child span、检测 active-path 环并复用共享节点，再使用既有 value shape/scan 算法核对具体 extent 和平移；没有另写一套 shape 语义。
- 新增 22 种 TD/code/scan 损坏测试，覆盖 guard-page 指针、共享 scan 在不同 extent 下的检查、box scan 平移和继承/签名关系的组合。格式化和全 workspace/all-targets Clippy 通过；image 相关 5 项 Rust/C 测试通过。静态存储和初始化关联尚待接入。

## 6. 静态 roots、immortal 与初始化关系

- 先登记只读 immortal 的精确对象起点、实际大小、header TD 和动态 String 长度，再解析所有 EncodedStaticValue 的 GC leaf 与 relocation；非引用 payload/padding 继续由 compiler/reader 负责。
- static storage 检查完整 writable extent、None/Recursive scan、ZST token 与零初态。显式遍历 References/Sequence/Array 的实际 leaf，检查动态 count、对齐、重复 offset、非空引用和 immortal 精确地址。
- unit 直接关联已登记的 storage、独占 failure slot、cell、initializer/ensure/gateway；核对 owner、地址和 fingerprint 镜像，拒绝多重角色和未归属的 Zeroed storage。root failure 使用同一规则。去重后统一拒绝 storage/token/cell 和 immortal 的区间重叠。
- 生成直接指向原 record 的唯一 GC root 列表及 canonical eager unit 序列；lazy 只登记。新增 40 种损坏用例及 Sequence/Array、String、ZST、eager ID 顺序组合。
- 格式化和全 workspace/all-targets Clippy 通过；codegen/runtime 306 项、toolchain 9 项通过。GC 发布和 startup 尚待完整 stackmap 关联完成。

## 7. 完整 stackmap 规范化与登记关联

- 将原 579 行 parser 按 blob、record 和索引职责拆分。保留前三个 location、全部 root pairs/live-outs、function address、instruction offset 及 blob offset；读取 count 后按剩余实际字节检查再分配。
- CommonCrypto SHA-256 消费与 Rust normalizer 相同的 domain-separated runtime scalar/count 编码。C 测试逐字锁定已有 Rust 固定向量，并验证 ConstantIndex 解析后与 Constant 产生相同摘要。
- raw records 经 typed safepoint/callable 关联、平台 root/frame 校验和 normalized fingerprint 比较后生成唯一 PC 索引。合法 ODR 多 blob 仅同 PC/完整 canonical payload 合并；额外或缺失 site、Strong 重复、owner/PC/location/live-out/fingerprint 错误均拒绝。
- 修复 AArch64 frame offset 在极大已加载 stack size 与正 offset 组合下的有符号加法溢出，使用边界内无符号计算。
- 新增三个连续 blob、零 root、ODR/ConstantIndex 组合及 20 个故障注入用例。格式化与全 workspace/all-targets Clippy 通过；codegen/runtime 307 项、toolchain 9 项全部通过。此索引将在下一功能直接交给 GC。

## 8. 逐次 gateway 的线程握手

- 主线程 attach 改为 native-safe、零 managed depth 和空 boundary。新增无参数 gateway 的 EntryPending 状态；只有真实 safepoint 入口可在同一 world mutex/epoch 协议下发布首个 anchor 并转为活动段。
- collector 只跳过明确 Pending 的新段，仍扫描既有 roots/冻结外层段；活动 managed 线程即使暂时没有 anchor 也必须等下一个 poll。返回到 C 后直接发布 native-safe，不能使用已离栈帧 park。
- 普通 managed entry 和携带 closure 参数的 callback 保留活动段握手，callback 深度与 native transition 的 segment 状态按原链 LIFO 恢复。spec/design 已说明 callback 准备阶段不能作为空段跳过。
- 新增条件变量控制的真实 GC 测试：入口前、Pending 首次 poll、活动段再次 poll、返回发布、两个 gateway 之间及两层 nested callback；另覆盖未 poll 就返回/进入其他 runtime entry 和活动段缺 anchor 的拒绝。
- 格式化和全 workspace/all-targets Clippy 通过；codegen/runtime 308 项全部通过。旧 harness 暂时显式 enter/leave 其历史入口，随后与 GC/startup 一并替换。

## 9. 唯一 registry、完整程序 startup 与实际运行迁移

- GC 直接引用 registry 的原 static storage record、精确 immortal 索引和已关联的 stackmap 索引，删除旧 by-value 根表、单表重复检查及 GC 的第二次 section 读取。运行期类型和初始化单元只做已登记地址查询；静态布局验证与动态对象大小算法各保留一份。
- 新增私有 `scoop_rt_run_program`，一次性完成 loaded image 收集/验证、真实 String TD 绑定、registry 发布、GC/线程/callback 初始化、逐次 eager/root gateway 调用和原有 shutdown。非法状态码与重复启动 fatal；failure reporter 在 world mutex 下重新从稳定 slot 读取异常，只将只读类型名称带出临界区。
- 移除 runtime 自带 main、旧 `scoop_main` 绑定、旧 image 初始化循环和 C frame boundary 替换。删除已无构建/测试入口引用、仍依赖退休单表及历史直接 C managed 调用的手工 smoke runner；其值、线程、callback、handle、GC 和调度能力由现行 focused C 测试及实际产物 fixture 验证。
- 所有现行实际产物运行辅助改为编译普通 executable runner Cone，引用各 artifact 原 image/root descriptor，并调用统一 startup；不再从 Link 数据合成 root/immortal 表。sibling 地址测试直接读取实际静态 TD/itable，保持 shutdown 后不进入 managed code。focused GC 测试直接提供所测的已解析 V1 record，加载边界仍由独立测试覆盖。
- 新增 `m23-runtime-images` 真实 fixture：3 个 image 的 empty/ordinary/NoGC main；6 个 image 的 chain/diamond 和动态 ready-set 顺序；跨 image 静态对象/String roots；root 与 eager 未捕获异常。输入 image 枚举逆序，provider/consumer 源码在链接运行前移走；全部在普通与 moving-GC stress 模式通过。trace 使用当前 Cone 已支持的 fixture-native 声明，不引入 M23-10 的外来 generic native body 消费。
- 格式化与全 workspace/all-targets Clippy 通过；codegen/runtime 308 项通过；实际泛型初始化组合、sibling adapter/member 并集组合及新增 3 项多 image 运行矩阵通过。已重新构建配套 scoopc。历史 core-layout artifact fingerprint snapshot 与本阶段格式尚待同步，未计为通过项。

## 10. gateway 的完整 LIR 结构合同

- 在完整 LIR 验证边界识别现有 root/initialization gateway body key，核对 executable main 或 eager ensure 的 typed 本地引用；lazy unit 不能有 startup gateway，root 使用自身唯一 failure storage。
- 验证 managed C `uint32_t(void)` 签名、首个真实 poll、单一 invoke 及独立成功/失败退出。返回值封闭为 0/1；root catch 必须物化、写入自身 failure root、EndCatch，eager catch 只能平衡原 ensure 的异常。
- 检查按 gateway 身份/目标与 CFG/catch 职责拆成两个短模块，不重放语言类型检查、safepoint liveness 或 ABI 语义。共用原有完整 LIR 的 ABI、site 与 root-plan 验证。
- 增加 managed main、NoGC main、eager 三种正例及 23 种错误结构；包含缺 poll、poll 重排、错误 runtime/ensure 目标、catch 生命周期、错误 owner/failure storage、lazy gateway 和非法状态码。
- 格式化与全 workspace/all-targets Clippy 通过；LIR 473 项、codegen/runtime 308 项通过；重建配套 scoopc 后，全部 3 项真实多 image 启动矩阵再次通过。上次提交后的专用 target 清理释放 893.7 MiB，清理后的 toolchain 9 项通过。

## 11. 启动生命周期、lazy 失败与实际初始化环

- 新增实际 3+ Cone 运行组合：六层 eager 提前 ensure、未访问 lazy 无副作用、lazy 失败不重试及其内部引用跨移动 GC 保留、间接初始化环和 initializer 内捕获自身环后成功。eager provider 失败时，后续依赖单元和 main 均未执行。
- 重复启动和 initializer 内 C 重入沿同一 startup 入口拒绝；实际 image 输入为空或缺失 core 时，在任何 eager 副作用之前失败。所有 provider 源码在消费前移走，完整 8 项启动矩阵在普通与 moving-GC stress 模式均通过。
- 将逐次 gateway wrapper 单独放入短文件，focused C 测试直接调用生产 wrapper。覆盖非法状态码、空 failure root、错误 unit 状态，以及另一线程真正移动异常后从稳定 slot 报告 root/unit 失败。
- 格式化与全 workspace/all-targets Clippy 通过；codegen/runtime 309 项通过。实际循环 fixture 解构 Option 后比较保存的 String 引用，验证 failure payload 的存活，不依赖额外的聚合值相等调用。

## 待完成

1. 补充多名初始化等待者与真实移动 GC 的组合，并完成既有静态值/ODR/重建 core 运行矩阵回归。
2. 同步全部受格式变化影响的 golden，运行配套 scoopc 的完整 workspace 回归，整理验收文档并清理 target。
