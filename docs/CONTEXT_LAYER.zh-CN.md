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
| Source model | `data/**`, `index/routes.json` | 板子、事实、外设和 skill-registry 数据。 |
| Generated skills | install/pipeline output | 安装和 official-source pipeline 时产生。 |

当前关键边界是：板子检测只使用明确 token，不使用危险的前缀/子串匹配；source
model 只给真实芯片生成 chip skill，复合标签和容量信息保留在外设/板级事实里；capsule
可以把已有 facts/demo refs 转成 readiness signal，但不能把上下文表述成硬件已验证。

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

实际使用时，板子 skill 是紧凑快照；更细的上下文来自 `context`/`hook` capsule 加上
`source query` 和 `verify sources`。例如：

```bash
lilygo-skills context --json "T-Watch Ultra Arduino LVGL touch does not move"
lilygo-skills source query --board board-t-watch-ultra --topic io --json
lilygo-skills verify sources --board board-t-watch-ultra --topic io --json
```

这些输出会按任务加载 display/touch/IMU/source refs 和证据边界，不会把全部 fact pack
默认塞进 prompt。

## 默认注入和预算

默认只注入：

- 解析出的 board id 和 matched skills；
- 当前任务需要的 top facts；
- 验证边界(`context-injection`、`hardware_verified=false`、`evidence_boundary=V3`)；
- 该板子的 `source query` 展开命令。

完整 fact pack、reference 文档和模板不会默认注入。只有实现、调试、生成或验证任务需要
时，Agent 才读取对应文件。

prompt budget 是正确性的一部分。thin `context` capsule 保持紧凑并指向 `source query`；
thick `hook` capsule 内联 top-ranked facts，让常见问题就地回答。两个 surface 都不会把
整个 fact pack、reference 文档或生成 skill 正文塞进 prompt。

稳定展开命令会一直保留：

```bash
lilygo-skills source query --board <board-id> --topic io --json
lilygo-skills verify sources --board <board-id> --json
lilygo-skills context --json "<prompt>"
```

不完整的 starter board pack 也遵循这个规则。它可以暴露 `unknown_with_sources` 加官方
reference，让 Agent 知道下一步去哪查；但不能为了填满 capsule 而编造引脚、外设或运行
行为。

thick capsule 还会内联少量关键的 source-recovery 指针：板子自己的源码 header 和 driver
事实。比如 T-Display-S3 的 TFT_eSPI + I2C prompt 可以暴露 Setup206、`pin_config.h`、
`PIN_IIC_SDA=GPIO18`、`PIN_IIC_SCL=GPIO17` 和 `source query` 命令，而不是把整个 fact
pack 注入 prompt。

## Preferences 和 References

Preferences 是公开行为偏好，例如框架顺序、串口调试工具、代码行数限制和安全默认。
References 是公开阅读提示，例如官方示例、源码、datasheet、硬件说明和项目设计文档。

两者都不能覆盖官方 source facts。板子或 topic 缺少关键事实时，应先返回
`unknown_with_sources`，而不是把 reference 当作已经 ready。

## 安装态

`node install.js --all` 挂载 runtime，没有编译步骤。安装器把 Node dispatcher(`bin/**`)
和数据模型(`data/**`)作为一个自包含 runtime 一起复制到 `~/.claude/lilygo-skills/` 和
`~/.codex/lilygo-skills/`，接好 Codex/Claude 入口和 `UserPromptSubmit` hook，并安装
router Skill。`--build` 出于向后兼容仍可接受，但是 no-op；JS dispatcher 不需要编译任何
东西。

`doctor --json` 会从当前 HOME 验证安装态 runtime 数据模型：runtime 数据文件齐全、board
registry 和 fact packs 匹配、V3 evidence 覆盖、sniff matchers 可加载，并返回一份
`sample_injection` capsule 确认注入链路。

支持模型按 board family 扩展。当前已验证 runtime 覆盖从 LilyGO ESP32 系列开始。
没有对应 V4/V5 证据时，不把 source/context 结果表述成已经完成的硬件行为。
