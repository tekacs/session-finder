use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use clap::{Arg, Command};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

#[derive(Debug, Serialize, Deserialize)]
struct SessionMessage {
    #[serde(rename = "type")]
    msg_type: String,
    message: Option<InnerMessage>,
    timestamp: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct InnerMessage {
    role: Option<String>,
    content: Option<Content>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum Content {
    Text(String),
    Array(Vec<ContentBlock>),
}

#[derive(Debug, Serialize, Deserialize)]
struct ContentBlock {
    r#type: String,
    text: Option<String>,
}

#[derive(Debug)]
struct SessionInfo {
    path: PathBuf,
    session_id: String,
    project_path: String,
    last_modified: DateTime<Utc>,
    line_count: usize,
    topics: Vec<String>,
    first_messages: Vec<String>,
    last_messages: Vec<String>,
    common_terms: Vec<String>,
    file_size_bytes: u64,
}

fn main() -> Result<()> {
    let matches = Command::new("session-finder")
        .about("Find and analyze Claude Code sessions")
        .arg(
            Arg::new("query")
                .help("Search terms to find in sessions")
                .required(true)
                .num_args(1..),
        )
        .arg(
            Arg::new("project")
                .short('p')
                .long("project")
                .help("Filter by project path")
                .value_name("PATH"),
        )
        .arg(
            Arg::new("limit")
                .short('l')
                .long("limit")
                .help("Maximum number of results to return")
                .value_name("NUM")
                .default_value("10"),
        )
        .arg(
            Arg::new("recent")
                .short('r')
                .long("recent")
                .help("Show only sessions from the last N days")
                .value_name("DAYS"),
        )
        .get_matches();

    let search_terms: Vec<&str> = matches.get_many::<String>("query").unwrap().map(|s| s.as_str()).collect();
    let project_filter = matches.get_one::<String>("project");
    let limit: usize = matches.get_one::<String>("limit").unwrap().parse()?;
    let recent_days = matches.get_one::<String>("recent").map(|s| s.parse::<i64>()).transpose()?;

    let sessions = find_sessions(&search_terms, project_filter, recent_days)?;
    let top_sessions = rank_and_limit_sessions(sessions, limit);

    display_results(&top_sessions)?;

    Ok(())
}

fn find_sessions(
    search_terms: &[&str],
    project_filter: Option<&String>,
    recent_days: Option<i64>,
) -> Result<Vec<SessionInfo>> {
    let projects_dir = Path::new(&std::env::var("HOME")?)
        .join(".claude")
        .join("projects");

    if !projects_dir.exists() {
        return Err(anyhow!("Projects directory not found: {:?}", projects_dir));
    }

    // First, use ripgrep to find files containing our search terms
    let rg_files = find_files_with_ripgrep(&projects_dir, search_terms)?;
    
    let mut sessions = Vec::new();
    
    for file_path in rg_files {
        let full_path = projects_dir.join(file_path);
        if let Some(session_info) = analyze_session_file(&full_path, search_terms, project_filter, recent_days)? {
            sessions.push(session_info);
        }
    }

    Ok(sessions)
}

fn find_files_with_ripgrep(projects_dir: &Path, search_terms: &[&str]) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    
    // Use ripgrep to find files containing any of the search terms
    let search_pattern = search_terms.join("|");
    let output = process::Command::new("rg")
        .args(&["-li", "--glob", "*.jsonl", &search_pattern])
        .current_dir(projects_dir)
        .output()
        .map_err(|e| anyhow!("Ripgrep failed: {}. Make sure 'rg' is in your PATH", e))?;
    
    if !output.status.success() {
        return Err(anyhow!("Ripgrep command failed with status: {}", output.status));
    }
    
    let output_str = String::from_utf8(output.stdout)?;
    
    for line in output_str.lines() {
        if line.ends_with(".jsonl") {
            files.push(PathBuf::from(line.trim()));
        }
    }
    
    Ok(files)
}

fn analyze_session_file(
    file_path: &Path,
    search_terms: &[&str],
    project_filter: Option<&String>,
    recent_days: Option<i64>,
) -> Result<Option<SessionInfo>> {
    let metadata = fs::metadata(file_path)?;
    let last_modified = DateTime::from(metadata.modified()?);
    let file_size_bytes = metadata.len();
    
    // Check if file is recent enough
    if let Some(days) = recent_days {
        let cutoff = Utc::now() - chrono::Duration::days(days);
        if last_modified < cutoff {
            return Ok(None);
        }
    }
    
    let session_id = extract_session_id(file_path)?;
    let project_path = decode_project_path(file_path)?;
    
    // Check project filter
    if let Some(filter) = project_filter {
        if !project_path.contains(filter) {
            return Ok(None);
        }
    }
    
    let content = fs::read_to_string(file_path)?;
    let line_count = content.lines().count();
    
    // Extract enhanced session data
    let (topics, first_messages, last_messages, common_terms) = analyze_session_content_enhanced(&content, search_terms)?;
    
    Ok(Some(SessionInfo {
        path: file_path.to_path_buf(),
        session_id,
        project_path,
        last_modified,
        line_count,
        topics,
        first_messages,
        last_messages,
        common_terms,
        file_size_bytes,
    }))
}

fn extract_session_id(file_path: &Path) -> Result<String> {
    file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow!("Could not extract session ID from path: {:?}", file_path))
}

fn decode_project_path(file_path: &Path) -> Result<String> {
    let parent = file_path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("");
    
    // Decode escaped path: -Users-amar-repos-project -> /Users/amar/repos/project
    if parent.starts_with('-') {
        let decoded = parent[1..].replace('-', "/");
        Ok(format!("/{}", decoded))
    } else {
        Ok(parent.to_string())
    }
}

fn analyze_session_content_enhanced(content: &str, search_terms: &[&str]) -> Result<(Vec<String>, Vec<String>, Vec<String>, Vec<String>)> {
    let mut topics = Vec::new();
    let mut all_messages = Vec::new();
    let mut word_freq = HashMap::new();
    
    // Parse all JSONL lines to get complete session data
    for line in content.lines() {
        if let Ok(msg) = serde_json::from_str::<SessionMessage>(line) {
            if let Some(inner_msg) = &msg.message {
                if let Some(role) = &inner_msg.role {
                    if let Some(content) = &inner_msg.content {
                        let content_text = match content {
                            Content::Text(text) => text.clone(),
                            Content::Array(blocks) => {
                                blocks.iter()
                                    .filter_map(|block| {
                                        if block.r#type == "text" {
                                            block.text.as_ref()
                                        } else {
                                            None
                                        }
                                    })
                                    .cloned()
                                    .collect::<Vec<String>>()
                                    .join(" ")
                            }
                        };
                        
                        if !content_text.is_empty() {
                            all_messages.push(format!("{}: {}", role, truncate_text(&content_text, 200)));
                            
                            // Extract topics from content matching search terms
                            for term in search_terms {
                                if content_text.to_lowercase().contains(&term.to_lowercase()) {
                                    extract_topics_from_text(&content_text, term, &mut topics);
                                }
                            }
                            
                            // Count word frequencies for common terms (filtering boilerplate)
                            for word in content_text.split_whitespace() {
                                let clean_word = word.to_lowercase().trim_matches(|c: char| !c.is_alphanumeric()).to_string();
                                if clean_word.len() > 2 && !is_boilerplate_word(&clean_word) {
                                    *word_freq.entry(clean_word).or_insert(0) += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    // Get first and last messages
    let first_messages = all_messages.iter().take(8).cloned().collect();
    let last_messages = all_messages.iter().rev().take(8).cloned().collect::<Vec<_>>().into_iter().rev().collect();
    
    
    // Get most common terms (top 50 meaningful terms)
    let mut common_terms: Vec<(String, usize)> = word_freq.into_iter().collect();
    common_terms.sort_by(|a, b| b.1.cmp(&a.1));
    let common_terms: Vec<String> = common_terms.into_iter().take(50).map(|(word, count)| format!("{}({})", word, count)).collect();
    
    // Deduplicate topics
    topics.sort();
    topics.dedup();
    
    Ok((topics, first_messages, last_messages, common_terms))
}


fn extract_topics_from_text(text: &str, search_term: &str, topics: &mut Vec<String>) {
    let re = Regex::new(&format!(r"(?i)\b{}\b[\w\s]*", regex::escape(search_term))).unwrap();
    
    for mat in re.find_iter(text) {
        let topic = mat.as_str().trim().to_string();
        if topic.len() > 3 && topic.len() < 50 {
            topics.push(topic);
        }
    }
}


fn truncate_text(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        text.to_string()
    } else {
        // Find the last valid char boundary at or before max_len
        let mut boundary = max_len;
        while boundary > 0 && !text.is_char_boundary(boundary) {
            boundary -= 1;
        }
        format!("{}...", &text[..boundary])
    }
}

fn rank_and_limit_sessions(mut sessions: Vec<SessionInfo>, limit: usize) -> Vec<SessionInfo> {
    // Sort by relevance (more topics = higher relevance) and recency
    sessions.sort_by(|a, b| {
        let relevance_cmp = b.topics.len().cmp(&a.topics.len());
        if relevance_cmp == std::cmp::Ordering::Equal {
            b.last_modified.cmp(&a.last_modified)
        } else {
            relevance_cmp
        }
    });
    
    sessions.into_iter().take(limit).collect()
}

fn is_boilerplate_word(word: &str) -> bool {
    matches!(word,
        // Common English words
        "the" | "and" | "for" | "with" | "that" | "this" | "but" | "not" | "are" | "was" | "were" |
        "has" | "had" | "have" | "can" | "will" | "would" | "could" | "should" | "may" | "might" |
        "get" | "put" | "set" | "run" | "use" | "add" | "see" | "now" | "let" | "all" | 
        "one" | "two" | "three" | "four" | "five" | "six" | "seven" | "eight" | "nine" | "ten" |
        "from" | "into" | "over" | "then" | "when" | "what" | "where" | "which" | "who" | "why" | "how" |
        "you" | "your" | "i'm" | "i'll" | "i've" | "it's" | "we're" | "they" | "them" | "their" |
        "more" | "most" | "some" | "any" | "each" | "both" | "other" | "same" | "next" | "last" |
        "first" | "out" | "off" | "way" | "too" | "own" | "just" | "only" | "also" | "back" |
        
        // Programming boilerplate
        "let" | "mut" | "use" | "pub" | "impl" | "struct" | "enum" | "type" | "trait" | "fn" |
        "async" | "await" | "self" | "super" | "crate" | "mod" | "extern" | "const" | "static" |
        "str" | "string" | "bool" | "true" | "false" | "none" | "some" | "ok" | "err" | "result" |
        "vec" | "option" | "clone" | "into" | "from" | "new" | "default" | "debug" | "derive" |
        "cargo" | "toml" | "src" | "lib" | "main" | "test" | "tests" | "target" | "build" |
        
        // Claude Code / JSONL boilerplate
        "user" | "assistant" | "message" | "content" | "role" | "type" | "timestamp" | "session" |
        "request" | "response" | "interrupted" | "tool" |
        
        // Common version numbers and paths that appear frequently
        "100644" | "registry" | "https" | "github" | "com" | "crates" | "index" |
        
        // Common technical terms that don't add much context
        "code" | "line" | "file" | "path" | "name" | "text" | "data" | "info" | "log" | "debug" |
        "check" | "fix" | "update" | "change" | "version" | "issue" | "error" | "warning" |
        "output" | "input" | "return" | "function" | "method" | "call" | "create" | "make" |
        "work" | "working" | "works" | "used" | "using" | "added" | "removed" | "fixed" |
        "need" | "needs" | "want" | "trying" | "looks" | "seems" | "actually" | "really" |
        "good" | "great" | "perfect" | "okay" | "right" | "correct" | "wrong" | "better" |
        "think" | "know" | "understand" | "mean" | "say" | "tell" | "show" | "find" |
        "help" | "try" | "attempt" | "continue" | "start" | "stop" | "end" | "done" |
        "here" | "there" | "where" | "when" | "what" | "how" | "why" | "who" | "which" |
        "before" | "after" | "during" | "while" | "until" | "since" | "about" | "around" |
        "above" | "below" | "over" | "under" | "through" | "across" | "between" | "among" |
        "without" | "within" | "outside" | "inside" | "instead" | "besides" | "except" |
        "including" | "excluding" | "according" | "regarding" | "concerning" | "despite" |
        "however" | "therefore" | "otherwise" | "moreover" | "furthermore" | "nevertheless" |
        "although" | "because" | "unless" | "whether" | "either" | "neither" | "both" |
        "different" | "similar" | "various" | "several" | "multiple" | "single" | "individual" |
        "general" | "specific" | "particular" | "special" | "common" | "normal" | "regular" |
        "current" | "previous" | "recent" | "latest" | "original" | "initial" | "final" |
        "example" | "instance" | "case" | "situation" | "condition" | "state" | "status" |
        "problem" | "solution" | "answer" | "question" | "reason" | "cause" | "result" |
        "important" | "necessary" | "required" | "optional" | "available" | "possible" |
        "simple" | "complex" | "easy" | "difficult" | "hard" | "soft" | "quick" | "slow" |
        "big" | "small" | "large" | "little" | "long" | "short" | "high" | "low" |
        "full" | "empty" | "complete" | "incomplete" | "total" | "partial" | "whole" |
        "sure" | "certain" | "unclear" | "unknown" | "obvious" | "clear" | "visible" |
        "open" | "close" | "closed" | "old" | "new" | "fresh" | "clean" | "dirty" |
        "ready" | "busy" | "free" | "active" | "inactive" | "enabled" | "disabled" |
        "public" | "private" | "local" | "remote" | "external" | "internal" | "native"
    )
}

fn display_results(sessions: &[SessionInfo]) -> Result<()> {
    if sessions.is_empty() {
        println!("No sessions found matching your criteria.");
        return Ok(());
    }
    
    println!("Found {} relevant session(s):\n", sessions.len());
    
    for (i, session) in sessions.iter().enumerate() {
        println!("{}. Session: {}", i + 1, session.session_id);
        println!("   File: {}", session.path.display());
        println!("   Project: {}", session.project_path);
        println!("   Modified: {}", session.last_modified.format("%Y-%m-%d %H:%M:%S UTC"));
        println!("   Size: {} bytes, {} lines", session.file_size_bytes, session.line_count);
        
        if !session.topics.is_empty() {
            println!("   Topics: {}", session.topics.join(", "));
        }
        
        if !session.first_messages.is_empty() {
            println!("   First messages:");
            for msg in &session.first_messages {
                println!("     {}", msg);
            }
        }
        
        if !session.last_messages.is_empty() {
            println!("   Last messages:");
            for msg in &session.last_messages {
                println!("     {}", msg);
            }
        }
        
        if !session.common_terms.is_empty() {
            println!("   Common terms: {}", session.common_terms.join(", "));
        }
        
        println!("   Resume: claude --resume {}", session.session_id);
        println!();
    }
    
    Ok(())
}