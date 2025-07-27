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
- **Timeline extraction** - shows chronological evolution of solutions with `--timeline` flag
- **Code diff timeline** - extracts all code changes with context using `--code-diff` flag
- **Content type detection** - classifies code blocks, tool calls, errors, and discussions
- **Rich session metadata** - file sizes, line counts, modification times  
- **Content analysis** - first/last messages, extracted topics, common terms
- **Path decoding** - converts encoded paths (e.g., `-Users-amar-repos-project` → `/Users/amar/repos/project`)
- **Intelligent filtering** - removes boilerplate terms, focuses on meaningful content
- **Robust error handling** - handles special regex characters gracefully
- **Flexible search** - by content, project path, recency, result limits

## Command Line Usage

```bash
session-finder [OPTIONS] <SEARCH_TERMS>...

Arguments:
  <SEARCH_TERMS>...  Terms to search for in session content

Options:
  -p, --project <PROJECT>           Filter by project path
  -r, --recent <DAYS>               Only show sessions from last N days
  -l, --limit <LIMIT>               Limit number of results [default: 10]
  -t, --timeline <SESSION_ID>       Extract timeline for specific session
  -d, --code-diff <SESSION_ID>      Extract timeline of code diffs for specific session
  -c, --context <NUM>               Context messages before/after matches [default: 2]
  -h, --help                        Print help
```

## Examples

```bash
# Find sessions about Rust error handling
session-finder "rust error handling"

# Find recent sessions in a specific project
session-finder --project "/Users/amar/repos/myproject" --recent 7 "debugging"

# Limit results and search for authentication topics
session-finder --limit 5 "authentication login jwt"

# Extract timeline showing evolution of solutions for a specific session
session-finder --timeline abc123 "tree-sitter"

# Extract timeline with more context messages
session-finder --timeline abc123 --context 3 "use_wildcard"

# Extract code diff timeline showing all code changes
session-finder --code-diff abc123

# Extract code diff timeline with context
session-finder --code-diff abc123 --context 1
```

## Output Format

### Standard Search Results
Each session result includes:
- **Session ID** and resume command
- **Project path** (decoded from session filename)
- **Timestamps** (first and last messages)
- **File metadata** (size, line count)
- **Content preview** (first and last messages, truncated)
- **Common terms** (filtered to remove boilerplate)

### Timeline Extraction
Timeline output shows:
- **Chronological message flow** with timestamps and roles
- **Content type classification** (Discussion, Code Block, Tool Call, Error, Success Response)
- **Context messages** before and after each match
- **Evolution of solutions** showing how problems were identified and resolved

### Code Diff Timeline
Code diff timeline output shows:
- **Chronological code changes** with timestamps and roles
- **Tool operations** (Write, Edit, MultiEdit, Bash commands)
- **Code blocks** from user messages and markdown
- **Clear formatting** with emojis and structured diffs (Replace/With for edits)
- **Context messages** before and after each code change

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