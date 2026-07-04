# 上下文层说明

English version: [CONTEXT_LAYER.md](CONTEXT_LAYER.md)。

lilygo-skills 不是把所有资料一次性塞进 prompt。它把稳定的运行逻辑、可生成的
Skill 文件、source facts、项目偏好和参考源拆开，让 Agent 先拿到最小可用上下文，
再按任务需要读取更深的资料。

## 静态层和动态层

| 层 | 路径 | 说明 |
|----|------|------|
| Meta Skill | `skills/lilygo-router/SKILL.md` | 唯一提交的 Skill 入口。 |
| Static references | `skills/references/*.md` | 可读的上下文展开文档，不是路由 Skill。 |
| Templates | `templates/skills/*.md` | 生成 runtime Skill 文件的公开模板。 |
| Source model | `data/**`, `index/**` | 板子、事实、外设、playbook、route 数据。 |
| Generated skills | install/cache/project output | 安装、更新或显式生成时产生。 |

当前关键边界是：route 层只使用明确 token，不使用危险的前缀/子串匹配；source
model 只给真实芯片生成 chip skill，复合标签和容量信息保留在外设/板级事实里；
`goal complete` 是 completion coordinator，可以把已有 facts/demo refs 转成下一步
source readiness，但不能把上下文表述成硬件已验证。

模板只规定生成文件的结构，不承担板子事实。不同生成 skill 看起来有相同段落，是因为它们
共享 `templates/skills/*.md`；真正的内容来自 source model、fact pack、source pack、
recipe/playbook 数据和官方资料。这样开源仓库能保持可读、可复现，也不会提交大量生成
快照。

以 T-Watch Ultra 为例，生成出来的 `board-t-watch-ultra` 不是空模板。它会包含：
ESP32-S3、Arduino FQBN、AMOLED `CO5300`、触摸 `CST9217`、GNSS `MIA-M10Q`、
LoRa radio option `SX1262 or SX1280`、NFC `ST25R3916`、IMU `Bosch BHI260AP`、
电源 `AXP2101`、RTC `PCF85063A`、震动 `DRV2605`、扩展 IO `XL9555`、SD、PSRAM、
官方 LilyGoLib 文档、driver header 和 demo 路径。

聚焦 T-Watch Ultra 的 generated tree 大致是：

```text
skills/board-t-watch-ultra/SKILL.md
skills/chip-bhi260ap/SKILL.md
skills/chip-xl9555/SKILL.md
skills/feature-raise-to-wake/SKILL.md
skills/periph-display/SKILL.md
skills/periph-imu/SKILL.md
skills/periph-input/SKILL.md
skills/app-ota/SKILL.md
skills/app-watch-ui-lvgl/SKILL.md
skills/debug-lvgl-loop/SKILL.md
skills/fw-lvgl/SKILL.md
skills/playbook-build-flash-serial/SKILL.md
skills/playbook-lvgl-debug/SKILL.md
skills/playbook-ota-debug/SKILL.md
skills/playbook-source-discovery/SKILL.md
skills/references/*.md
templates/skills/*.md
index/routes.json
```

实际使用时，板子 skill 是紧凑快照；更细的上下文来自 `goal plan.context_capsule`、
`source query` 和 `source completeness`。例如：

```bash
lilygo-skills goal plan --json "T-Watch Ultra Arduino LVGL touch does not move"
lilygo-skills goal plan --json "T-Watch Ultra ESP-IDF OTA rollback manifest debug"
lilygo-skills source query --board board-t-watch-ultra --topic io --json
```

这三类输出会按任务加载 display/touch/OTA/IMU/source refs/demo refs/权限和证据边界，
不会把全部 fact pack 默认塞进 prompt。

## 默认注入

默认只注入：

- matched skill id 和摘要；
- 当前任务需要的 top facts；
- readiness 或 `needs_source_ingestion`；
- source/query/generation 的下一步命令；
- evidence boundary 和权限提示。

完整 fact pack、reference 文档和模板不会默认注入。只有实现、调试、setup、生成或
验证任务需要时，Agent 才读取对应文件。

## Preferences 和 References

Preferences 是公开行为偏好，例如框架顺序、串口调试工具、代码行数限制和安全默认。
References 是公开阅读提示，例如官方示例、源码、datasheet、硬件说明和项目设计文档。

两者都不能覆盖官方 source facts。板子或 topic 缺少关键事实时，应先返回
`needs_source_ingestion` 或 `unknown_with_sources`，而不是把 reference 当作已经 ready。

## 安装态

`node install.js --all` 默认先挂载 Skill。没有编译好的 runtime binary 时，它会进入
mount-only 模式：接好 Codex/Claude 入口，复制 meta router、source data、
`skills/references/` 和 `templates/skills/`，并提供 setup-only launcher。完整动态
注入需要后续 `node install.js --all --build` 或 `--bin /path/to/lilygo-skills`。
安装态 Agent 因此至少能查看和源码 checkout 一致的上下文契约，并能通过 setup plan
继续配置 Rust/Cargo、Arduino、PlatformIO、ESP-IDF 或 Rust esp-rs 工具链。

支持模型按 board family 扩展。当前已验证 runtime 覆盖从 LilyGO ESP32 系列开始。
没有对应 V4/V5 证据时，不把 source/context 结果表述成已经完成的硬件行为。
