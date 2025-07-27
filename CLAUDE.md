# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build and Development Commands

Build the session-finder binary:
```bash
just build
# or: cargo build --release
```

Run with search terms:
```bash
just run "search terms here"
# or: cargo run --release -- "search terms"
```

Test the code:
```bash
just test
# or: cargo test
```

Clean build artifacts:
```bash
just clean
# or: cargo clean
```

Install to local bin directory:
```bash
just install
```

## Architecture Overview

This is a single-binary Rust CLI tool that searches through Claude Code session files. The architecture consists of:

- **Main binary** (`src/main.rs`): CLI interface using clap for argument parsing
- **Session analysis**: Parses JSONL session files containing Claude Code conversation data
- **Search strategy**: Uses ripgrep for fast file searching, then performs detailed content analysis
- **Content extraction**: Deserializes JSONL messages to extract topics, common terms, and conversation summaries
- **Ranking system**: Sorts results by relevance (topic matches) and recency

## Key Technical Details

**Session file location**: `~/.claude/projects/[encoded-path]/[session-id].jsonl`

**Path encoding**: Project paths are encoded (e.g., `/Users/amar/repos/project` becomes `-Users-amar-repos-project`)

**JSONL structure**: Each line is a JSON message with nested content that can be either plain text or structured blocks

**Dependencies**: Requires `rg` (ripgrep) to be available in PATH for file searching

**Performance**: Uses ripgrep for initial filtering, then performs detailed analysis only on matching files

## Integration Context

This tool is designed to replace multi-command shell pipelines in the Claude Code session-finder agent. The binary is called directly by the agent to avoid permission prompts for multiple shell commands.

The corresponding agent file (`session-finder.md`) should be symlinked to `~/.claude/agents/` for Claude Code integration.