//! Find references functionality for the Qi Language Server
//!
//! This module provides "find all references" functionality, allowing
//! users to find all occurrences of a symbol in their codebase.

use anyhow::Result;
use log::debug;
use lsp_server::{Connection, Message, Request, Response};
use lsp_types::{
    Location, ReferenceParams,
};

use crate::document::DocumentManager;

/// Handle references requests
pub async fn handle_references(
    connection: &Connection,
    request: Request,
    document_manager: &DocumentManager,
) -> Result<()> {
    debug!("Handling references request");

    let params: ReferenceParams = serde_json::from_value(request.params)?;
    let uri = params.text_document_position.text_document.uri.to_string();
    let position = params.text_document_position.position;

    // Try to find all references
    let references = find_all_references(&uri, position.into(), params.context.include_declaration, document_manager);

    let response = Response {
        id: request.id,
        result: Some(serde_json::to_value(references)?),
        error: None,
    };

    connection.sender.send(Message::Response(response))?;
    debug!("Sent references response");

    Ok(())
}

/// Find all references to a symbol at the given position
fn find_all_references(
    uri: &str,
    position: crate::document::DocumentPosition,
    include_declaration: bool,
    document_manager: &DocumentManager,
) -> Vec<Location> {
    // Get the word at the cursor position
    let word = match get_word_at_position(uri, position.into(), document_manager) {
        Some(word) => word,
        None => return Vec::new(),
    };

    let mut references = Vec::new();

    // Find definition if requested
    if include_declaration {
        if let Some(definition) = crate::definition::find_definition_in_ast(
            &word,
            uri,
            document_manager.get_document_ast(uri).unwrap().as_ref(),
            document_manager,
        ) {
            references.push(definition);
        }
    }

    // Find all usages in the current document
    references.extend(find_references_in_document(&word, uri, document_manager));

    // Find references in other workspace documents
    let all_uris = document_manager.get_all_uris();
    for other_uri in all_uris {
        if other_uri != uri {
            let doc_references = find_references_in_document(&word, &other_uri, document_manager);
            references.extend(doc_references);
        }
    }

    references
}

/// Get the word at the current cursor position
fn get_word_at_position(
    uri: &str,
    position: lsp_types::Position,
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

/// Find all references to a symbol in a specific document
fn find_references_in_document(
    symbol: &str,
    uri: &str,
    document_manager: &DocumentManager,
) -> Vec<Location> {
    let mut references = Vec::new();

    if let Some(ast) = document_manager.get_document_ast(uri) {
        // Search through the AST for all occurrences of the symbol
        find_references_in_ast(symbol, uri, &qi_compiler::parser::AstNode::程序((*ast).clone()), &mut references, document_manager);
    }

    references
}

/// Recursively search the AST for references to a symbol
fn find_references_in_ast(
    symbol: &str,
    uri: &str,
    node: &qi_compiler::parser::AstNode,
    references: &mut Vec<Location>,
    document_manager: &DocumentManager,
) {
    use qi_compiler::parser::AstNode;

    match node {
        // Check if this node itself is a reference to the symbol
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
        AstNode::字段访问表达式(field_access) => {
            if field_access.field == symbol {
                if let Some(location) = crate::definition::span_to_location(&field_access.span, uri, document_manager) {
                    references.push(location);
                }
            }
            // Check object
            find_references_in_ast(symbol, uri, &field_access.object, references, document_manager);
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
        AstNode::数组访问表达式(array_access) => {
            // Check array and index
            find_references_in_ast(symbol, uri, &array_access.array, references, document_manager);
            find_references_in_ast(symbol, uri, &array_access.index, references, document_manager);
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
        AstNode::取地址表达式(address_of_expr) => {
            find_references_in_ast(symbol, uri, &address_of_expr.expression, references, document_manager);
        }
        AstNode::解引用表达式(dereference_expr) => {
            find_references_in_ast(symbol, uri, &dereference_expr.expression, references, document_manager);
        }
        // Add other node types as needed
        _ => {}
    }
}