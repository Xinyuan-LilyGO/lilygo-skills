# 验证等级

English version: [VERIFICATION_LEVELS.md](VERIFICATION_LEVELS.md)。

lilygo-skills 使用显式验证等级，避免把 source context 误认为硬件成功。

`V` 表示 verification，也就是“这件事已经被什么级别的证据证明”。数字越高，
证据越接近真实设备。V3 说明上下文和资料链路可信；V5 才说明请求绑定的真实硬件或
live transport 已经跑通。

| Level | 含义 | 示例证据 |
|-------|------|----------|
| V0 | 静态文件或 schema 存在 | Skill 文件、registry entry、JSON 可解析 |
| V1 | Registry integrity | `lilygo-skills verify --json` |
| V2 | Route behavior | `route`、route fixtures、benchmark coverage |
| V3 | Source/context/completeness | `source query`、`source completeness`、dry-run enrichment、hook output |
| V4 | 无物理证明的 runnable artifact | Build output、simulator page data、OTA harness artifact |
| V5 | 物理设备或 live transport proof | Flash success、serial app log、OTA 到设备、display pixels、peripheral behavior |

## 自然语言触发

用户可以直接指定想要的证据等级：

| 用户可以说 | Agent 应该做 |
|------------|--------------|
| “验证这个 prompt 会不会注入正确的 T-Display-S3/LVGL context。” | 跑 route、hook 或 benchmark，通常到 V2/V3 |
| “确认这个板子的显示资料和 demo reference 是完整的。” | 跑 `source query`、`source completeness`，必要时给出 enrichment 下一步，通常到 V3 |
| “帮我 build 到可运行产物，但先不要烧录。” | 走 setup/build 计划和 build output，目标 V4 |
| “我插上板子了，帮我 flash 后看串口日志。” | 请求串口/烧录权限，收集 flash success 和 serial app log，目标 V5 |
| “帮我验证 OTA 真的到设备。” | 请求网络/OTA 权限，使用项目私有 runner，收集传输和设备侧确认，目标 V5 |

资料查询和实现思路停留在 source/context 等级。用户要求执行，并给出相应权限和
设备/网络条件时，才进入 build、flash、serial、OTA 或 display 证据路径。

## 当前 Release Claim

当前 release 对 source/context/completeness 行为验证到 V3。Verified 表示：

- 代表性的重叠板子可以精确 product routing。
- `source completeness` 会显式返回 complete、partial、`needs_source_ingestion` 或
  unsupported。
- `update board-facts --dry-run` 会报告 enrichment path，且不写入。
- Route/hook/goal readiness signals 保持紧凑且 no-write。
- Unsupported enrichment apply fail closed。
- Benchmarks、smokes、installer dry-runs 和 installed runtime probes 通过。

V4/V5 证据是 task-scoped。用户请求并授权后，goal flow 会为该任务记录对应的 build
artifact、flash result、serial log、display artifact、OTA transfer 或 peripheral
measurement。

Hardware gold-standard harness 是验证 surface 的一部分，但默认运行仍是 V3：

```bash
bash scripts/hardware-gold-standard-live-smoke.sh --dry-run
```

Dry-run 验证权限模型、脱敏输出形态、失败模式报告和 artifact hash，不接触硬件。
无设备、串口选错或烧录超时这类 boundary mode 会返回结构化 boundary result；真实
设备证据会把任务升级到 V4/V5。

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
cargo run -p lilygo-skills-cli -- verify --json
cargo run --release -p lilygo-skills-cli -- benchmark --json --iterations 5000
bash scripts/source-completeness-smoke.sh --dry-run
bash scripts/board-completeness-smoke.sh --dry-run
bash scripts/full-evidence-smoke.sh --dry-run
```

只有任务确实需要执行时才使用 goal permissions：

```bash
cargo run -p lilygo-skills-cli -- goal start --plan .tmp/goal-plan.json --allow-build --json
cargo run -p lilygo-skills-cli -- goal start --plan .tmp/goal-plan.json --allow-flash --allow-serial --port <port> --json
cargo run -p lilygo-skills-cli -- goal start --plan .tmp/goal-plan.json --allow-network --allow-ota --json
cargo run -p lilygo-skills-cli -- goal start --plan .tmp/goal-plan.json --allow-simulator --json
```

如果没有 execution permission，`goal start` 应保持 dry-run 或 no-write planning surface。

真实硬件 harness 的权限形态是显式的：

```bash
bash scripts/hardware-gold-standard-live-smoke.sh --dry-run
bash scripts/hardware-gold-standard-live-smoke.sh --port <port> --allow-build --allow-flash --allow-serial
```

第二种形式只有在请求的操作真实执行、并写出和该操作绑定的脱敏 artifact 后，才可能产生
V4/V5 证据。
