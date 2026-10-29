# 原生窗口与 Metal surface

`renderer` 与 `app` 是两个普通 Cone。renderer 从 `scoop.window.native` 取得显式 lease，自行安装 CAMetalLayer、建立 Metal queue、提交真实 drawable，并在关闭时先恢复原 layer 再释放 lease。窗口库不持有图形设备。

准备 Darwin sysroot 后可运行：

```sh
scoop run sysroot/src/scoop.window/examples/metal/app --release
scoop test sysroot/src/scoop.window/examples/metal/app --release
```

应用显示蓝色窗口，普通文字与输入法组合／提交通过同一批次应用到 TextInputState，并在标题栏显示结果。候选窗锚点固定在内容区 `(12, 34)`；这是窗口／文本输入接入示例，编辑器导航、删除和排版由上层实现。关闭窗口、按未被输入法消耗的 Escape 或请求应用退出即可结束。

测试入口在实际提交一帧后自动退出，验证 surface 收尾先于原生窗口销毁。本示例同步等待这一帧的 GPU 完成，以展示最小所有权顺序；完整 renderer 的多帧并行、后台提交和帧调度属于其自身实现。
