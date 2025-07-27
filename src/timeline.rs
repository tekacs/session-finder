use anyhow::{anyhow, Result};
use regex::Regex;
use serde_json;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir;

use crate::{
    ClassifiedContent, CodeInfo, ContentType, ErrorInfo, SessionMessage, TimelineEntry,
    TimelineExtraction, ToolInfo, Content, ContentBlock,
};

#[derive(Debug)]
pub struct CodeDiffTimeline {
    pub session_id: String,
    pub code_changes: Vec<CodeDiffEntry>,
}

#[derive(Debug)]
pub struct CodeDiffEntry {
    pub message_index: usize,
    pub timestamp: String,
    pub role: String,
    pub code_content: String,
    pub language: Option<String>,
    pub change_type: CodeChangeType,
    pub context_before: Vec<String>,
    pub context_after: Vec<String>,
}

#[derive(Debug)]
pub enum CodeChangeType {
    Edit,      // File edits
    Write,     // New file writes
    CodeBlock, // Code blocks in discussions
    BashCommand, // Executable commands
}

pub fn extract_timeline(
    session_path: &str,
    search_terms: &[&str],
    context_size: usize,
) -> Result<TimelineExtraction> {
    let full_path = resolve_session_path(session_path)?;
    let session_id = extract_session_id_from_path(&full_path)?;
    let content = fs::read_to_string(&full_path)?;
    
    let all_messages = parse_session_messages(&content)?;
    let matching_indices = find_matching_messages(&all_messages, search_terms);
    
    let timeline: Vec<TimelineEntry> = matching_indices
        .into_iter()
        .map(|index| {
            let msg = &all_messages[index];
            let context_before = extract_context_messages(&all_messages, index, context_size, true);
            let context_after = extract_context_messages(&all_messages, index, context_size, false);
            
            TimelineEntry {
                message_index: index,
                timestamp: msg.timestamp.clone().unwrap_or_default(),
                role: msg.message.as_ref()
                    .and_then(|m| m.role.clone())
                    .unwrap_or_default(),
                classified_content: classify_message_content(msg),
                context_before,
                context_after,
            }
        })
        .collect();

    Ok(TimelineExtraction {
        session_id,
        query_term: search_terms.join(" "),
        timeline,
    })
}

fn resolve_session_path(session_path: &str) -> Result<PathBuf> {
    let path = Path::new(session_path);
    
    // If it's already a full path, use it
    if path.is_absolute() && path.exists() {
        return Ok(path.to_path_buf());
    }
    
    // If it's just a session ID, try to find it in ~/.claude/projects
    let projects_dir = Path::new(&std::env::var("HOME")?)
        .join(".claude")
        .join("projects");
    
    if path.extension().is_none() {
        // It's probably just a session ID, search for it
        for entry in walkdir::WalkDir::new(&projects_dir) {
            let entry = entry?;
            if entry.file_type().is_file() {
                if let Some(stem) = entry.path().file_stem() {
                    if stem == session_path {
                        return Ok(entry.path().to_path_buf());
                    }
                }
            }
        }
    }
    
    // Try as relative to projects dir
    let candidate = projects_dir.join(session_path);
    if candidate.exists() {
        return Ok(candidate);
    }
    
    Err(anyhow!("Could not resolve session path: {}", session_path))
}

fn extract_session_id_from_path(path: &Path) -> Result<String> {
    path.file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow!("Could not extract session ID from path: {:?}", path))
}

fn parse_session_messages(content: &str) -> Result<Vec<SessionMessage>> {
    let mut messages = Vec::new();
    
    for (index, line) in content.lines().enumerate() {
        if let Ok(mut msg) = serde_json::from_str::<SessionMessage>(line) {
            // Store the line index for reference
            if msg.timestamp.is_none() {
                msg.timestamp = Some(format!("line_{}", index));
            }
            messages.push(msg);
        }
    }
    
    Ok(messages)
}

fn find_matching_messages(messages: &[SessionMessage], search_terms: &[&str]) -> Vec<usize> {
    messages
        .iter()
        .enumerate()
        .filter_map(|(index, msg)| {
            if let Some(inner_msg) = &msg.message {
                if let Some(content) = &inner_msg.content {
                    let content_text = extract_content_text(content);
                    
                    // Skip lines that mention session-finder to avoid false positives
                    if content_text.to_lowercase().contains("session-finder") || 
                       content_text.to_lowercase().contains("session_finder") {
                        return None;
                    }
                    
                    for term in search_terms {
                        if content_text.to_lowercase().contains(&term.to_lowercase()) {
                            return Some(index);
                        }
                    }
                }
            }
            None
        })
        .collect()
}

fn extract_context_messages(
    messages: &[SessionMessage],
    center_index: usize,
    context_size: usize,
    before: bool,
) -> Vec<String> {
    let mut context = Vec::new();
    
    if before {
        let start = center_index.saturating_sub(context_size);
        for i in start..center_index {
            if let Some(msg) = messages.get(i) {
                context.push(format_message_summary(msg));
            }
        }
    } else {
        let end = std::cmp::min(center_index + context_size + 1, messages.len());
        for i in (center_index + 1)..end {
            if let Some(msg) = messages.get(i) {
                context.push(format_message_summary(msg));
            }
        }
    }
    
    context
}

fn classify_message_content(msg: &SessionMessage) -> ClassifiedContent {
    if let Some(inner_msg) = &msg.message {
        if let Some(content) = &inner_msg.content {
            let content_text = extract_content_text(content);
            let content_type = determine_content_type(content, &content_text);
            
            return ClassifiedContent {
                raw_content: content_text,
                content_type,
            };
        }
    }
    
    ClassifiedContent {
        raw_content: String::new(),
        content_type: ContentType::PlainText,
    }
}

fn determine_content_type(content: &Content, content_text: &str) -> ContentType {
    match content {
        Content::Array(blocks) => {
            // Check for tool calls first
            for block in blocks {
                if block.r#type == "tool_use" {
                    return ContentType::ToolCall(ToolInfo {
                        tool_name: block.name.clone().unwrap_or_default(),
                        action_type: classify_tool_action(&block.name.as_deref().unwrap_or_default()),
                        target_files: extract_target_files(&block.input),
                    });
                }
            }
        }
        _ => {}
    }
    
    // Check for code blocks
    if let Some(code_info) = extract_code_block_info(content_text) {
        return ContentType::CodeBlock(code_info);
    }
    
    // Check for error messages
    if let Some(error_info) = detect_error_patterns(content_text) {
        return ContentType::ErrorMessage(error_info);
    }
    
    // Check for success responses
    if is_success_response(content_text) {
        return ContentType::SuccessResponse;
    }
    
    ContentType::Discussion
}

fn extract_content_text(content: &Content) -> String {
    match content {
        Content::Text(text) => text.clone(),
        Content::Array(blocks) => {
            blocks
                .iter()
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
    }
}

fn extract_code_block_info(content: &str) -> Option<CodeInfo> {
    let fence_regex = Regex::new(r"```(\w+)?\n(.*?)\n```").ok()?;
    
    if let Some(captures) = fence_regex.captures(content) {
        let language = captures.get(1).map(|m| m.as_str().to_string());
        let code = captures.get(2).map(|m| m.as_str()).unwrap_or("");
        let line_count = code.lines().count();
        let is_complete = is_complete_code_block(code, language.as_deref());
        
        return Some(CodeInfo {
            language,
            is_complete,
            line_count,
        });
    }
    
    None
}

fn is_complete_code_block(code: &str, language: Option<&str>) -> bool {
    match language {
        Some("rust") => {
            code.contains("fn ") && (code.contains("{") && code.contains("}"))
        }
        Some("javascript") | Some("typescript") => {
            code.contains("function ") || code.contains("=>") || code.contains("{")
        }
        Some("python") => {
            code.contains("def ") || code.contains("class ")
        }
        _ => code.lines().count() > 3 // Simple heuristic for other languages
    }
}

fn classify_tool_action(tool_name: &str) -> String {
    match tool_name {
        "Read" | "Glob" | "Grep" => "read",
        "Edit" | "Write" | "MultiEdit" => "write",
        "Bash" => "execute",
        "LS" => "list",
        _ => "other",
    }
    .to_string()
}

fn extract_target_files(input: &Option<serde_json::Value>) -> Vec<String> {
    let mut files = Vec::new();
    
    if let Some(input_val) = input {
        if let Some(file_path) = input_val.get("file_path") {
            if let Some(path_str) = file_path.as_str() {
                files.push(path_str.to_string());
            }
        }
        if let Some(path) = input_val.get("path") {
            if let Some(path_str) = path.as_str() {
                files.push(path_str.to_string());
            }
        }
    }
    
    files
}

fn detect_error_patterns(content: &str) -> Option<ErrorInfo> {
    if content.contains("error[E") || content.contains("cannot find") {
        Some(ErrorInfo {
            error_type: "compilation".to_string(),
            severity: "error".to_string(),
            source: Some("rustc".to_string()),
        })
    } else if content.contains("warning:") {
        Some(ErrorInfo {
            error_type: "compilation".to_string(),
            severity: "warning".to_string(),
            source: Some("rustc".to_string()),
        })
    } else if content.contains("Permission denied") || content.contains("No such file") {
        Some(ErrorInfo {
            error_type: "tool_error".to_string(),
            severity: "error".to_string(),
            source: Some("system".to_string()),
        })
    } else if content.contains("panicked at") || content.contains("thread 'main' panicked") {
        Some(ErrorInfo {
            error_type: "runtime".to_string(),
            severity: "error".to_string(),
            source: Some("rust".to_string()),
        })
    } else {
        None
    }
}

fn is_success_response(content: &str) -> bool {
    let success_indicators = [
        "works", "perfect", "great", "excellent", "success", "completed",
        "fixed", "solved", "done", "good", "that's it"
    ];
    
    let lower_content = content.to_lowercase();
    success_indicators.iter().any(|&indicator| lower_content.contains(indicator))
}

fn format_message_summary(msg: &SessionMessage) -> String {
    if let Some(inner_msg) = &msg.message {
        if let Some(role) = &inner_msg.role {
            if let Some(content) = &inner_msg.content {
                let content_text = extract_content_text(content);
                let truncated = if content_text.len() > 100 {
                    format!("{}...", &content_text[..97])
                } else {
                    content_text
                };
                return format!("{}: {}", role, truncated);
            }
        }
    }
    "Unknown message".to_string()
}

pub fn display_timeline(timeline: &TimelineExtraction) -> Result<()> {
    println!("=== Timeline for \"{}\" in session {} ===\n", 
             timeline.query_term, timeline.session_id);
    
    for entry in &timeline.timeline {
        let content_type_label = match &entry.classified_content.content_type {
            ContentType::PlainText => "Discussion".to_string(),
            ContentType::CodeBlock(info) => {
                format!("Code Block ({}, {} lines)", 
                       info.language.as_deref().unwrap_or("unknown"), 
                       info.line_count)
            }
            ContentType::ToolCall(info) => {
                format!("Tool Call ({} → {})", 
                       info.tool_name, 
                       info.target_files.join(", "))
            }
            ContentType::ErrorMessage(info) => {
                format!("Error ({})", info.error_type)
            }
            ContentType::SuccessResponse => "Success Response".to_string(),
            ContentType::Discussion => "Discussion".to_string(),
        };
        
        println!("[Message {} - {}] {}: {}", 
                 entry.message_index, 
                 entry.timestamp, 
                 entry.role, 
                 content_type_label);
        
        if !entry.context_before.is_empty() {
            println!("  Context before:");
            for ctx in &entry.context_before {
                println!("    {}", ctx);
            }
        }
        
        println!("  → {}", entry.classified_content.raw_content);
        
        if !entry.context_after.is_empty() {
            println!("  Context after:");
            for ctx in &entry.context_after {
                println!("    {}", ctx);
            }
        }
        
        println!();
    }
    
    Ok(())
}

pub fn extract_code_diff_timeline(
    session_path: &str,
    search_terms: &[&str],
    context_size: usize,
) -> Result<CodeDiffTimeline> {
    let full_path = resolve_session_path(session_path)?;
    let session_id = extract_session_id_from_path(&full_path)?;
    let content = fs::read_to_string(&full_path)?;
    
    let all_messages = parse_session_messages(&content)?;
    let code_change_indices = find_code_change_messages(&all_messages);
    
    let code_changes: Vec<CodeDiffEntry> = code_change_indices
        .into_iter()
        .map(|index| {
            let msg = &all_messages[index];
            let context_before = extract_context_messages(&all_messages, index, context_size, true);
            let context_after = extract_context_messages(&all_messages, index, context_size, false);
            let (code_content, language, change_type) = extract_code_from_message(msg);
            
            CodeDiffEntry {
                message_index: index,
                timestamp: msg.timestamp.clone().unwrap_or_default(),
                role: msg.message.as_ref()
                    .and_then(|m| m.role.clone())
                    .unwrap_or_default(),
                code_content,
                language,
                change_type,
                context_before,
                context_after,
            }
        })
        .filter(|entry| {
            // If no search terms provided, include all code changes
            if search_terms.is_empty() {
                return true;
            }
            
            // Check if any search term matches the code content or context
            search_terms.iter().any(|term| {
                let term_lower = term.to_lowercase();
                
                // Check code content
                if entry.code_content.to_lowercase().contains(&term_lower) {
                    return true;
                }
                
                // Check context before
                if entry.context_before.iter().any(|ctx| 
                    ctx.to_lowercase().contains(&term_lower)) {
                    return true;
                }
                
                // Check context after
                if entry.context_after.iter().any(|ctx| 
                    ctx.to_lowercase().contains(&term_lower)) {
                    return true;
                }
                
                false
            })
        })
        .collect();

    Ok(CodeDiffTimeline {
        session_id,
        code_changes,
    })
}

fn find_code_change_messages(messages: &[SessionMessage]) -> Vec<usize> {
    messages
        .iter()
        .enumerate()
        .filter_map(|(index, msg)| {
            if let Some(inner_msg) = &msg.message {
                if let Some(content) = &inner_msg.content {
                    if has_code_content(content) {
                        return Some(index);
                    }
                }
            }
            None
        })
        .collect()
}

fn has_code_content(content: &Content) -> bool {
    match content {
        Content::Array(blocks) => {
            blocks.iter().any(|block| {
                // Check for tool calls that modify code
                if block.r#type == "tool_use" {
                    if let Some(name) = &block.name {
                        return matches!(name.as_str(), "Edit" | "Write" | "MultiEdit" | "Bash");
                    }
                }
                false
            })
        }
        Content::Text(text) => {
            // Check for code blocks in markdown
            text.contains("```")
        }
    }
}

fn extract_code_from_message(msg: &SessionMessage) -> (String, Option<String>, CodeChangeType) {
    if let Some(inner_msg) = &msg.message {
        if let Some(content) = &inner_msg.content {
            match content {
                Content::Array(blocks) => {
                    // Look for tool calls first
                    for block in blocks {
                        if block.r#type == "tool_use" {
                            if let Some(name) = &block.name {
                                let change_type = match name.as_str() {
                                    "Edit" | "MultiEdit" => CodeChangeType::Edit,
                                    "Write" => CodeChangeType::Write,
                                    "Bash" => CodeChangeType::BashCommand,
                                    _ => continue,
                                };
                                
                                let code_content = format_tool_content(name, &block.input);
                                return (code_content, None, change_type);
                            }
                        }
                    }
                    
                    // Look for code blocks in text blocks
                    for block in blocks {
                        if block.r#type == "text" {
                            if let Some(text) = &block.text {
                                if let Some((code, lang)) = extract_code_block_from_text(text) {
                                    return (code, lang, CodeChangeType::CodeBlock);
                                }
                            }
                        }
                    }
                }
                Content::Text(text) => {
                    if let Some((code, lang)) = extract_code_block_from_text(text) {
                        return (code, lang, CodeChangeType::CodeBlock);
                    }
                }
            }
        }
    }
    
    ("".to_string(), None, CodeChangeType::CodeBlock)
}

fn extract_code_block_from_text(text: &str) -> Option<(String, Option<String>)> {
    // Find code blocks manually to handle multiline content
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;
    
    while i < lines.len() {
        let line = lines[i];
        if line.starts_with("```") {
            // Extract language if present
            let language = if line.len() > 3 {
                let lang_part = &line[3..].trim();
                if lang_part.is_empty() {
                    None
                } else {
                    Some(lang_part.to_string())
                }
            } else {
                None
            };
            
            // Find the closing fence
            let mut code_lines = Vec::new();
            i += 1;
            while i < lines.len() && !lines[i].starts_with("```") {
                code_lines.push(lines[i]);
                i += 1;
            }
            
            if !code_lines.is_empty() {
                return Some((code_lines.join("\n"), language));
            }
        }
        i += 1;
    }
    
    None
}

fn format_tool_content(tool_name: &str, input: &Option<serde_json::Value>) -> String {
    if let Some(input_val) = input {
        match tool_name {
            "Write" => {
                let file_path = input_val.get("file_path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let content = input_val.get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                
                format!("📝 Write to {}\n{}", file_path, content)
            },
            "Edit" | "MultiEdit" => {
                let file_path = input_val.get("file_path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let old_string = input_val.get("old_string")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let new_string = input_val.get("new_string")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                
                format!("✏️ Edit {}\n--- Replace:\n{}\n+++ With:\n{}", 
                       file_path, old_string, new_string)
            },
            "Bash" => {
                let command = input_val.get("command")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let description = input_val.get("description")
                    .and_then(|v| v.as_str());
                
                if let Some(desc) = description {
                    format!("🔧 {} ({})", command, desc)
                } else {
                    format!("🔧 {}", command)
                }
            },
            _ => format!("🔧 {} with input: {}", tool_name, input_val)
        }
    } else {
        format!("🔧 {}", tool_name)
    }
}

pub fn display_code_diff_timeline(timeline: &CodeDiffTimeline) -> Result<()> {
    println!("=== Code Diff Timeline for session {} ===\n", timeline.session_id);
    
    for entry in &timeline.code_changes {
        let change_type_label = match entry.change_type {
            CodeChangeType::Edit => "Edit",
            CodeChangeType::Write => "Write",
            CodeChangeType::CodeBlock => "Code Block",
            CodeChangeType::BashCommand => "Bash",
        };
        
        let language_info = entry.language.as_deref().unwrap_or("unknown");
        
        println!("[Message {} - {}] {}: {} ({})", 
                 entry.message_index, 
                 entry.timestamp, 
                 entry.role, 
                 change_type_label,
                 language_info);
        
        if !entry.context_before.is_empty() {
            println!("  Context before:");
            for ctx in &entry.context_before {
                println!("    {}", ctx);
            }
        }
        
        println!("  Code:");
        for line in entry.code_content.lines() {
            println!("    {}", line);
        }
        
        if !entry.context_after.is_empty() {
            println!("  Context after:");
            for ctx in &entry.context_after {
                println!("    {}", ctx);
            }
        }
        
        println!();
    }
    
    Ok(())
}