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
| Project ledger | `.lilygo-skills/ledger.json`, `.lilygo-skills/context-digest.json` | prompt-safe 项目记忆和重复上下文 digest。 |

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

## 默认注入和预算

默认只注入：

- matched skill id 和摘要；
- 当前任务需要的 top facts；
- readiness 或 `needs_source_ingestion`；
- source/query/generation 的下一步命令；
- evidence boundary 和权限提示。

完整 fact pack、reference 文档和模板不会默认注入。只有实现、调试、setup、生成或
验证任务需要时，Agent 才读取对应文件。

prompt budget 是正确性的一部分。纯查询 prompt 保持只读和紧凑：fact table、source
refs 和 `source query` 命令就足够。实现/调试 prompt 可以增加
`goal-plan-bridge`、精选 demo 和带权限的 next actions，但重复的 board、framework、
demo、project-skill 和 topic 细节会折叠成短的增量提示，并保留展开命令。

增量 hook context 是按 session 隔离的。只有 hook event 带稳定 session id，或测试设置了
`LILYGO_SKILLS_SESSION_ID` 时才启用。Cache 会按 TTL 和 runtime 版本失效；
`LILYGO_SKILLS_DISABLE_INCREMENTAL=1` 可以强制回到完整 capsule。增量 capsule 仍保留
关键引脚、source-query 展开和 evidence boundary；被裁掉的只是重复的大块 facts、
demo、recipe、generated skills 和 playbook 摘要。

Project ledger context 是按固件仓库隔离的。执行 `project init` 后，runtime 可以把
公开 context digest 和曾验证 capability 摘要写到 `.lilygo-skills/`。重复 prompt
因此可以拿到更短的 ledger capsule，而不是同一份完整上下文。这个 capsule 仍保留关键
事实、证据边界、stale 标记和展开命令。代码、source signature、runtime 版本、TTL
或用户显式要求重新验证变化时，ledger 条目会变成 stale。

稳定展开命令会一直保留：

```bash
lilygo-skills source query --board <board-id> --topic io --json
lilygo-skills index query <skill-or-playbook-id> --json
lilygo-skills goal plan --json "<prompt>"
lilygo-skills project ledger show --project <project-dir> --json
```

不完整的 starter board pack 也遵循这个规则。它可以暴露 `unknown_with_sources` 或
`needs_source_ingestion` 加官方 reference，让 Agent 知道下一步去哪查；但不能为了填满
capsule 而编造引脚、外设或运行行为。

实现和调试类 prompt 还会得到一个紧凑的 source recovery capsule：最近的官方 demo
路径、板子自己的源码 header、少量关键事实和扩展命令。比如 T-Display-S3 的
TFT_eSPI + I2C prompt 可以暴露 `examples/tft/tft.ino`、Setup206、
`pin_config.h`、`PIN_IIC_SDA=GPIO18`、`PIN_IIC_SCL=GPIO17` 和 `source query`
命令，而不是把整个 fact pack 注入 prompt。

## Preferences 和 References

Preferences 是公开行为偏好，例如框架顺序、串口调试工具、代码行数限制和安全默认。
References 是公开阅读提示，例如官方示例、源码、datasheet、硬件说明和项目设计文档。

两者都不能覆盖官方 source facts。板子或 topic 缺少关键事实时，应先返回
`needs_source_ingestion` 或 `unknown_with_sources`，而不是把 reference 当作已经 ready。

## 安装态

`node install.js --all` 默认先挂载 Skill。没有编译好的 runtime binary 时，它会进入
mount-only 模式：接好 Codex/Claude 入口，复制 meta router、source data、
`skills/references/` 和 `templates/skills/`，并提供 setup-only launcher。完整动态
注入可以后续通过 release 包预编译路径（`node install.js --all --prebuilt-only`）、
`node install.js --all --build` 或 `--bin /path/to/lilygo-skills` 开启。
安装态 Agent 因此至少能查看和源码 checkout 一致的上下文契约，并能通过 setup plan
继续配置 Rust/Cargo、Arduino、PlatformIO、ESP-IDF 或 Rust esp-rs 工具链。

`doctor --json` 会从被检查的 HOME 验证安装态 runtime。除了 route 和 hook 样例，它还会
在 Codex 与 Claude 两端 runtime 都存在时比较镜像。一端存在时通过；两端一致时通过；
两端漂移时报告 warning，并给出重跑安装命令。

支持模型按 board family 扩展。当前已验证 runtime 覆盖从 LilyGO ESP32 系列开始。
没有对应 V4/V5 证据时，不把 source/context 结果表述成已经完成的硬件行为。
