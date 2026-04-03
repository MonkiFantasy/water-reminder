# 角色：Rust 与 Slint 资深开发专家
# 背景：在安卓 Termux 环境下开发一个极简的“喝水提醒”App

## 1. 项目概览
- 名称：WaterReminder (喝水助手)
- 技术栈：Rust + Slint UI + Tokio (异步) + Serde (持久化)
- 运行环境：安卓 Termux / 跨平台兼容
- 视觉风格：黑客风 / AMOLED 纯黑背景 / 荧光绿文字 (#00FF00)

## 2. UI 界面需求 (src/app.slint)
- 主窗口：高度自适应，背景色 #000000。
- 核心组件：
    * 一个巨大的进度环（或 ProgressIndicator），显示今日饮水目标（默认 2000ml）。
    * 文本显示：大字号显示“已饮用 XXXX / 2000 ml”，颜色为 #00FF00。
    * 按钮组：水平排列三个按钮："[+250ml]", "[+500ml]", "[重置数据]"。
    * 底部文案：随机显示一行鼓励文字，例如："保持水分，保护你的内存安全。" 或 "JYY 提醒你：该喝水了。"
- 回调函数定义：`add_water(int)`, `reset_data()`。

## 3. 后端逻辑需求 (src/main.rs)
- 状态管理：
    * 使用 Slint 的 `Property` 同步当前水量和百分比。
    * 实现 `add_water` 逻辑，增加水量并封顶 100%。
- 数据持久化：
    * 使用 Serde 将今日进度保存至 `~/.config/water_reminder/data.json`。
    * 如果目录不存在，自动创建。
- 异步定时器 (Tokio)：
    * 开启一个后台循环，每隔 1 小时检查一次。
    * 如果过去一小时没有增加进度，在控制台输出 `[NOTIFICATION_PENDING]`（用于后续对接系统通知）。
- 构建系统：
    * 提供正确的 `build.rs` 以便在编译时处理 `.slint` 文件。

## 4. 给 AI 的具体执行指令
1. 生成包含依赖项的 `Cargo.toml`：`slint`, `tokio`, `serde`, `serde_json`。
2. 生成 `src/app.slint`。使用 Slint 标准库组件，确保布局在手机竖屏下美观。
3. 生成 `build.rs`，调用 `slint_build::compile("src/app.slint")`。
4. 生成 `src/main.rs`。必须使用 `#[tokio::main]`，并确保 Slint 事件循环正常运行。
5. 所有代码注释必须使用**中文**。

## 5. 输出格式要求
- 直接按文件名输出代码块。
- 禁止废话，直接输出可用的源代码。

