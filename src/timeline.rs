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