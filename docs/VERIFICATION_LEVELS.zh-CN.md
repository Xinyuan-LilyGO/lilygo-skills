# 验证等级

English version: [VERIFICATION_LEVELS.md](VERIFICATION_LEVELS.md)。

lilygo-skills 使用显式验证等级，避免把 source context 误认为硬件成功。

`V` 表示 verification，也就是“这件事已经被什么级别的证据证明”。数字越高，
证据越接近真实设备。V3 说明上下文和资料链路可信；V5 才说明请求绑定的真实硬件或
live transport 已经跑通。

| Level | 含义 | 示例证据 |
|-------|------|----------|
| V0 | 静态文件或 schema 存在 | Skill 文件、registry entry、JSON 可解析 |
| V1 | Data-integrity self-check | `lilygo-skills doctor --json` |
| V2 | Context routing | `lilygo-skills context`、`eval/**` 下的 context fixtures |
| V3 | Source/context/verification | `source query`、`verify sources`、hook output |
| V4 | 无物理证明的 runnable artifact | Build output、simulator page data、OTA harness artifact |
| V5 | 物理设备或 live transport proof | Flash success、serial app log、OTA 到设备、display pixels、peripheral behavior |

## 自然语言触发

用户可以直接指定想要的证据等级：

| 用户可以说 | Agent 应该做 |
|------------|--------------|
| “验证这个 prompt 会不会注入正确的 T-Display-S3/LVGL context。” | 跑 `context` 和 `hook`，通常到 V2/V3 |
| “确认这个板子的显示资料和 demo reference 是否仍然成立。” | 跑 `source query` 和 `verify sources`，必要时报告 drift 或 enrichment 下一步，通常到 V3 |
| “帮我 build 到可运行产物，但先不要烧录。” | 走 setup/build 计划和 build output，目标 V4 |
| “我插上板子了，帮我 flash 后看串口日志。” | 请求串口/烧录权限，收集 flash success 和 serial app log，目标 V5 |
| “帮我验证 OTA 真的到设备。” | 请求网络/OTA 权限，使用项目私有 runner，收集传输和设备侧确认，目标 V5 |

资料查询和实现思路停留在 source/context 等级。用户要求执行，并给出相应权限和
设备/网络条件时，才进入 build、flash、serial、OTA 或 display 证据路径。

## 当前 Release Claim

当前 release 对 source/context/verification 行为验证到 V3。Verified 表示：

- 代表性的重叠板子可以精确 product routing。
- `verify sources` 会按 fact 显式返回 `OK`、`DRIFT` 或 `UNREACHABLE`。
- `source query` 返回带 source 引用的事实和 unknowns，不编造值。
- `context` 和 `hook` readiness signals 保持紧凑且 no-write。
- `doctor` 在 data-integrity 问题上 fail closed。
- `npm test`、`npx tsc --noEmit`、ci-gate suite 和 installer dry-runs 通过。

V4/V5 证据是 task-scoped。用户要求执行并授权对应设备/网络后，agent 会为该任务记录对应的
build artifact、flash result、serial log、display artifact、OTA transfer 或
peripheral measurement。JS thin core 不再提供自动化硬件 harness 命令；这类证据由 agent
在运行授权任务时收集。

## 何时声明硬件成功

存在和请求任务绑定的 live evidence 时使用 V5，例如：

- 目标板子的 build + flash 命令成功。
- 预期固件的 serial monitor output。
- OTA 命令结果和设备侧确认。
- LVGL simulator artifact 对应 V4；真实 display/camera/touch 证据对应 V5。
- IMU、GNSS、LoRa、power、haptic、audio 或 storage 的外设专用 logs / measurements。

官方 demo 链接和 datasheet 路径是有用的 source evidence，但不能证明本地固件 build 或
已连接板子可工作。

## Common Verification Commands

```bash
lilygo-skills doctor --json
lilygo-skills verify sources --board <board-id> --json
npx tsc --noEmit
npm test
bash scripts/ci-gate.sh
```

V4/V5 证据是 task-scoped，位于 JS thin core 之外。任务确实需要执行时，agent 只有在用户
授予对应设备/网络权限后，才直接运行 build、flash、serial、OTA 或 peripheral 步骤，并记录
和该操作绑定的脱敏 artifact：

- 用项目自己的工具链 build(PlatformIO、Arduino、ESP-IDF、Rust)。
- 在连接的端口上 flash 并读串口 monitor。
- 通过项目私有 runner 跑 OTA 并抓设备侧确认。

没有这些显式权限时，验证停留在 V3 source/context 边界，不执行任何操作。
