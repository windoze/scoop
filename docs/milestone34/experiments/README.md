# M34 ABI 技术验证

`aggregate.c` 只用于从独立 C compiler 观察真实签名，不是 Scoop caller／callee 自证。Clang 22.1.8 的复跑方式：

```sh
clang -target aarch64-apple-darwin -O1 -S -emit-llvm aggregate.c -o aggregate-darwin.ll
clang -target x86_64-unknown-linux-gnu -O1 -S -emit-llvm aggregate.c -o aggregate-gnu.ll
clang -target x86_64-unknown-linux-musl -O1 -S -emit-llvm aggregate.c -o aggregate-musl.ll
```

| C 类型／场景 | Darwin/AArch64 | Linux/amd64 GNU／musl |
| --- | --- | --- |
| 两个 UInt64 | `[2 x i64]` 参数及结果 | 两个 i64 参数，`{i64, i64}` 结果 |
| Double + UInt64 | `[2 x i64]` 参数及结果 | double、i64 参数，`{double, i64}` 结果 |
| 两个 Float | `[2 x float]` 参数，原 struct 结果 | `<2 x float>` 参数及结果 |
| 四个 Double | `[4 x double]` 参数，原 HFA struct 结果 | byval 参数，隐藏 sret 结果 |
| 六个整数后传 Mixed | 剩余两个整数寄存器 | 整个 Mixed 回退到 byval，不仅回退整数分量 |

M34-5 据此实现目标 classifier，并以独立 C 函数的实际互调补齐边界、padding、嵌套、寄存器耗尽与位型测试；这份实验不代表 DirectC aggregate 已交付。

`coercion.c` 补充小尺寸、显式对齐、packed 和 HFA 边界，使用上面的命令替换输入文件即可复跑。Clang 22.1.8 的关键结果：3-byte struct 在 Darwin 使用 i64 参数／i24 结果，在 SysV 两者均为 i24；16-byte 对齐且带尾部 padding 的单 Float 在 Darwin 使用 i128，在 SysV 只使用 float。packed `{char, double}` 在 SysV 为 MEMORY，在 Darwin 使用 `[2 x i64]`；三个 Double 在 Darwin 为 HFA，五个 Double 改用独立副本指针。SysV 寄存器不足时整个 aggregate 回退，剩余的另一类寄存器留给后续参数。

M34-5a 的 `m34-small-values-native` 正式 fixture 用独立 LLVM 入口验证 Scoop 小值调用，包含 signaling NaN 位模式、负零、3-byte carrier 的高位补零、struct padding 补零、参数寄存器回退和 Managed invoke／GC。这些入口明确遵守 Scoop ABI；正向 C aggregate 调用另由 M34-5b 验收。

混合 AS1／metadata 的返回、invoke 与 relocation 由 codegen 中 `interface_parts_call_invoke_and_relocation_survive_target_codegen` 验证，覆盖三个 target × debug／release。invoke 使用既有零 gc-live statepoint／显式根协议；普通 Itanium invoke 直接交给 RS4GC 会产生不可用的异常边 relocation，不能据此更换既有协议。

实验结合实际 codegen 的根协议保存完整接口，跨 site 对 object 执行既有根标记和 volatile leaf 回写，再以 volatile object reload 与原 metadata 重建，覆盖 PHI／select 合流。仅有 volatile store 仍可能发生旧值转发，因此重读同样必须阻止该合并。检查只有 AS1 object 形成 relocation，metadata 不进入 roots，不存在 managed pointer 与整数互转，并执行 LLVM safepoint verifier 和目标机器码生成。直接返回跨 site 的原 aggregate SSA 不符合此协议：InstCombine 可重新合并 extract／insert，RS4GC 不会自动为整体 aggregate 回写 object。后续条件 poll 的根存储只在慢路径物化，仍须保持这一回写与合流关系。
