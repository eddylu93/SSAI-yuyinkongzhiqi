AU05 Desk 使用 GPL-3.0，许可全文位于项目根目录 LICENSE。单次语音时长排名取自 GetSayAll/remote-mic-app-windows 的 statistics.rs，保留时长清理、应用名称规范化、排序及截断逻辑，并接入本项目的 SQLite 查询与序列化接口。[原始实现](https://github.com/GetSayAll/remote-mic-app-windows/blob/180f958d922b51dd0f803a609cd03240e3745266/crates/sayall-core/src/statistics.rs)

Vue 界面采用 Phosphor Icons 图标。设备照片由用户提供。安装目录内包含对应源码、依赖锁文件和随包许可证。Windows 设备直连使用 Microsoft Windows Raw Input，Windows 和 macOS 的厂商直控都使用 hidapi。历史插件方案归档于 legacy/studio，独立版从源码构建及运行均无需该目录。[Phosphor Vue](https://github.com/phosphor-icons/vue) · [hidapi-rs](https://github.com/ruabmbua/hidapi-rs)

AU05 通信帧格式、事件代码、状态查询及临时订阅时序参考了 MIT 项目 AU05 Keys 和 open-vibekey。此处实现为 Rust，两个项目的许可随包提供。[AU05 Keys](https://github.com/elliclee/ulanzi-au05-keys/tree/7819351a5ba5c68874d0519fabe3bf62db47e328) · [open-vibekey](https://github.com/coopjz/open-vibekey/tree/468cf657ddb100a4f0a594353f82909c9a830aa9)

灯效参数和命令字段参考了 [OpenVibeKey 的 AU05 HID 设备实现](https://github.com/palaemonboy/OpenVibeKey/blob/c6466fd4687da612c9c52b006399f87148903727/native/VibeKit/Sources/VibeKitHID/VibeKitDevice.swift)。MIT 许可随安装包提供。
