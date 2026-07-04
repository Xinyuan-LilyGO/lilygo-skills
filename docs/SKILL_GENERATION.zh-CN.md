# Skill 生成

English version: [SKILL_GENERATION.md](SKILL_GENERATION.md)。

本项目发布一个小型 meta Skill 和一个确定性生成器。源码树不提交已经生成的板子、
芯片、外设、框架、playbook、debug、app 或 recipe Skill 快照。

## Source Inputs

| 路径 | 作用 |
|------|------|
| `skills/lilygo-router/SKILL.md` | Agent 加载的 meta 入口。 |
| `skills/references/*.md` | 运行上下文的静态展开文档。 |
| `templates/skills/*.md` | 生成 Markdown Skill 文件的模板。 |
| `index/routes.json` | Skill registry 和 route trigger。 |
| `data/boards.json` | 板子和产品 source model。 |
| `data/facts/**` | source-backed 板级事实和 completeness 数据。 |
| `data/peripherals/**` | 外设、芯片、功能 source pack。 |
| `data/playbooks/**` | generated playbook source model。 |
| `data/skills/reference/**` | reference practice skills 的源 Markdown。 |

## Generated Output

```text
<out>/skills/<skill-id>/SKILL.md
<out>/skills/references/*.md
<out>/templates/skills/*.md
<out>/index/routes.json
```

只有包含 `SKILL.md` 的目录会成为 routed generated skill。`skills/references/`
只是支持展开的目录，不会变成 skill。

## 自然语言触发

用户不需要记住生成命令，可以直接说：

```text
在当前固件仓库初始化 LilyGO Skill。我用 T-Display-S3，框架是 PlatformIO。
```

Agent 应该把它落到项目级初始化：

```bash
lilygo-skills project init --project . --board board-t-display-s3 --framework fw-platformio --json
```

这会写入可提交的 `.lilygo-skills/project.json`，并生成 ignored 的
`.lilygo-skills/generated-skills/`。

如果用户说：

```text
重新生成这个项目的 LilyGO skills，并检查是否完整。
```

Agent 应该只写 generated cache，不改源码树：

```bash
lilygo-skills generate skills --out .lilygo-skills/generated-skills --json
lilygo-skills verify --generated-root .lilygo-skills/generated-skills --json
```

route 和 hook 不会因为普通问题自动生成文件。它们只报告需要的 layer、缺失项和可执行
的 generate/update 下一步；真正写入必须来自用户明确的安装、初始化、生成或更新请求。

## Template Shape, Source Content

生成文件形状稳定，因为 CLI 从 `templates/skills/*.md` 渲染它们。有用内容不在模板里，
而是来自 `data/boards.json`、`data/facts/**`、`data/peripherals/**`、
`data/playbooks/**`、`data/recipes/**`、`index/routes.json` 和官方 source references。

Generated chip skill 有意保持窄：只为真实芯片标识生成。类似 `SX1262 or SX1280` 的复合
描述、容量标签和存储介质保留在板级/外设事实和 `source query` 输出中。这样 chip 层保持
精确，同时 `goal plan`、`goal complete` 和 `source query` 仍能暴露相关事实。

例如 `board-t-watch-ultra` 使用共享 board template，但内容包含 T-Watch Ultra 的具体事实：
ESP32-S3、Arduino FQBN、AMOLED `CO5300`、触摸 `CST9217`、GNSS `MIA-M10Q`、
radio option `SX1262 or SX1280`、NFC `ST25R3916`、IMU `Bosch BHI260AP`、
电源 `AXP2101`、RTC `PCF85063A`、震动 `DRV2605`、扩展 IO `XL9555`、SD、内存、
官方 LilyGoLib 文档、driver header 和 demo path。

查看具体生成内容：

```bash
lilygo-skills generate skills --out .tmp/generated-skills --json
sed -n '1,220p' .tmp/generated-skills/skills/board-t-watch-ultra/SKILL.md
lilygo-skills goal plan --json "T-Watch Ultra Arduino LVGL touch does not move"
lilygo-skills source query --board board-t-watch-ultra --topic io --json
```

Board Skill 是紧凑快照。更丰富的任务上下文在 `goal plan.context_capsule`、
`source query` 和 `source completeness` 中，这样默认注入保持小，但实现或调试时仍能获取
source-backed facts 和官方 demo references。

## Template-Driven Renderers

当前由模板渲染：

- `templates/skills/board.md`
- `templates/skills/peripheral.md`
- `templates/skills/playbook.md`

已提交但作为公开 shape contract 保留：

- `templates/skills/reference.md`
- `templates/skills/framework.md`

每个模板渲染文件都包含：

```text
Generation Contract: templates/skills/<kind>.md
```

## Verification

```bash
lilygo-skills generate skills --out .tmp/generated-skills --json
lilygo-skills verify --generated-root .tmp/generated-skills --json
bash scripts/static-context-template-smoke.sh --dry-run
bash scripts/meta-only-source-smoke.sh
```

生成属于 V3 source/context 证据。硬件行为仍然需要自己的 build、simulator、flash、
serial、OTA、RF/GNSS、display 或外设证据。
