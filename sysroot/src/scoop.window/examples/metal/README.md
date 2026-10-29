# 原生窗口与 Metal surface

`renderer` 与 `app` 是两个普通 Cone。renderer 从 `scoop.window.native` 取得显式 lease，自行安装 CAMetalLayer、建立 Metal queue、提交真实 drawable，并在关闭时先恢复原 layer 再释放 lease。窗口库不持有图形设备。

准备 Darwin sysroot 后可运行：

```sh
scoop run sysroot/src/scoop.window/examples/metal/app --release
scoop test sysroot/src/scoop.window/examples/metal/app --release
```

应用显示蓝色窗口，关闭窗口或请求应用退出即可结束。测试入口在实际提交一帧后自动退出，验证 surface 收尾先于原生窗口销毁。本示例同步等待这一帧的 GPU 完成，以展示最小所有权顺序；完整 renderer 的多帧并行、后台提交和帧调度属于其自身实现。
