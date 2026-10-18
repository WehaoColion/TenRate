# iOS 1.0.0.1 — 原生 iPhone 首版候选

2026-10-18。应用户明确选择“新建 iOS 客户端”，新增独立 SwiftUI 工程。
PR：[37](https://github.com/WehaoColion/TenRate/pull/37)，未合并。

支持计时格子、命名、分类、启停、短休息投影、累计时间、历史、归档恢复，以及纯文本便签编辑、搜索、回收站和恢复。工作区在本机原子保存，可导出 JSON 备份。
计时、便签数据变更直接使用现有 Rust 核心源码。iOS 最低版本 18，云端工具链要求 Xcode 27 和 iOS 27 SDK，并运行 iPhone 模拟器检查。

这不是 Android/Windows 全功能移植：账户同步、财务界面、AI、知识画布、附件、备份导入尚未接入。加密、富文本及结构化内容禁止编辑，防止覆盖损失。不修改现有桌面服务或账户身份。

安装产物是 arm64 未签名 IPA，必须重新签名后才能安装。没有 Apple 签名材料时，不将其标作可直接安装的正式包，也不把 Android keystore 用作 Apple 签名。
产品版本展示和文件名为 1.0.0.1，Apple 营销版本 1.0.0、构建号 1。以后固定前三段，仅递增第四段。

## 检查状态

- Rust 业务流程 4 项通过：计时重新打开与归档恢复、Unicode 便签回收恢复和编辑保护、错误请求拒绝，以及规范化纯文本便签的再次编辑。
- iOS 数据测试 4 项通过：任务名称重开保留、损坏文件不覆盖、写入失败保持旧状态、便签保存和重开后仍可编辑。
- iPhone 界面测试 1 项通过：打开格子面板、保存名称、新建便签、保存后显示、重开后保留内容、再次进入编辑器。界面测试发现的单一纯文本块误判问题已修复；加密、富文本及复杂结构继续禁止编辑。
- [最终 Actions 构建](https://github.com/WehaoColion/TenRate/actions/runs/39135713201) 全部成功，源码提交 `2671be9abaca884fc10fbe443812a4291442d14b`。Xcode 27.0、iPhoneOS 27.0 SDK，最低 iOS 18.0；检查并查看了浅色、深色及格子面板模拟器截图。
- 真机安装、真机深色/大字体、后台恢复及重启未验收。未声称任何跨端同步已通过。
- 云端环境无法检查用户原 Windows 电脑的自启动任务、本机守护程序及公网服务身份；本轮没有修改这些服务。

安装说明见 [ios/README.md](../../../ios/README.md)。

## 已交付产物

[GitHub iOS 预发布](https://github.com/WehaoColion/TenRate/releases/tag/ios-v1.0.0.1-build-3) 包含未签名 IPA、安装说明、校验文件和模拟器截图；不是已签名正式发布。

- 文件：`TenRate-iOS-1.0.0.1-unsigned.ipa`，1,280,497 字节。
- 架构：arm64 真机应用，包名 `com.ofairyo.tenrate.ios`。
- SHA256：`618d0c1f6fde2b3910cccacc4b134bab7f34bc8b0e30a1a1f3cf5d97799bb3da`。
- 本地提取的 IPA 与 GitHub Release 资产摘要一致，已检查包内版本、SDK 和最低系统版本。
- Apple 签名材料尚未确认；安装前必须重新签名，尚未进行真机安装验收。
