# 0006. GPUI 与 tokio runtime 共存，用 channel 桥接

- **Status:** Accepted
- **Date:** 2026-05-06
- **依赖于:** 0002, 0004

## Context

GPUI 有自己的 executor（BackgroundExecutor / ForegroundExecutor）。但 russh 强依赖 tokio runtime（其 async I/O、定时器、TLS 等都跑在 tokio 上）。两者必须共存。候选：

- **完全跑在 GPUI executor 里**：要把 russh 改成 GPUI executor 兼容，工程量巨大且不可持续
- **完全跑在 tokio 里**：UI 线程必须是 GPUI 主线程，无法替代
- **两者共存，用 channel 桥接**：Zed 自己的做法

## Decision

`aish-app` 启动时在专属线程跑一个 `tokio::runtime::Builder::new_multi_thread().build()`。所有 async 调用通过 `runtime.spawn()` 提交，结果通过 `tokio::sync::oneshot` 或 `tokio::sync::mpsc` channel 回到 GPUI。GPUI Model 用 `cx.spawn(|this, mut cx| async { ... })` 接收 channel 并 update Model。

## Consequences

**好处：**
- 两个 executor 各司其职，互不干扰
- channel 隔离了 lifetime / Send 复杂度（避免在 GPUI 内部直接 await tokio future）
- Zed 已用此方案，是经过验证的模式

**代价：**
- 多一层 channel 间接，需要小心背压（unbounded channel 可能 OOM）
- Debug 时 stack trace 跨 runtime 边界，定位问题麻烦
