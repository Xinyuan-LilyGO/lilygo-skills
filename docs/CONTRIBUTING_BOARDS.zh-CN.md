# 新增或改进板子

English version: [CONTRIBUTING_BOARDS.md](CONTRIBUTING_BOARDS.md)。

本指南用于新增或改进 LilyGO board support。

## Support Boundary

支持模型会逐步覆盖 LilyGO boards。Runtime 覆盖从 LilyGO ESP32 系列产品开始：

- ESP32
- ESP32-S2
- ESP32-S3
- ESP32-C3
- ESP32-P4

其他 LilyGO 产品先进入 source candidate flow。补充公开资料、board-family metadata
和 evidence requirements 后，build、flash、OTA 和硬件调试指导会通过同一套
source-backed support flow 展开。

## Add Or Improve A Board

1. 确认精确 product id、aliases、MCU family 和支持框架。
2. 按权威顺序收集 source refs：官方 repo/code/examples 优先，其次官方硬件文档、
   documentation repo、wiki fallback 和本地 reference patterns。
3. 新增或刷新 board source metadata。
4. 生成或更新 compact board skills。
5. 在官方 source 能证明时，为 IO、pinout、bus、expander、connector、peripheral 和
   quick-start topics 添加 source facts。
6. 在 `eval/**` 下添加 context/verification fixtures 和 negative over-injection cases。
7. 运行 verification suite。

Board 和 fact-pack 数据现在随 skill 目录一起分发，由 official-source pipeline 生成。
重新生成并 diff 数据：

```bash
node pipeline/run-official-source-pipeline.js --all-boards --json
node pipeline/diff-gold-fact-packs.js
```

Pipeline 默认是 dry 的（只把 plan 写到 `.tmp/pipeline/`）；只有确认 diff 正确后再加
`--write`，才会写入 `data/facts/**`。提交前先检查 JSON plan，不要手改 generated fact
packs。

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
应调用 `source query` 或 `verify sources`。

## Tests And Smokes

最低要求：

```bash
npx tsc --noEmit
npm test
bash scripts/ci-gate.sh
git diff --check
```

新增和既有 family trigger 重叠的板子时，添加 exact-board precedence regression，避免
`context` output 把误导性的 generic board 当作 selected context。

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
