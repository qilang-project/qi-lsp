//! Rename functionality for the Qi Language Server
//!
//! This module provides rename (refactoring) functionality for Qi source code,
//! allowing users to safely rename symbols across their codebase.

use anyhow::Result;
use log::{debug, warn};
use lsp_server::{Connection, Message, Request, Response};
use lsp_types::{
    RenameParams, TextEdit, WorkspaceEdit, Position,
};
use std::collections::HashMap;

use qi_compiler::parser::AstNode;
use crate::document::DocumentManager;

/// Handle rename requests
pub async fn handle_rename(
    connection: &Connection,
    request: Request,
    document_manager: &DocumentManager,
) -> Result<()> {
    debug!("Handling rename request");

    let params: RenameParams = serde_json::from_value(request.params)?;
    let uri = params.text_document_position.text_document.uri.to_string();
    let position = params.text_document_position.position;
    let new_name = params.new_name;

    // Try to perform rename
    let workspace_edit = perform_rename(&uri, position, &new_name, document_manager);

    let response = Response {
        id: request.id,
        result: Some(serde_json::to_value(workspace_edit)?),
        error: None,
    };

    connection.sender.send(Message::Response(response))?;
    debug!("Sent rename response");

    Ok(())
}

/// Perform rename operation
pub fn perform_rename(
    uri: &str,
    position: Position,
    new_name: &str,
    document_manager: &DocumentManager,
) -> Option<WorkspaceEdit> {
    // Get the word at cursor position
    let old_name = match get_word_at_position(uri, position, document_manager) {
        Some(word) => word,
        None => {
            warn!("No symbol found at position {:?}", position);
            return None;
        }
    };

    // Validate new name
    if !is_valid_identifier(new_name) {
        warn!("Invalid new name: '{}'", new_name);
        return None;
    }

    // Safety check: if new name is the same as old name, return empty edit
    if old_name == new_name {
        debug!("New name is same as old name, returning empty edit");
        return Some(WorkspaceEdit {
            changes: None,
            document_changes: None,
            change_annotations: None,
        });
    }

    // Check if rename is safe
    if !is_rename_safe(&old_name, new_name, document_manager) {
        warn!("Rename from '{}' to '{}' is not safe", old_name, new_name);
        return None;
    }

    // Find all references
    let references = find_all_references(&old_name, uri, document_manager);
    if references.is_empty() {
        debug!("No references found for symbol '{}'", old_name);
        return Some(WorkspaceEdit {
            changes: None,
            document_changes: None,
            change_annotations: None,
        });
    }

    // Group references by document
    let mut changes_by_document: HashMap<lsp_types::Uri, Vec<TextEdit>> = HashMap::new();

    for location in references {
        let doc_uri = location.uri;
        let text_edit = TextEdit {
            range: location.range,
            new_text: new_name.to_string(),
        };

        changes_by_document
            .entry(doc_uri)
            .or_insert_with(Vec::new)
            .push(text_edit);
    }

    // Create workspace edit
    Some(WorkspaceEdit {
        changes: Some(changes_by_document),
        document_changes: None,
        change_annotations: None,
    })
}

/// Get the word at the current cursor position
fn get_word_at_position(
    uri: &str,
    position: Position,
    document_manager: &DocumentManager,
) -> Option<String> {
    let line_content = document_manager.get_line_content(uri, position.line as usize)?;
    let char_pos = position.character as usize;

    if char_pos >= line_content.len() {
        return None;
    }

    // Find the start and end of the word
    let mut start = char_pos;
    let mut end = char_pos;

    // Find word start (move left until non-word character)
    while start > 0 {
        let ch = line_content.chars().nth(start - 1)?;
        if !is_word_char(ch) {
            break;
        }
        start -= 1;
    }

    // Find word end (move right until non-word character)
    while end < line_content.len() {
        let ch = line_content.chars().nth(end)?;
        if !is_word_char(ch) {
            break;
        }
        end += 1;
    }

    if start < end {
        Some(line_content[start..end].to_string())
    } else {
        None
    }
}

/// Check if a character is part of a word (identifier or Chinese character)
fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_' || ch as u32 >= 0x4E00 && ch as u32 <= 0x9FFF
}

/// Validate if a string is a valid identifier
fn is_valid_identifier(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }

    // Check if name starts with a valid character
    if let Some(first_char) = name.chars().next() {
        let char_code = first_char as u32;
        if !first_char.is_alphabetic() && first_char != '_' && (char_code < 0x4E00 || char_code > 0x9FFF) {
            return false;
        }
    }

    // Check all characters are valid
    for ch in name.chars() {
        if !is_word_char(ch) {
            return false;
        }
    }

    // Check for Qi language keywords
    if is_qi_keyword(name) {
        return false;
    }

    true
}

/// Check if a string is a Qi language keyword
fn is_qi_keyword(name: &str) -> bool {
    match name {
        "包" | "导入" | "导出" | "公开" | "私有" | "可变" | "不可变" |
        "函数" | "异步函数" | "方法" | "返回" | "等待" | "循环" |
        "当" | "对于" | "如果" | "否则" | "匹配" | "跳出" | "继续" |
        "结构体" | "枚举" | "接口" | "实现" | "使用" | "作为" |
        "整数" | "浮点数" | "字符串" | "布尔" | "字符" | "空" |
        "真" | "假" | "无" => true,
        _ => false,
    }
}

/// Check if rename operation is safe
fn is_rename_safe(
    old_name: &str,
    new_name: &str,
    document_manager: &DocumentManager,
) -> bool {
    // Check if old name is a keyword
    if is_qi_keyword(old_name) {
        warn!("Cannot rename keyword: '{}'", old_name);
        return false;
    }

    // Check if new name conflicts with existing symbols in scope
    // This is a simplified check - in a full implementation we would
    // need to analyze the scope at the cursor position
    let all_uris = document_manager.get_all_uris();

    for uri in all_uris {
        if let Some(ast) = document_manager.get_document_ast(&uri) {
            if has_symbol_conflict(&ast, new_name) {
                warn!("New name '{}' conflicts with existing symbol", new_name);
                return false;
            }
        }
    }

    true
}

/// Check if AST contains a symbol that conflicts with the new name
fn has_symbol_conflict(ast: &qi_compiler::parser::Program, new_name: &str) -> bool {
    use qi_compiler::parser::AstNode;

    for statement in &ast.statements {
        if symbol_node_contains_name(statement, new_name) {
            return true;
        }
    }

    false
}

/// Check if an AST node contains a symbol with the given name
fn symbol_node_contains_name(node: &qi_compiler::parser::AstNode, name: &str) -> bool {

    match node {
        AstNode::函数声明(func_decl) => func_decl.name == name,
        AstNode::结构体声明(struct_decl) => struct_decl.name == name,
        AstNode::枚举声明(enum_decl) => enum_decl.name == name,
        AstNode::变量声明(var_decl) => var_decl.name == name,
        AstNode::方法声明(method_decl) => method_decl.method_name == name,

        // Recursively check nested nodes
        AstNode::块语句(block_stmt) => {
            block_stmt.statements.iter().any(|stmt| symbol_node_contains_name(stmt, name))
        }
        AstNode::如果语句(if_stmt) => {
            if_stmt.then_branch.iter().any(|stmt| symbol_node_contains_name(stmt, name)) ||
            if_stmt.else_branch.as_ref().map_or(false, |stmt| symbol_node_contains_name(stmt, name))
        }
        AstNode::当语句(while_stmt) => {
            while_stmt.body.iter().any(|stmt| symbol_node_contains_name(stmt, name))
        }
        AstNode::对于语句(for_stmt) => {
            for_stmt.body.iter().any(|stmt| symbol_node_contains_name(stmt, name))
        }
        AstNode::循环语句(loop_stmt) => {
            loop_stmt.body.iter().any(|stmt| symbol_node_contains_name(stmt, name))
        }
        AstNode::函数声明(func_decl) => {
            func_decl.body.iter().any(|stmt| symbol_node_contains_name(stmt, name))
        }
        AstNode::方法声明(method_decl) => {
            method_decl.body.iter().any(|stmt| symbol_node_contains_name(stmt, name))
        }
        _ => false,
    }
}

/// Find all references to a symbol (reusing references module functionality)
fn find_all_references(
    symbol: &str,
    uri: &str,
    document_manager: &DocumentManager,
) -> Vec<lsp_types::Location> {
    // Reuse the references functionality
    let mut references = Vec::new();

    // Find definition if it exists
    if let Some(definition) = crate::definition::find_definition_in_ast(
        symbol,
        uri,
        document_manager.get_document_ast(uri).unwrap().as_ref(),
        document_manager,
    ) {
        references.push(definition);
    }

    // Find all usages in the current document
    references.extend(find_all_references_in_document(symbol, uri, document_manager));

    // Find references in other workspace documents
    let all_uris = document_manager.get_all_uris();
    for other_uri in all_uris {
        if other_uri != uri {
            let doc_references = find_all_references_in_document(symbol, &other_uri, document_manager);
            references.extend(doc_references);
        }
    }

    references
}

/// Find all references to a symbol in a specific document
fn find_all_references_in_document(
    symbol: &str,
    uri: &str,
    document_manager: &DocumentManager,
) -> Vec<lsp_types::Location> {
    let mut references = Vec::new();

    if let Some(ast) = document_manager.get_document_ast(uri) {
        find_references_in_ast(symbol, uri, &qi_compiler::parser::AstNode::程序((*ast).clone()), &mut references, document_manager);
    }

    references
}

/// Recursively search the AST for references to a symbol
fn find_references_in_ast(
    symbol: &str,
    uri: &str,
    node: &qi_compiler::parser::AstNode,
    references: &mut Vec<lsp_types::Location>,
    document_manager: &DocumentManager,
) {
    use qi_compiler::parser::AstNode;

    match node {
        AstNode::标识符表达式(ident_expr) => {
            if ident_expr.name == symbol {
                if let Some(location) = crate::definition::span_to_location(&ident_expr.span, uri, document_manager) {
                    references.push(location);
                }
            }
        }
        AstNode::函数调用表达式(func_call) => {
            if func_call.callee == symbol {
                if let Some(location) = crate::definition::span_to_location(&func_call.span, uri, document_manager) {
                    references.push(location);
                }
            }
            // Check arguments
            for arg in &func_call.arguments {
                find_references_in_ast(symbol, uri, arg, references, document_manager);
            }
        }
        AstNode::方法调用表达式(method_call) => {
            if method_call.method_name == symbol {
                if let Some(location) = crate::definition::span_to_location(&method_call.span, uri, document_manager) {
                    references.push(location);
                }
            }
            // Check object and arguments
            find_references_in_ast(symbol, uri, &method_call.object, references, document_manager);
            for arg in &method_call.arguments {
                find_references_in_ast(symbol, uri, arg, references, document_manager);
            }
        }
        AstNode::赋值表达式(assign_expr) => {
            // Check target and value
            find_references_in_ast(symbol, uri, &assign_expr.target, references, document_manager);
            find_references_in_ast(symbol, uri, &assign_expr.value, references, document_manager);
        }
        AstNode::二元操作表达式(binary_expr) => {
            // Check left and right operands
            find_references_in_ast(symbol, uri, &binary_expr.left, references, document_manager);
            find_references_in_ast(symbol, uri, &binary_expr.right, references, document_manager);
        }
        AstNode::表达式语句(expr_stmt) => {
            find_references_in_ast(symbol, uri, &expr_stmt.expression, references, document_manager);
        }
        AstNode::返回语句(return_stmt) => {
            if let Some(value) = &return_stmt.value {
                find_references_in_ast(symbol, uri, value, references, document_manager);
            }
        }
        AstNode::如果语句(if_stmt) => {
            find_references_in_ast(symbol, uri, &if_stmt.condition, references, document_manager);
            for stmt in &if_stmt.then_branch {
                find_references_in_ast(symbol, uri, stmt, references, document_manager);
            }
            if let Some(else_branch) = &if_stmt.else_branch {
                find_references_in_ast(symbol, uri, else_branch, references, document_manager);
            }
        }
        AstNode::块语句(block_stmt) => {
            for stmt in &block_stmt.statements {
                find_references_in_ast(symbol, uri, stmt, references, document_manager);
            }
        }
        AstNode::当语句(while_stmt) => {
            find_references_in_ast(symbol, uri, &while_stmt.condition, references, document_manager);
            for stmt in &while_stmt.body {
                find_references_in_ast(symbol, uri, stmt, references, document_manager);
            }
        }
        AstNode::对于语句(for_stmt) => {
            find_references_in_ast(symbol, uri, &for_stmt.range, references, document_manager);
            for stmt in &for_stmt.body {
                find_references_in_ast(symbol, uri, stmt, references, document_manager);
            }
        }
        AstNode::循环语句(loop_stmt) => {
            for stmt in &loop_stmt.body {
                find_references_in_ast(symbol, uri, stmt, references, document_manager);
            }
        }
        AstNode::函数声明(func_decl) => {
            // Search in function body (handles both regular and async functions)
            for stmt in &func_decl.body {
                find_references_in_ast(symbol, uri, stmt, references, document_manager);
            }
        }
        AstNode::方法声明(method_decl) => {
            // Search in method body
            for stmt in &method_decl.body {
                find_references_in_ast(symbol, uri, stmt, references, document_manager);
            }
        }
        AstNode::结构体声明(struct_decl) => {
            // Search in struct methods
            for method in &struct_decl.methods {
                find_references_in_ast(symbol, uri, &qi_compiler::parser::AstNode::方法声明(method.clone()), references, document_manager);
            }
        }
        AstNode::等待表达式(await_expr) => {
            find_references_in_ast(symbol, uri, &await_expr.expression, references, document_manager);
        }
        AstNode::字符串连接表达式(string_concat) => {
            find_references_in_ast(symbol, uri, &string_concat.left, references, document_manager);
            find_references_in_ast(symbol, uri, &string_concat.right, references, document_manager);
        }
        AstNode::结构体实例化表达式(struct_literal) => {
            for field in &struct_literal.fields {
                find_references_in_ast(symbol, uri, &field.value, references, document_manager);
            }
        }
        AstNode::数组字面量表达式(array_literal) => {
            for element in &array_literal.elements {
                find_references_in_ast(symbol, uri, element, references, document_manager);
            }
        }
        // Add other node types as needed
        _ => {}
    }
}