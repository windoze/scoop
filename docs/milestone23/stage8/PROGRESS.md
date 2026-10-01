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

## 待完成

3. GC 直接消费唯一 registry，移除旧单表与重复 section 读取。
4. EntryPending、逐次 gateway 线程握手、canonical eager 顺序、异常报告与 shutdown。
5. 真实 3+ Cone 产物运行、损坏输入和受控线程竞争矩阵，以及历史运行辅助迁移和最终全仓回归。
