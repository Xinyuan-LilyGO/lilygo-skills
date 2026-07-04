# 板级事实

English version: [BOARD_FACTS.md](BOARD_FACTS.md)。

Board facts 是 AI 写固件或调试步骤前可以使用的 source-backed claim。它们和 route
skills 分开存放，这样默认上下文可以保持紧凑。

## Facts 存放位置

```text
data/facts/board-fact-packs.json
```

每个 pack 以 `board_id` 为 key，可以包含：

- `mcu_family`
- `supported`
- `pin_matrix`
- `bus_matrix`
- `expander_matrix`
- `connector_matrix`
- `peripheral_table`
- `source_refs`
- `conflicts`

每条 fact 都应携带 claim、value、topic、source kind、source URL 或 portable
reference、source hash、authority rank、evidence level、stale flag 和 confidence。

## 查询 Facts

```bash
cargo run -p lilygo-skills-cli -- source query --board board-t-watch-ultra --topic io --json
cargo run -p lilygo-skills-cli -- source query --board board-t-watch-ultra --topic expander --json
cargo run -p lilygo-skills-cli -- source query --board board-t-watch-ultra --topic peripheral --json
```

有效 topic 包括 IO、pinout、bus、expander、connector、peripheral、display、IMU、
power、LoRa、GNSS 和 input。

## Confidence Values

- `exact`：值被 source 直接证明。
- `derived`：值由 source-backed metadata 推导。
- `unknown_with_sources`：当前 source 能证明 topic 存在或相关，但不能证明精确值。

`unknown_with_sources` 是有意设计的状态。它比编造空闲 GPIO、扩展 IO 通道或总线连接安全。

## Completeness

Completeness 按 board + topic 评估：

```bash
cargo run -p lilygo-skills-cli -- source completeness --board board-t-display-s3 --topic display --json
```

状态：

- `complete`：quick-start contract 需要的事实和 reference 已足够。
- `partial`：已有部分事实，但缺少关键事实。
- `needs_source_ingestion`：支持的 board/topic，但本地 fact pack 需要从官方 source 补充。
- `unsupported`：超出当前支持边界。

Route、hook 和 goal plan 可以暴露紧凑 readiness 状态，但不能写 fact pack。

## Enrichment

显式 update 命令用于 enrichment：

```bash
cargo run -p lilygo-skills-cli -- update board-facts \
  --board board-t-display-s3 \
  --topic display \
  --dry-run \
  --json
```

Dry-run 输出应该包含 source adapters、planned reads、planned writes、parsed facts /
unknowns、source hashes、validation status 和 follow-up commands。

去掉 `--dry-run` 后只能写入已支持且通过验证的内容。不支持的板子或当前 LilyGO 支持范围外
的目标必须 fail closed，不能修改 fact pack。

## Source Authority

权威顺序：

1. 官方代码、headers、examples 和产品仓库。
2. 官方 LilyGO hardware docs。
3. `Xinyuan-LilyGO/documentation`。
4. `wiki.lilygo.cc` fallback pages。
5. 项目 reference skills。
6. 辅助 community/tool references。

当 source 冲突时，更高权威 source 胜出，冲突应保留在 fact pack 或 `source query`
输出中。

## Context Budget

Fact pack 默认不整包注入。Route 和 hook 输出应该只包含 matched skill ids、短摘要、
top-ranked facts、overflow counts，以及 `source query`、`source completeness` 或
`update board-facts` 命令。

当用户要求实现细节、引脚分配、外设行为或调试时，AI 应调用 lookup commands。

## Privacy Boundary

Board facts 是公开 source facts。它们不能包含本地串口、Wi-Fi 值、OTA host、本地日志路径
或私有机器证据。这些内容属于 ignored project-local state，不应注入公开上下文。
