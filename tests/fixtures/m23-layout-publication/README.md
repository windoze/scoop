# Layout 产物发布

本组复用 `m23-any-call-signatures/standalone.scoop` 与 `combined.scoop` 的真实产物，
提供方为 `m23-core-layout-exports/property-initialization-provider.scoop`。
独立用例覆盖首次发布，组合用例覆盖已有目标的原子替换；两者均重开实际落盘文件，
核对共用语义结果生成的发布摘要、Link 对象数量与 dependency record。

负例保留合法 envelope/hash，分别破坏源码调用签名、当前和依赖产物的 Link Code
投影、实际 Link 对象，并覆盖缺失依赖及 rename 失败。每次失败均确认已发布文件
字节不变且无临时文件残留。

HIR/MIR/LIR golden 沿用原源码 fixture，发布结果单独锁定在本目录的两个 snapshot。
