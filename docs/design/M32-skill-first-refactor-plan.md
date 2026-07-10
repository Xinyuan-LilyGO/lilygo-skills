# M32 · Skill-first 大重构方案(req-change 已立项,待"开干")

日期:2026-07-10 · 状态:**req-change/design/review/测试协议齐备,未开工** · 分支:m25-local-integration

## REQ-CHANGE 记录(M32 立项)

- REQ-M32-1(用户):架构简化为 skill 驱动,天然多平台;调试机制不依赖 Rust 代码,交给 SKILL.md。
- REQ-M32-2(用户):router 用 Rust,保持"简单的 Rust";单二进制、既有核心测试与打磨保留。
- REQ-M32-3(用户):保留并可证明我们相对官方 assistant 的特质(§4.6 六条,各配证明方式)。
- REQ-M32-4(用户):真实效果必须可测可证(§7 测试协议;coverage 基线禁倒退)。
- REQ-M32-5(用户,本次):router 的"选择智能"(选哪些子 skill/playbook)应交给 AI;
  Rust 只留确定性部分(相关性闸门/认板/诚实边界)。
- 流程:本文档 = req-change + design;§6 = review 清单;§7 = 效果测试协议。
  提交时机:R0(打 tag `pre-M32` 时本文档随之入库)。

## 0. 触发与共识

- 用户结论:官方同组织出了 [lilygo-assistant-skill](https://github.com/Xinyuan-LilyGO/lilygo-assistant-skill)
  (3 个文件:SKILL.md + 170 行 mcp.mjs + README,纯 skill 形态),对比之下我们太复杂。
- 方向:**简化成 skill 为主 → 天然多平台(Claude Code / Codex / 任意 agent)**;
  Context Kernel(确定性上下文获取)保留;**调试机制不再依赖 5K Rust,交给 SKILL.md 散文**。
- 事实:官方 assistant 的 description 几乎逐字来自我们 router 的 SKILL.md,是我们的轻量衍生。
  它把数据放在托管 MCP 服务(Railway);我们的数据在本地、带 sha256 官方源背书 + 门禁。

## 1. 目标架构(= 官方的形态 + 我们的内核)

| 部件 | 现状 A | 目标 |
|---|---|---|
| SKILL.md | 描述+行为规则(已加强) | **操作系统**:查询协议、调试循环、诚实规则,全部散文(取代 goal/ 5K) |
| CLI | 30 个子命令 | **收敛到 ~4 个**:`context`(新,Context Kernel 一击:CWD→板→胶囊≤1KB,自动获取全部上下文)、`source query`(pull)、`list`、`doctor`(瘦) |
| hook | 全量胶囊注入 | **可选薄适配器**:一行调 `context`,Claude Code 想要确定性就装;其他平台纯 skill |
| 数据 | data/ + JS pipeline + 门禁 | **不动**(护城河:本地、sha256 官方源、equivalence/authority 门禁、离线可用) |

## 2. 砍单(相对当前 20,524 行,staged,每步全绿)

| 模块 | 行 | 处置 |
|---|---|---|
| goal/(plan/recipes/playbooks) | 5,048 | → SKILL.md 散文;**被 coverage 评分的 expand/source-query 指针由 `context` 原生承接(硬约束)** |
| 生成栈 generate+product_source+peripheral+source_gen | 2,607 | 删(skill 静态化;product_source 已近孤儿;与 JS pipeline 重叠) |
| project_ledger | 1,253 | 删(hook-only 状态跟踪) |
| benchmark | 1,108 | 移出交付面(或删,保 eval/ JS 门禁) |
| commands/ 30→4 子命令 | ~1,500→~400 | 大瘦身 |
| 各模块随行测试 | ~含在上 | 同步去 |

估计:20.5K → **~7-8K**(生产码 ~5K:source 614 + facts 1,879 + router 1,396 + registry 469 + context/commands ~700)。

## 3. 效果保证(不许拍脑袋,全部可测)

1. **coverage-gate 改评 `context` 输出**:covered ≥ 55/62 基线,**禁止倒退、禁止改基线**。
   goal 削掉后,胶囊里被评分的指针(expand/next 等)必须由 `context` 复现——这是迁移的硬约束,
   M29 已证明 gate 抓得住这种回归(当时想删 `next` 被 gate 拦下)。
2. **数据门禁不动**:verify-auto-mapping / verify-source-authority / ingest 等价 / 48 ci-gates / fmt / clippy。
3. **skill-pull A/B(本机 claude -p,quota 已恢复)**:同一组板级任务,
   A = hook+skill(现状) vs B = 纯 skill(禁 hook)。量两个数:模型**真的调了 CLI 吗**、**引脚答对率/不乱编**。
   B≈A → 放心把 hook 降级为可选;B≪A → hook 适配器保留为默认推荐。
4. 诚实标记(hardware_verified=false / evidence_boundary)全程保留。
5. P4 真机 runner A/B 仍是最终判据(需用户凭据,不伪造)。

## 4. 工作包(每个独立提交、独立可回滚,砍前打 tag)

- WP0:打 tag `pre-M32`;分支继续 m25-local-integration。
- WP1:新增 `lilygo-skills context`(复用 router/facts/source 核心,输出=今日胶囊等价物+指针),
  coverage-gate 切到 context 输出,**绿**(此时零删除)。
- WP2:SKILL.md 重写为操作文档(查询协议/调试循环散文,吸收 goal 的知识),对齐官方 assistant 形态。
- WP3:hook 改薄适配器(内部就是调 context,输出不变),**绿**。
- WP4:A/B 实测(claude -p × N 任务,A vs B),出数字。
- WP5:按 A/B 结果分阶段砍:goal → 生成栈 → ledger → benchmark → 子命令收敛,**每砍一刀全绿再砍下一刀**。
- WP6:README/README.zh 重写 + 安装脚本更新 + 两机重部署。

## 4.6 Plan R(最终定调:router 用 Rust,2026-07-10 用户三点)

用户定调:① router 用 Rust;② 保留我们的特质(与官方的区别和提升);③ 真实效果必须可证。
Plan S 的 JS 重写作废,修正为 **Rust 精简版**:

**形态:单 Rust binary,4 个子命令**(context / source query / list / doctor-lite)+ SKILL.md 操作文档 + data + JS pipeline。

**保留集合(实测行数,含内联测试):** source.rs 614 + facts/ 1,879 + router/ 1,396 +
registry 469 + text_match 71 + project_context 268 + main 38 = **4,735**;
加瘦身 commands(~400)+ context 装配(~200)≈ **~5.3K 总(生产码 ~3.8K)**。
相比现状 20,524 = **−74%**。砍单不变:goal 5,048 / 生成栈 2,607 / ledger 1,253 /
benchmark 1,108 / commands 30→4。

**为什么 Rust 而不是 JS(比 Plan S 多 ~3.5K 换回):**
单二进制零依赖(不要求 Node)、核心模块的既有测试继续有效、CJK 匹配与字节预算打磨保留、
coverage-gate 现成对接(它本来就是调 Rust binary)。

**认板增强(诚实缺口):**今天认板主要靠 `.lilygo-skills/project.json` profile;
Plan R 给 `context` 补 platformio.ini / sdkconfig / *.ino 嗅探,无 profile 也能认板(小增量,~100 行)。

**我们 vs 官方 assistant 的特质(每条都有证明方式):**
| 特质 | 我们 | 官方 assistant | 证明 |
|---|---|---|---|
| 数据可验证 | 每 pin 官方源 URL+行号+sha256 | 托管 MCP 黑盒 | 仓库记录+门禁 |
| 离线+速度 | 本地毫秒、断网可用 | 依赖 Railway 服务存活 | 断网 demo |
| 自动认板 | context 从项目文件自动知道板子 | 用户必须自己报板名 | A/B 任务集 |
| 质量门禁 | coverage 基线+等价+权威门禁 | 全仓 4 文件,0 测试 0 门禁 | 仓库事实 |
| 数据自增长 | manifest+auto_pins pipeline | 服务端黑盒 | pipeline 演示 |
| 诚实标记 | 机器可查(hardware_verified 等) | 仅散文规则 | gate 断言 |

**真实效果证明(③):A/B/C 三组同任务集**(claude -p,板级问答+调试任务):
A=我们 hook+skill / B=我们纯 skill / C=官方 assistant-skill。
量:引脚答对率、不乱编率、是否真调了工具。C 组给出"我们 vs 官方"的直接数字;
coverage 基线 55 禁倒退;P4 真机仍是最终判据。

## 4.5 彻底版 Plan S(已被 4.6 取代,存档:JS 重写路线)

第 1-4 节的 ~7-8K 是保守估计:把整个 Rust 内核当"不可动"。但按"skill 驱动、
核心=router(认板→给对应 skill/facts)"收敛,这个职责小到不需要 Rust crate:

**目标形态(和官方 assistant 同构,数据内核是我们的):**

```
SKILL.md                    操作文档(散文:查询协议/调试循环/诚实规则)
scripts/query.mjs   ~350 行  router 本体:认板(platformio.ini/sdkconfig/*.ino/参数)
                            → 输出该板 facts 胶囊;query <board> <topic> = source query
data/*.json                 事实包(JS pipeline 生成,sha256 官方源背书)
pipeline/*.js        674 行  数据生成+门禁(不变)
eval/*.js            578 行  coverage-gate 改评 query.mjs 输出,基线 55 不变
```

**代码总量:20,524 → ~1.8-2K(-91%)。Rust crate 整体退役**(打 tag 归档,A/B+coverage
证明等价后才删)。

**诚实代价(必须认):**
1. 214 个 Rust 测试作废 → 为 query.mjs 补 node --test(~100 行);
2. 48 个 ci-gate 中 Rust 行为门禁作废 → 门禁套件围绕 JS 重建(数据门禁全保);
3. 单二进制安装 → 依赖 Node(官方 assistant 同样要求,可接受);
4. CJK 分词/字节预算等打磨 → keyword-rules.json 数据已在,query.mjs 需最小移植;
5. hook 仍可选:薄 hook 一行调 `node query.mjs`(Claude Code 要确定性就装)。

**执行序(每步绿):** S1 写 query.mjs+测试,coverage-gate 切换,基线 55 达标 →
S2 SKILL.md 围绕 query.mjs 重写 → S3 A/B(hook+skill vs 纯skill) →
S4 Rust 退役(tag 归档,ci-gate 重建) → S5 README/安装/两机重部署。

## 4.7 router/facts 再精简(REQ-M32-5,实测拆解)

**router 为什么 1,396 行?** 实测:mod.rs 1,327 = 生产 587 + **测试 740(56%)**+ tools.rs 69。
生产 587 里两类:

| 类别 | 内容 | 行(约) | Plan R 处置 |
|---|---|---|---|
| **确定性核心(必须留 Rust)** | LilyGO 相关性闸门(has_lilygo_signal/keyword-rules 数据)、认板/板名覆盖 profile、硬件诚实边界(needs_hardware_boundary)、noop | ~230 | 保留 |
| **选择智能(交给 AI)** | 选哪些 periph/playbook/derived 子上下文、family/prefix fallback 抑制、framework 澄清问题、skill 排序 | ~360 | 删,由 SKILL.md 引导 AI 自选(facts 索引作数据) |

→ router 砍到 **~250 生产行 + ~250 测试 ≈ 500**(原 1,396,−64%)。
"AI 自己判断是否相关/自己选自己"不行的三样必须确定性:相关性闸门(不然乱注入)、
认板(事实身份)、诚实边界(不能让模型自查自纠)——其余选择全交 AI。

**facts/ 同拆**:build 486(胶囊装配,瘦到 ~300)+ completeness 674(审计面,
**评审决定去留**,倾向移出交付面)+ mod 319 生产。→ facts ~1,879 → **~800-1,000**。

**Plan R 总量修正:20,524 → ~4.3K(−79%)**(source 614 + facts ~900 + router ~500 +
registry 469 + text_match 71 + project_context+嗅探 ~370 + commands ~400 + context ~200 + main 38)。

## 6. Review 清单(每刀过闸)

- [ ] coverage-gate(评 `context` 输出)covered ≥ 55,基线文件未改;
- [ ] 数据门禁:verify-auto-mapping / verify-source-authority / ingest 等价 全 PASS;
- [ ] cargo test(保留集)/ fmt / clippy 全绿;ci-gate 重建后 gates 数记录在案;
- [ ] 诚实标记(hardware_verified/evidence_boundary)在 context 输出中断言存在;
- [ ] 每刀独立 commit + 可回滚;砍单外文件零改动(git diff 审计);
- [ ] eval/tasks.json 与 graders 未被触碰;
- [ ] README/安装文档与新形态一致;两机部署验证。

## 7. 效果测试协议(REQ-M32-4,写死)

**T1 注入侧(自动,每刀跑):** coverage-gate 评 `context` 输出,covered ≥ 55/62;胶囊 ≤1KB。

**T2 端到端 A/B/C(claude -p,同一任务集 ≥12 题:引脚问答/接线/烧录/调试各若干,中英混合):**
- A = 我们 hook+skill(现状);B = 我们纯 skill(禁 hook);C = 官方 assistant-skill。
- 指标:引脚答对率(对照 sha256 官方源真值)、不乱编率(错误值=乱编)、工具真调率。
- 通过标准:B 引脚答对率 ≥ C 且 B 不乱编率 ≥ C(证明特质);B ≈ A(差 ≤1 题)才把 hook 降为"可选"叙事,否则 hook 保持默认推荐。
- 记录:eval/ab-c-results.json + 原始 transcript 归档,不许挑样本。

**T3 断网特质证明:** 断网跑 `context`/`source query` 正常、官方 C 组失败——录屏/日志留证。

**T4 最终判据:** P4 真机 runner A/B(需用户凭据,不伪造不替代)。

## 8. 文档驱动引导(REQ-M32-6,2026-07-10 用户:"多一些文档,引导这些,类似 dev-flow")

原则:像 dev-flow 一样"**薄代码 + 厚引导文档**"。R5b/R5d 砍掉的 playbooks/recipes 选择
逻辑,其**知识不能丢**——要在删代码**之前**沉淀成一套 AI 读着就能执行的引导文档。
效果由三重保住,不靠被删的代码:①**引导文档**(AI 知道"怎么做")②**coverage 门禁**
(注入事实不倒退)③**A/B/C 实测**(证明真效果)。

### 引导文档集(SKILL.md 为入口,指向 guides/)

放在 `skills/lilygo-router/guides/`,SKILL.md 与 `context`/`source query` 的 expand 指针指向它们:

- `query-protocol.md` — 拿 context → 拉 source 的操作协议
- `debug-display-bringup.md` — 屏点亮:ST7789/TFT_eSPI Setup / ESP-IDF i80 / 背光电源
- `debug-lora-gnss.md` — SX126x/RadioLib + GNSS 上手与排错
- `debug-power-battery.md` — 电源轨/充电/电量
- `debug-flash-serial.md` — build/upload/monitor 有界步骤 + 失败归类
- `debug-lvgl-loop.md` — LVGL 刷新/触摸循环排错
- `board-bringup-checklist.md` — 新板从零上手清单(认板→源→demo→证据)
- `toolchain-setup.md` — Arduino/PlatformIO/ESP-IDF/Rust 工具链
- `honesty-evidence.md` — 证据等级/hardware_verified/不乱编 的判定细则

这些是现有 recipe-*/playbook-* 的**散文版**。沉淀成文档后:(a) 保住效果与知识;
(b) 解锁把 Rust `playbooks.rs`/`recipes.rs` 进一步瘦成"仅 ID 输出"甚至挪成 JSON 数据。

### 顺序(关键:先沉淀知识,再砍代码)

R5c(ledger)→ **R-docs(写引导文档集,从现有 recipe/playbook 代码提炼,不得杜撰)** →
R5d(激进砍 capsule/死模块/model/router,此时知识已在文档里)→ R4 A/B/C → R6。
R-docs 与 R5d 不并行(同仓冲突);R-docs 纯新增文档,gate 只需 ci-gate/coverage 不倒退。

## 5. 风险与诚实边界

- coverage 基线由 goal 供给的 3 个指针事实:迁移不成 = 不许砍 goal(gate 说了算)。
- 官方 assistant 的托管 MCP 是另一条数据道路;我们不抄它(provenance 不可验、不可离线),
  但 SKILL.md 可提示"两者可共存"。
- 砍单执行期任何一步红 → 回滚该步,不硬推。
