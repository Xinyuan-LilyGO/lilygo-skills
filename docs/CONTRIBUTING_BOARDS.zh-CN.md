# 新增或改进板子

English version: [CONTRIBUTING_BOARDS.md](CONTRIBUTING_BOARDS.md)。

本指南用于新增或改进 LilyGO board support。

## Support Boundary

支持模型目标是逐步覆盖 LilyGO boards。当前已验证 runtime 覆盖从 LilyGO ESP32
系列产品开始：

- ESP32
- ESP32-S2
- ESP32-S3
- ESP32-C3
- ESP32-P4

其他 LilyGO 产品可以记录为 source candidate，但在完成设计和验证前，build、flash、
OTA 和 hardware-debug guidance 必须保持 unsupported。

## Add Or Improve A Board

1. 确认精确 product id、aliases、MCU family 和支持框架。
2. 按权威顺序收集 source refs：官方 repo/code/examples 优先，其次官方硬件文档、
   documentation repo、wiki fallback 和本地 reference patterns。
3. 新增或刷新 board source metadata。
4. 生成或更新 compact board skills。
5. 在官方 source 能证明时，为 IO、pinout、bus、expander、connector、peripheral 和
   quick-start topics 添加 source facts。
6. 为应当 quick-start ready 的 topic 添加 completeness gate。
7. 添加 route fixtures 和 negative over-injection cases。
8. 运行 verification suite。

常用命令：

```bash
cargo run -p lilygo-skills-cli -- sync-boards --dry-run --json
cargo run -p lilygo-skills-cli -- update boards --dry-run --json
cargo run -p lilygo-skills-cli -- update skills --dry-run --json
cargo run -p lilygo-skills-cli -- update fact-packs --dry-run --json
cargo run -p lilygo-skills-cli -- update board-facts --board <board-id> --topic <topic> --dry-run --json
cargo run -p lilygo-skills-cli -- update source-packs --dry-run --json
cargo run -p lilygo-skills-cli -- update peripheral-skills --dry-run --json
```

只有当 planned writes 正确且位于支持路径内时，才去掉 `--dry-run`。`update skills` 和
`update peripheral-skills` 只能把 generated runtime skills 写入
`.lilygo-skills/generated-skills/` 或 `--out <generated-root>`，不能写入 source
`skills/` 树。

## Fact Quality Rules

- 只有高权威 source 直接证明的值才使用 `exact`。
- 值由 source-backed metadata 推导时使用 `derived`。
- 相关 source 存在但不能证明精确值时使用 `unknown_with_sources`。
- 不要根据产品名猜测空闲 GPIO、扩展 IO 通道、电源 rail、display bus 或 touch controller。
- 保留 source refs 和 hashes，方便未来 update 检测 stale facts。

## Skill Quality Rules

Generated skills 应保持紧凑：

- Trigger terms 和 aliases。
- 板子、芯片或框架的用途。
- 高价值 source pointers。
- 深入查询命令。
- Verification boundary。

不要把完整 datasheet、长源码文件或完整 fact pack 粘到 `SKILL.md` 里。AI 需要更多信息时，
应调用 `source query`、`source completeness`、`index query` 或 `reference list`。

## Tests And Smokes

最低要求：

```bash
cargo test -q -p lilygo-skills-cli
cargo run -q -p lilygo-skills-cli -- verify --json
cargo run --release -q -p lilygo-skills-cli -- benchmark --json --iterations 5000
bash scripts/source-completeness-smoke.sh --dry-run
bash scripts/board-completeness-smoke.sh --dry-run
bash scripts/full-evidence-smoke.sh --dry-run
git diff --check
```

新增和既有 family trigger 重叠的板子时，添加 exact-board precedence regression，避免
route output 把误导性的 generic board 当作 selected context。

## Documentation Checklist

行为变化时同步更新：

- `README.md`
- `README.zh-CN.md`
- `ARCHITECTURE.md`
- `ARCHITECTURE.zh-CN.md`
- `docs/BOARD_FACTS.md`
- `docs/BOARD_FACTS.zh-CN.md`
- `docs/VERIFICATION_LEVELS.md`
- `docs/VERIFICATION_LEVELS.zh-CN.md`

Release-process 变化需要在同一次 change 中更新对应 public docs、smoke gates 和
verification commands。
