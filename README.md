# Claude Code Session Finder

A powerful session discovery system for Claude Code that combines a specialized agent with a high-performance Rust utility to help you find and analyze your conversation history.

## Overview

The session finder system consists of two complementary components:

1. **Session Finder Agent** - A Claude Code agent that provides intelligent session discovery through natural language queries
2. **Rust Utility** - A fast, single-binary tool that performs the actual session analysis without triggering permission prompts

Together, they enable seamless discovery of past conversations, code solutions, and project discussions within your Claude Code history.

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
Once symlinked, use the session-finder agent in Claude Code with natural language:

**Basic session discovery:**
```
Use the session-finder agent to find sessions about "rust error handling"
Find my previous conversations about JWT authentication
Show me sessions where I worked on database migrations
```

**Timeline extraction to see solution evolution:**
```
Find sessions about "tree-sitter parsing" and show me the timeline of how we solved it
Show me the conversation flow for sessions about "authentication bug" to see how we debugged it
Find sessions about "API design" and extract the timeline to see how the solution evolved
```

**Code diff timeline to track implementation changes:**
```
Find sessions about "payment system" and show me all the code changes we made
Show me the code evolution for sessions about "database schema migration"
Find sessions about "user authentication" and extract all the code diffs to see the implementation
```

**Resurrecting and comparing approaches:**
```
Find sessions where we worked on "caching strategies" - I want to compare our previous approach to what I'm doing now
Show me how we solved "rate limiting" before and extract the code so I can adapt it to this new service
Find sessions about "error handling patterns" and show me the timeline - I think we had a better approach before
Compare our previous "database connection pooling" implementation to see if we should revert some changes
Find sessions where we debugged "memory leaks" and show me the code diffs - I'm seeing similar issues now
```

The agent will intelligently interpret your request and use the Rust utility to search through your session history or extract detailed timelines.

## Why Add This to Your Claude Code?

### Benefits
- **Instant session discovery** - quickly find relevant past conversations without browsing through files
- **Natural language queries** - ask the agent in plain English rather than constructing complex search terms
- **No permission prompts** - seamless experience without shell command interruptions
- **Rich context** - see conversation evolution, code changes, and solution development over time
- **Project-aware search** - filter by specific codebases or time periods
- **Learning from history** - rediscover solutions, patterns, and approaches from previous work
- **Approach comparison** - easily compare current implementations to previous solutions
- **Code archaeology** - resurrect and adapt working solutions from past sessions
- **Decision tracing** - understand why certain architectural choices were made
- **Debug pattern recognition** - find similar issues and their previous solutions

### Key Features
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

## How It Works

The session finder agent acts as an intelligent interface that:
1. **Interprets natural language queries** about your session history, including requests to compare approaches or resurrect solutions
2. **Translates requests** into appropriate search parameters and timeline extractions for the Rust utility
3. **Executes the binary** with optimized arguments for discovery, timeline analysis, or code diff extraction
4. **Presents results** with relevant context, code comparisons, and resume commands for further exploration

The Rust utility handles the heavy lifting:
- **Fast file scanning** using ripgrep for initial filtering
- **Content analysis** with JSON parsing and topic extraction
- **Timeline reconstruction** showing conversation evolution
- **Metadata enrichment** with file stats and decoded paths

## Agent Integration

The session-finder agent is configured to use the release binary at `~/.claude/support/session-finder/target/release/session-finder`, providing seamless integration with your Claude Code workflow.