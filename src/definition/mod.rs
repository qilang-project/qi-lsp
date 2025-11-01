//! Go to definition functionality for the Qi Language Server
//!
//! This module provides "go to definition" functionality, allowing
//! users to navigate to the definition of symbols in their code.

use anyhow::Result;
use log::debug;
use lsp_server::{Connection, Message, Request, Response};
use lsp_types::{
    GotoDefinitionParams, GotoDefinitionResponse, Location, Position, Range, Uri,
};

use crate::document::DocumentManager;

/// Handle definition requests
pub async fn handle_definition(
    connection: &Connection,
    request: Request,
    document_manager: &DocumentManager,
) -> Result<()> {
    debug!("Handling definition request");

    let params: GotoDefinitionParams = serde_json::from_value(request.params)?;
    let uri = params.text_document_position_params.text_document.uri.to_string();
    let position = params.text_document_position_params.position;

    // Try to find the definition
    let definition_response = find_definition(&uri, position, document_manager);

    let response = Response {
        id: request.id,
        result: Some(serde_json::to_value(definition_response)?),
        error: None,
    };

    connection.sender.send(Message::Response(response))?;
    debug!("Sent definition response");

    Ok(())
}

/// Find the definition of a symbol at the given position
fn find_definition(
    uri: &str,
    position: Position,
    document_manager: &DocumentManager,
) -> Option<GotoDefinitionResponse> {
    // Get the word at the cursor position
    let word = get_word_at_position(uri, position, document_manager)?;

    // Try to find the definition in the current document's AST
    if let Some(ast) = document_manager.get_document_ast(uri) {
        if let Some(location) = find_definition_in_ast(&word, uri, &ast, document_manager) {
            return Some(GotoDefinitionResponse::Scalar(location));
        }
    }

    // Search in other workspace documents
    let all_uris = document_manager.get_all_uris();
    for other_uri in all_uris {
        if other_uri != uri {
            if let Some(ast) = document_manager.get_document_ast(&other_uri) {
                if let Some(location) = find_definition_in_ast(&word, &other_uri, &ast, document_manager) {
                    return Some(GotoDefinitionResponse::Scalar(location));
                }
            }
        }
    }

    None
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

/// Find the definition of a symbol in the AST
pub fn find_definition_in_ast(
    symbol: &str,
    uri: &str,
    ast: &qi_compiler::parser::Program,
    document_manager: &DocumentManager,
) -> Option<Location> {
    // Search through the AST for the symbol definition
    for statement in &ast.statements {
        if let Some(location) = search_statement_for_definition(symbol, uri, statement, document_manager) {
            return Some(location);
        }
    }

    None
}

/// Search a statement for a symbol definition
fn search_statement_for_definition(
    symbol: &str,
    uri: &str,
    statement: &qi_compiler::parser::AstNode,
    document_manager: &DocumentManager,
) -> Option<Location> {
    use qi_compiler::parser::AstNode;

    match statement {
        AstNode::函数声明(func_decl) => {
            if func_decl.name == symbol {
                return span_to_location(&func_decl.span, uri, document_manager);
            }
        }
        AstNode::异步函数声明(async_func_decl) => {
            if async_func_decl.name == symbol {
                return span_to_location(&async_func_decl.span, uri, document_manager);
            }
        }
        AstNode::变量声明(var_decl) => {
            if var_decl.name == symbol {
                return span_to_location(&var_decl.span, uri, document_manager);
            }
        }
        AstNode::结构体声明(struct_decl) => {
            if struct_decl.name == symbol {
                return span_to_location(&struct_decl.span, uri, document_manager);
            }
            // Also search in struct fields
            for field in &struct_decl.fields {
                if field.name == symbol {
                    return span_to_location(&field.span, uri, document_manager);
                }
            }
        }
        AstNode::枚举声明(enum_decl) => {
            if enum_decl.name == symbol {
                return span_to_location(&enum_decl.span, uri, document_manager);
            }
            // Also search in enum variants
            for variant in &enum_decl.variants {
                if variant.name == symbol {
                    return span_to_location(&variant.span, uri, document_manager);
                }
            }
        }
        AstNode::方法声明(method_decl) => {
            if method_decl.method_name == symbol {
                return span_to_location(&method_decl.span, uri, document_manager);
            }
        }
        AstNode::块语句(block_stmt) => {
            // Search recursively in block statements
            for stmt in &block_stmt.statements {
                if let Some(location) = search_statement_for_definition(symbol, uri, stmt, document_manager) {
                    return Some(location);
                }
            }
        }
        AstNode::如果语句(if_stmt) => {
            // Search in then branch
            for stmt in &if_stmt.then_branch {
                if let Some(location) = search_statement_for_definition(symbol, uri, stmt, document_manager) {
                    return Some(location);
                }
            }
            // Search in else branch
            if let Some(else_branch) = &if_stmt.else_branch {
                if let Some(location) = search_statement_for_definition(symbol, uri, else_branch, document_manager) {
                    return Some(location);
                }
            }
        }
        AstNode::当语句(while_stmt) => {
            for stmt in &while_stmt.body {
                if let Some(location) = search_statement_for_definition(symbol, uri, stmt, document_manager) {
                    return Some(location);
                }
            }
        }
        AstNode::对于语句(for_stmt) => {
            for stmt in &for_stmt.body {
                if let Some(location) = search_statement_for_definition(symbol, uri, stmt, document_manager) {
                    return Some(location);
                }
            }
        }
        AstNode::循环语句(loop_stmt) => {
            for stmt in &loop_stmt.body {
                if let Some(location) = search_statement_for_definition(symbol, uri, stmt, document_manager) {
                    return Some(location);
                }
            }
        }
        // Handle other statement types as needed
        _ => {}
    }

    None
}

/// Convert a span to an LSP location
pub fn span_to_location(
    span: &qi_compiler::lexer::tokens::Span,
    uri: &str,
    document_manager: &DocumentManager,
) -> Option<Location> {
    let url = uri.parse::<Uri>().ok()?;
    let range = span_to_range(span, uri, document_manager)?;

    Some(Location { uri: url, range })
}

/// Convert a span to an LSP range
fn span_to_range(
    span: &qi_compiler::lexer::tokens::Span,
    uri: &str,
    document_manager: &DocumentManager,
) -> Option<Range> {
    let content = document_manager.get_document_content(uri)?;
    let lines: Vec<&str> = content.lines().collect();

    if lines.is_empty() {
        return None;
    }

    // Find line and character positions from byte offsets
    let mut byte_offset = 0;
    let mut start_line = 0;
    let mut start_char = 0;
    let mut end_line = 0;
    let mut end_char = 0;

    for (line_idx, line) in lines.iter().enumerate() {
        let line_start = byte_offset;
        let line_end = byte_offset + line.len() + 1; // +1 for newline

        if span.start >= line_start && span.start < line_end {
            start_line = line_idx;
            start_char = span.start - line_start;
        }

        if span.end >= line_start && span.end <= line_end {
            end_line = line_idx;
            end_char = span.end - line_start;
            break;
        }

        byte_offset = line_end;
    }

    Some(Range {
        start: Position {
            line: start_line as u32,
            character: start_char as u32,
        },
        end: Position {
            line: end_line as u32,
            character: end_char as u32,
        },
    })
}

/// Find all references to a symbol
pub fn find_references(
    symbol: &str,
    uri: &str,
    position: Position,
    document_manager: &DocumentManager,
) -> Vec<Location> {
    let mut references = Vec::new();

    // Find references in current document
    if let Some(ast) = document_manager.get_document_ast(uri) {
        references.extend(find_symbol_references(symbol, uri, &ast, document_manager));
    }

    // Find references in other workspace documents
    let all_uris = document_manager.get_all_uris();
    for other_uri in all_uris {
        if other_uri != uri {
            if let Some(ast) = document_manager.get_document_ast(&other_uri) {
                references.extend(find_symbol_references(symbol, &other_uri, &ast, document_manager));
            }
        }
    }

    references
}

/// Find references to a symbol in a specific document
fn find_symbol_references(
    symbol: &str,
    uri: &str,
    ast: &qi_compiler::parser::Program,
    document_manager: &DocumentManager,
) -> Vec<Location> {
    let references = Vec::new();
    // Simplified reference search - TODO: implement full AST traversal
        // find_references_in_ast(symbol, uri, &qi_compiler::parser::AstNode::程序((**ast).clone()), &mut references, document_manager);
    references
}

/// Recursively search AST for symbol references
fn find_references_in_ast(
    symbol: &str,
    uri: &str,
    node: &qi_compiler::parser::AstNode,
    references: &mut Vec<Location>,
    document_manager: &DocumentManager,
) {
    use qi_compiler::parser::AstNode;

    match node {
        // Check if this node is a reference to the symbol
        AstNode::标识符表达式(ident_expr) => {
            if ident_expr.name == symbol {
                if let Some(location) = span_to_location(&ident_expr.span, uri, document_manager) {
                    references.push(location);
                }
            }
        }
        AstNode::函数调用表达式(func_call) => {
            if func_call.callee == symbol {
                if let Some(location) = span_to_location(&func_call.span, uri, document_manager) {
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
                if let Some(location) = span_to_location(&method_call.span, uri, document_manager) {
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
                if let Some(location) = span_to_location(&field_access.span, uri, document_manager) {
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
        AstNode::表达式语句(expr_stmt) => {
            find_references_in_ast(symbol, uri, &expr_stmt.expression, references, document_manager);
        }
        AstNode::返回语句(return_stmt) => {
            if let Some(value) = &return_stmt.value {
                find_references_in_ast(symbol, uri, value, references, document_manager);
            }
        }
        // Handle statement nodes
        AstNode::块语句(block_stmt) => {
            for stmt in &block_stmt.statements {
                find_references_in_ast(symbol, uri, stmt, references, document_manager);
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
        // Handle declaration nodes (these are definitions, not references)
        AstNode::函数声明(func_decl) => {
            // Search in function body
            for stmt in &func_decl.body {
                find_references_in_ast(symbol, uri, stmt, references, document_manager);
            }
        }
        AstNode::异步函数声明(async_func_decl) => {
            // Search in async function body
            for stmt in &async_func_decl.body {
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
                find_references_in_ast(symbol, uri, &AstNode::方法声明(method.clone()), references, document_manager);
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
        _ => {}
    }
}