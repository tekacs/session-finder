# Claude Code Session Finder

A Rust utility for finding and analyzing Claude Code sessions without triggering multiple shell command permission prompts.

## Overview

This tool replaces the multi-command shell pipeline used by the Claude Code session-finder agent with a single Rust binary. It provides enhanced session analysis including metadata, content summarization, and intelligent filtering.

## Quick Start

### Installation
1. Clone this repository to your Claude Code support directory:
   ```bash
   cd ~/.claude/support
   git clone <this-repo> session-finder
   cd session-finder
   ```

2. Build the tool:
   ```bash
   just build
   # or: cargo build --release
   ```

3. Symlink the agent file to your Claude Code agents directory:
   ```bash
   ln -sf ~/.claude/support/session-finder/session-finder.md ~/.claude/agents/session-finder.md
   ```

### Usage from this repository:
```bash
# Find sessions with specific terms
just run "rust error handling"

# Or run directly
cargo run -- "authentication login"
```

### Usage from Claude Code:
Once symlinked, use the session-finder agent in Claude Code:
```
Use the session-finder agent to find sessions about "rust error handling"
```

## Features

- **Single binary execution** - eliminates permission prompts for multiple shell commands
- **Rich session metadata** - file sizes, line counts, modification times  
- **Content analysis** - first/last messages, extracted topics, common terms
- **Path decoding** - converts encoded paths (e.g., `-Users-amar-repos-project` → `/Users/amar/repos/project`)
- **Intelligent filtering** - removes boilerplate terms, focuses on meaningful content
- **Flexible search** - by content, project path, recency, result limits

## Command Line Usage

```bash
session-finder [OPTIONS] <SEARCH_TERMS>...

Arguments:
  <SEARCH_TERMS>...  Terms to search for in session content

Options:
  -p, --project <PROJECT>    Filter by project path
  -r, --recent <DAYS>        Only show sessions from last N days
  -l, --limit <LIMIT>        Limit number of results [default: 10]
  -h, --help                 Print help
```

## Examples

```bash
# Find sessions about Rust error handling
session-finder "rust error handling"

# Find recent sessions in a specific project
session-finder --project "/Users/amar/repos/myproject" --recent 7 "debugging"

# Limit results and search for authentication topics
session-finder --limit 5 "authentication login jwt"
```

## Output Format

Each session result includes:
- **Session ID** and resume command
- **Project path** (decoded from session filename)
- **Timestamps** (first and last messages)
- **File metadata** (size, line count)
- **Content preview** (first and last messages, truncated)
- **Common terms** (filtered to remove boilerplate)

## Building

Requirements:
- Rust 1.70+
- `rg` (ripgrep) in PATH

```bash
# Build release binary
cargo build --release

# Build and run with justfile
just build
just run "search terms"
```

## Integration

This tool is used by the Claude Code session-finder agent to provide seamless session discovery without shell command permission prompts. The agent configuration points to the release binary at `~/.claude/support/session-finder/target/release/session-finder`.