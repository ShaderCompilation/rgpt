# rgpt

`rgpt` is a fast Rust command-line assistant for chat completions. It is designed to feel at home in a terminal: ask a one-off question, pipe in context, keep a named conversation, generate a shell command, or run against a local Ollama model without changing tools.

It works with OpenAI's API and other OpenAI-compatible servers, including LM Studio, vLLM, and llama.cpp server. For Ollama, it also supports the native `/api/chat` API and its local-model tuning options.

## Inspiration

`rgpt` is inspired by [ShellGPT](https://github.com/TheR1D/shell_gpt). It was created to be more optimized for smaller local models and more performant through its Rust implementation(almost a meme now, i know).

## Highlights

- Stream responses directly to the terminal with optional ANSI color
- Use OpenAI-compatible `/chat/completions` servers or Ollama's native API
- Keep named chat sessions or start an interactive REPL
- Generate, review, describe, and optionally execute shell commands
- Create reusable system roles for repeatable workflows
- Pipe files and command output straight into a prompt
- Compose longer prompts in `$EDITOR`
- Cache text-only responses on disk and limit chat context for smaller models
- Let tool-capable models request shell execution, with confirmation by default

## Install

Prebuilt binaries for macOS and Linux will be available with the first release. Install the latest version with:

```sh
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/ShaderCompilation/rgpt/releases/latest/download/rgpt-installer.sh | sh
```

The installer places `rgpt` in `$CARGO_HOME/bin` (normally `~/.cargo/bin`). Add that directory to your `PATH` if it is not already present. You can also download an archive for a specific release from the [Releases page](https://github.com/ShaderCompilation/rgpt/releases).

### Uninstall

Run the installed command and confirm the prompt:

```sh
rgpt --uninstall
```

This removes the executable plus rgpt's configuration, saved chats, roles, and response cache. For a non-interactive uninstall, use `rgpt --uninstall --yes`.

### Build from source

To build locally, install a current stable Rust toolchain, then run:

```sh
git clone https://github.com/ShaderCompilation/rgpt.git
cd rgpt
cargo install --path .
```

For development, use `cargo run --` in place of `rgpt`:

```sh
cargo run -- "Explain the difference between a process and a thread"
```

## Quick start

### OpenAI or another compatible API

Set your API key and ask a question:

```sh
export OPENAI_API_KEY="..."
rgpt "Write a concise release note for a bug fix"
```

On its first run, `rgpt` creates its configuration file and, if no `OPENAI_API_KEY` environment variable is present, securely prompts for a key. The default endpoint is `https://api.openai.com/v1`.

To target another compatible server, set `API_BASE_URL` and select a model:

```sh
export API_BASE_URL="http://localhost:1234/v1"
rgpt --model "my-local-model" "Summarize this log"
```

### Ollama

With [Ollama](https://ollama.com/) running locally and a model downloaded:

```sh
ollama pull llama3.2
rgpt --ollama --model llama3.2 "Explain this Rust compiler error in one paragraph"
```

Native Ollama mode exposes controls useful for local models:

```sh
rgpt --ollama --model llama3.2 --num-ctx 8192 --num-predict 400 --keep-alive 10m \
  "Give me a plan for adding tests to this crate"
```

Use `OLLAMA_BASE_URL` to point at a remote or non-default Ollama host. Set `USE_OLLAMA=true` if Ollama should be your default backend.

## Everyday usage

```sh
# Ask a single question
rgpt "How do I find the ten largest files here?"

# Send command output or a file as context
git diff | rgpt "Review this change for edge cases"
rgpt "Summarize this config" < nginx.conf

# Add an instruction after piped context
journalctl -u api.service | rgpt "Identify the likely cause of failure"

# Compose the prompt in your configured editor
rgpt --editor

# Return only code, without Markdown fences or explanation
rgpt --code "Rust function that parses a comma-separated list of u16 values"

# Explain an existing shell command
rgpt --describe-shell "find . -type f -mtime -7 -print0 | xargs -0 ls -lh"
```

### Shell commands

`--shell` asks the model for a plain shell command and then offers to execute, modify, describe, or abort it. Review generated commands before execution.

```sh
rgpt --shell "remove all untracked build artifacts"
```

Add `--no-interaction` only when scripting and you explicitly want to skip the confirmation prompt:

```sh
rgpt --shell --no-interaction "print the current Git branch"
```

The model may also request the built-in `execute_shell_command` tool. Such requests are confirmed by default; `--no-interaction` is the explicit opt-out. Commands run through `$SHELL` (falling back to `/bin/sh`).

### Chats and REPL

Named chats are persisted across invocations:

```sh
rgpt --chat rust-help "I am building a CLI in Rust. Remember that context."
rgpt --chat rust-help "What error-handling approach would you recommend?"

rgpt --show-chat rust-help
rgpt --list-chats
```

Start an interactive session with `--repl`. Use `temp` for a throwaway conversation, `exit()` to leave, and `"""` on its own line to enter a multi-line prompt (close it with another `"""`).

```sh
rgpt --repl temp
rgpt --shell --repl ops
```

In a shell REPL, `e` executes the most recently generated command and `d` describes it.

## Roles

Roles are reusable system prompts. `rgpt` creates these defaults automatically:

- `ShellGPT` — concise programming and system-administration assistant
- `Shell Command Generator`
- `Shell Command Descriptor`
- `Code Generator`

Create and use a role:

```sh
rgpt --create-role reviewer
# Enter the role description when prompted

rgpt --role reviewer "Review this function for correctness"
rgpt --show-role reviewer
rgpt --list-roles
```

When continuing a named chat, its original role is retained. Passing a conflicting `--role` is rejected so a conversation does not silently change behavior.

## Configuration

Configuration lives in `rgpt/.rgptrc` under your platform configuration directory—usually `$XDG_CONFIG_HOME/rgpt/.rgptrc` or `~/.config/rgpt/.rgptrc` on Linux. Environment variables take precedence over values in that file.

Common settings:

```ini
# API backend
OPENAI_API_KEY=...
API_BASE_URL=default
DEFAULT_MODEL=gpt-5.4-mini
DEFAULT_TEMPERATURE=0.0
REQUEST_TIMEOUT=60
DISABLE_STREAMING=false

# Terminal and conversations
DEFAULT_COLOR=magenta
CHAT_CACHE_LENGTH=100
MAX_CONTEXT_TOKENS=0
CACHE_LENGTH=100

# Native Ollama backend
USE_OLLAMA=false
OLLAMA_BASE_URL=http://localhost:11434
OLLAMA_NUM_CTX=
OLLAMA_NUM_PREDICT=
OLLAMA_KEEP_ALIVE=
```

`MAX_CONTEXT_TOKENS=0` disables token-budget truncation. Set a positive value for models with a smaller context window; `rgpt` retains the system message and newest turns using a lightweight estimate. Set `CACHE_LENGTH=0` to disable the response cache. Tool-using responses are never cached.

You can override most request settings per call:

```sh
rgpt --model gpt-5.4-mini --temperature 0.2 --top-p 0.9 "Draft a changelog entry"
```

## Command reference

```text
rgpt [OPTIONS] [PROMPT]

Assistance
  -s, --shell             Generate a shell command and offer to execute it
  -d, --describe-shell    Describe a shell command
  -c, --code              Generate code only
      --editor            Compose a prompt in $EDITOR
      --no-interaction    Skip shell-execution confirmations

Conversation
      --chat <ID>         Continue a named chat; use "temp" for a throwaway chat
      --repl <ID>         Start an interactive session
      --show-chat <ID>    Print a saved chat
      --list-chats        List saved chats

Roles
      --role <ROLE>         Select a system role
      --create-role <NAME>  Create a role interactively
      --show-role <NAME>    Print a role
      --list-roles          List roles

Models
      --model <MODEL>
      --temperature <0.0-2.0>
      --top-p <0.0-1.0>
      --ollama
      --num-ctx <TOKENS>
      --num-predict <TOKENS>
      --keep-alive <DURATION>
```

Run `rgpt --help` for the authoritative, installed version of the command reference.

## Data and safety

`rgpt` stores configuration, roles, saved chats, and the response cache beneath your platform configuration directory. Named chat histories contain the prompts and responses you send; use `--chat temp` for an automatically discarded session.

Shell execution is powerful. Generated shell commands require a choice in the normal `--shell` flow, and model-requested command execution is denied unless you confirm it. Treat `--no-interaction` as an automation-only flag and use it deliberately.

## Development

```sh
cargo fmt --check
cargo test
cargo run -- --help
```

The project targets Linux and macOS and uses a blocking HTTP client to keep the command lightweight. Contributions that improve terminal UX, local-model behavior, correctness, or test coverage are welcome.
