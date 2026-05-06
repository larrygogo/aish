# 0001. 用 ADR 记录架构决策

- **Status:** Accepted
- **Date:** 2026-05-06

## Context

aish 在设计阶段做了多个绑定项目长期方向的技术选择（GUI 框架、SSH 库、tmux 集成方式等）。如果这些理由只散落在 commit message 或聊天记录里，半年后回看会无法理解"为什么当初选了 X 而不是 Y"，更不可能让贡献者快速接住上下文。

## Decision

采用 [Michael Nygard 风格的 Architecture Decision Records](https://cognitect.com/blog/2011/11/15/documenting-architecture-decisions)，每条决策一个 markdown 文件，存放于 `docs/adr/`，编号递增。每个 ADR 包含：

- **Status**：Proposed / Accepted / Deprecated / Superseded by ADR-N
- **Context**：决策背景，为什么需要做这个决策
- **Decision**：实际选择是什么
- **Consequences**：选择带来的好处与代价

## Consequences

- 好处：决策可追溯，新贡献者可以从 0001 开始顺序读取了解整个系统脉络
- 代价：每个重要决策需要花 5-10 分钟写文档；不重要的决策不应该走 ADR（避免噪音）
