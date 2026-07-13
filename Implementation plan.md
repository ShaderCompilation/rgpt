# Feasibility & Plan: Rust Port of shell_gpt

## Context

The user asked how easy it would be to build a Rust alternative to `shell_gpt` (the Python CLI at `/home/hayk/side-projects/terminal/shell_gpt`), with three explicit goals: better UX, lower resource usage, and better optimization for smaller/local LLMs (e.g. via Ollama). This document answers the feasibility question and lays out a concrete, phased approach so the user can decide whether/how to proceed.

**Verdict: easy-to-moderate.** The Python codebase is small — **1,494 lines across 20 files** (plus 1,209 lines of tests) — and architecturally it's a thin layer: parse CLI flags → build a message list → call one HTTP endpoint → stream tokens → optionally persist to a JSON file. Config, roles, chat history, and caching are all simple JSON/flat-file formats that map almost 1:1 onto Rust with `serde`/`clap`. This is a realistic solo side project, buildable incrementally in phases where each phase is independently useful — roughly **4-8 weekends** for a solid MVP (Phases 1-5 below).

Two parts are genuinely hard and are called out explicitly rather than discovered mid-build:
1. **Rich's live-updating markdown+syntax-highlighted terminal rendering** (`rich.live.Live` re-rendering the full buffer on every streamed chunk) has no drop-in Rust equivalent — expect to hand-roll a live-region redraw using `pulldown-cmark` + `syntect` + `crossterm`.
2. **Dynamic user-authored tool loading** (drop a `.py` file in a folder, it becomes an LLM-callable tool via `importlib` + Pydantic schema reflection) has no Rust equivalent at all — a scripting/plugin runtime would be required to replicate it.

## Scoping decisions (confirmed with user)

- **Platform**: Linux/macOS only for v1 — no Windows tty/PowerShell/cmd.exe handling.
- **Extensibility**: fixed, hardcoded built-in tools only (e.g. `execute_shell_command`). No plugin system in v1 — this sidesteps the one feature with no Rust equivalent.
- **Config/migration**: fresh config directory (e.g. `~/.config/<new-name>/`), not compatible with `~/.config/shell_gpt/`. Clean break, no shims.
- **HTTP client**: minimal blocking client (`ureq`), not async. Simpler code, smaller binary/dependency tree; fits a mostly request/response CLI. Revisit only if concurrency (e.g. parallel tool calls) becomes a real need.

## v1 MVP scope

**Include:**
- Single-shot prompt/response with streaming
- Config file + env var overrides, self-bootstrap on first run (prompt for API key)
- Default roles (general assistant, shell-command generator, shell-command describer, code generator) + custom role create/show/list
- Persistent named chat sessions + REPL mode
- `--shell` generation with Execute/Modify/Describe/Abort confirmation loop
- `--describe-shell`, `--code` modes
- Response caching (hash-keyed files, pruned by count)
- Piped stdin support, including the `__sgpt__eof__`-style sentinel for combined piped+interactive input
- `$EDITOR` prompt composition
- OpenAI-compatible HTTP client (covers OpenAI, Ollama's `/v1`, LM Studio, vLLM, llama.cpp server)
- Native Ollama client (`/api/chat`) for Ollama-specific options (`num_ctx`, `num_predict`, `keep_alive`)
- Fixed built-in tool calling (shell exec) with an iterative (not recursive) tool-call loop
- Markdown rendering (start with simple full-buffer print, polish to live-region later)

**Explicitly excluded (deliberate cuts, not gaps):**
- LiteLLM-style 100+ provider support — no Rust equivalent; OpenAI-compatible HTTP covers the local-model use case that actually matters here.
- Dynamic user-authored tool loading — built-in tools only.
- Shell integration installer (Ctrl-L keybinding in `.zshrc`/`.bashrc`) — trivial, can be added anytime, not core to the port's value.
- Windows support.
- Pixel-perfect Rich rendering fidelity.

## Architecture

```
src/
  main.rs            // entry point, arg parsing dispatch
  cli.rs             // clap derive struct, mirrors app.py's ~20 flags
  config.rs          // KEY=value config file, env override, bootstrap prompt
  role.rs            // SystemRole struct, default roles, JSON persistence
  chat.rs            // ChatSession: JSON message history, token-budget truncation
  cache.rs           // hash-keyed response cache, prune-by-count
  client/
    mod.rs            // trait: LlmClient (chat(), stream_chat())
    openai_compat.rs   // OpenAI-compatible HTTP + SSE streaming, via ureq
    ollama_native.rs   // native /api/chat for Ollama-specific options
  handler/
    mod.rs            // Handler trait: make_messages(), get_completion(), handle()
    default.rs        // single-shot, no persistence
    chat.rs           // + chat session persistence
    repl.rs           // + interactive loop on top of chat.rs
  tools/
    mod.rs            // ToolRegistry, iterative tool-call execution loop
    shell_exec.rs      // built-in execute_shell_command tool, confirm-before-execute
  render/
    mod.rs            // Printer trait: TextPrinter vs MarkdownPrinter
    markdown.rs        // pulldown-cmark -> syntect -> crossterm live redraw
  editor.rs           // $EDITOR tempfile round-trip
  shell_cmd.rs        // spawn via user's $SHELL for --shell execution
```

One deliberate improvement over the Python design: **role identity as explicit metadata, not string-sniffed.** Today's Python stores `"You are {name}\n{role}"` as the system message and regex-extracts the name back out later (`role.py: get_role_name`/`same_role` — confirmed by reading the source, this is exactly as fragile as it sounds). In Rust, persist chat metadata as an explicit struct (`{ role_name, created_at, messages }`) instead.

Critical Python files used as the reference for logic translation (confirmed by direct read):
- `sgpt/app.py` — CLI flow, flag dispatch, stdin/tty handling
- `sgpt/handlers/handler.py` — streaming + tool-call loop (confirmed: recursive `get_completion` re-invocation on `finish_reason == "tool_calls"`, no confirmation before tool execution)
- `sgpt/handlers/chat_handler.py` — chat session persistence/truncation
- `sgpt/role.py` — role storage, default role templates, OS/shell detection (confirmed: `ROLE_TEMPLATE`/`get_role_name` string-sniffing)
- `sgpt/config.py`, `sgpt/function.py`, `sgpt/printer.py`

## Design decisions for the three stated priorities

**Better UX**
- Fix a real safety gap: the Python tool confirms before executing a *generated* shell command (`--shell` flow) but has **zero confirmation** before executing an LLM-invoked tool call (`handle_function_call` in `handler.py`, confirmed by reading the code — it calls `get_function(name)(**dict_args)` immediately). The Rust version routes both paths through the same confirm-before-execute gate by default, with an explicit opt-out for scripting use.
- Clearer error messages via `anyhow`/`thiserror` instead of generic `UsageError` catch-alls.
- No interpreter/import startup lag — matters when iterating quickly against local models.

**Lower resource usage**
- Inherent wins: no interpreter startup, no venv/import scanning, small static binary, low idle memory, no GC.
- Implementation choice already made: `ureq` (blocking) over `reqwest`+`tokio` keeps the dependency tree and binary small for a tool that's fundamentally request/response plus simple streaming.
- Avoid a full TUI framework (`ratatui` is oriented at full-screen apps) — hand-roll the inline live-region redraw with `crossterm` primitives directly.

**Optimization for smaller/local models**
- Native Ollama client hitting `/api/chat` directly (not just the OpenAI-compat shim) to expose `num_ctx`, `num_predict`, `keep_alive` — a genuine differentiator, since the Python tool only reaches local models via the OpenAI-compat base-URL trick or LiteLLM.
- Optional "minimal schema" mode for built-in tool definitions — small models often handle flatter, less verbose JSON schemas better than the OpenAI SDK's default nested schema style.
- Token-budget-aware chat truncation instead of the Python version's pure message-count truncation (confirmed: `chat_handler.py` truncates by `CHAT_CACHE_LENGTH` message count, not tokens — a poor fit for small context windows).
- "Raw completion" mode that omits `tools`/`tool_choice` entirely for models that handle tool-calling poorly.

## Phased build plan

| Phase | Scope | Depends on |
|---|---|---|
| 1 | `clap` CLI skeleton, config load/bootstrap, OpenAI-compat HTTP client with SSE streaming (via `ureq`), single-shot query, plain-text streaming output | none |
| 2 | Roles: default 4 + custom create/show/list, JSON persistence | 1 |
| 3 | Chat sessions + REPL: persistent named chats, list/show, REPL loop, truncation (start by count, upgrade to token-budget) | 1, 2 |
| 3.5 | Native Ollama client (`/api/chat`), small-model schema/truncation optimizations | 1 |
| 4 | `--shell`/`--describe-shell`/`--code` + confirm loop, stdin/sentinel handling, `$EDITOR` | 1-3 |
| 5 | Built-in tool calling (fixed set) with confirm-before-execute, iterative tool loop, response caching | 1-4 |
| 6 | Markdown rendering polish: live-region redraw via `pulldown-cmark` + `syntect` + `crossterm` | can parallel 3-5 once Phase 1 streaming exists |
| 7 (stretch, only if needed later) | Shell integration installer, packaging/distribution | all prior |

Each phase is independently usable — after Phase 1 alone, there's already a working tool for basic single-shot queries against a local Ollama server.

## Effort estimate (solo, incremental)

- Phase 1: a few evenings to a weekend — highest leverage, satisfies the core "fast local-model queries" goal.
- Phase 2: a single evening.
- Phase 3: a weekend.
- Phase 3.5: a weekend.
- Phase 4: a weekend to a week — stdin/tty juggling and the confirm loop have real edge cases worth testing carefully.
- Phase 5: a weekend to a week — the iterative tool-call loop needs careful state threading (owned `Vec<Message>`, not recursion) plus the new confirm-before-execute UX.
- Phase 6: open-ended; timebox it and ship a "static re-print per chunk" fallback first, iterate from there.
- Phase 7: unscoped stretch, likely not needed given the extensibility decision above.

**Total for Phases 1-5 (usable MVP): roughly 4-8 weekends.**

## Verification

Since this is a from-scratch CLI tool, "verification" per phase means:
- Phase 1: run against a local Ollama instance (`ollama serve` + a pulled model) and against OpenAI, confirm streaming output renders correctly for both.
- Phase 2-3: create/list/show roles and chats, confirm JSON files persist correctly in the new config dir and survive process restarts.
- Phase 4: test piped stdin (`echo "list files" | <bin> -s`), confirm the Execute/Modify/Describe/Abort loop behaves correctly, test `$EDITOR` flow.
- Phase 5: trigger a built-in tool call against a model that supports tool-calling, confirm the confirm-before-execute gate blocks unconfirmed execution, confirm caching skips tool-call responses.
- Phase 6: visually compare streamed markdown/code-block output against the Python original for reasonable fidelity.
- No existing test suite to port initially; add `cargo test` coverage for config parsing, role JSON round-trip, and truncation logic as each phase lands.
