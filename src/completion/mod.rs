//! Code completion functionality for the Qi Language Server
//!
//! This module provides intelligent code completion suggestions
//! for Qi source code, including keywords, functions, variables, and more.

#![allow(dead_code)]

use anyhow::Result;
use log::debug;
use lsp_server::{Connection, Message, Request, Response};
use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionParams, CompletionResponse,
    InsertTextFormat, Position,
};

use crate::document::DocumentManager;

/// Handle completion requests
pub async fn handle_completion(
    connection: &Connection,
    request: Request,
    document_manager: &DocumentManager,
) -> Result<()> {
    debug!("Handling completion request");

    let params: CompletionParams = serde_json::from_value(request.params)?;
    let uri = params.text_document_position.text_document.uri.to_string();
    let position = params.text_document_position.position;

    // Get document and context
    let mut completion_items = Vec::new();

    // Add keyword completions
    completion_items.extend(get_keyword_completions());

    // Add type completions
    completion_items.extend(get_type_completions());

    // Get document-specific completions if AST is available
    if let Some(ast) = document_manager.get_document_ast(&uri) {
        completion_items.extend(get_context_completions(&ast, &uri, position, document_manager));
    }

    // Filter completions based on current context
    completion_items = filter_completions_by_context(completion_items, &uri, position, document_manager);

    let items_count = completion_items.len();
    let response = CompletionResponse::Array(completion_items);
    let response_json = serde_json::to_value(response)?;

    let response = Response {
        id: request.id,
        result: Some(response_json),
        error: None,
    };

    connection.sender.send(Message::Response(response))?;
    debug!("Sent completion response with {} items", items_count);

    Ok(())
}

/// Get Qi keyword completions
fn get_keyword_completions() -> Vec<CompletionItem> {
    let keywords = vec![
        ("包", "package", "包 主程序;"),
        ("函数", "function", "函数 ${1:name}(${2:param}: ${3:类型})${4:: ${5:返回类型}} {\n    ${6:// 函数体}\n    ${7:返回 ${8:result};}\n}"),
        ("异步函数", "async function", "异步函数 ${1:name}(${2:param}: ${3:类型})${4:: ${5:返回类型}} {\n    ${6:// 异步函数体}\n    ${7:等待 ${8:async_call};}\n    ${9:返回 ${10:result};}\n}"),
        ("变量", "variable", "变量 ${1:name}: ${2:类型} = ${3:value};"),
        ("如果", "if", "如果 ${1:condition} {\n    ${2:// 条件为真时执行}\n} 否则 {\n    ${3:// 条件为假时执行}\n}"),
        ("否则", "else", "否则 {\n    ${1:// else 分支内容}\n}"),
        ("当", "while", "当 ${1:condition} {\n    ${2:// 循环体内容}\n}"),
        ("对于", "for", "对于 ${1:item} 在 ${2:collection} 中 {\n    ${3:// 处理每个元素}\n}"),
        ("循环", "loop", "循环 {\n    ${1:// 循环体}\n    如果 ${2:break_condition} {\n        跳出;\n    }\n}"),
        ("返回", "return", "返回 ${1:value};"),
        ("类型", "struct", "类型 ${1:Name} {\n    ${2:字段}: ${3:类型};\n}"),
        ("枚举", "enum", "枚举 ${1:Name} {\n    ${2:变体},\n}"),
        ("公开", "public", "公开"),
        ("私有", "private", "私有"),
        ("导入", "import", "导入 标准库.${1:模块名};"),
        ("打印行", "println", "打印行(${1:\"内容\"});"),
        ("打印", "print", "打印(${1:\"内容\"});"),
        ("等待", "await", "等待 ${1:async_call};"),
        ("取地址", "address-of", "取地址 ${1:variable}"),
        ("解引用", "dereference", "解引用 ${1:pointer}"),
        ("启动", "spawn goroutine", "启动 ${1:函数名}();"),
        ("选择", "match/select", "选择 ${1:value} {\n    情况 ${2:pattern} => ${3:result},\n    情况 _ => ${4:default},\n}"),
    ];

    keywords
        .into_iter()
        .enumerate()
        .map(|(index, (keyword, detail, insert_text))| CompletionItem {
            label: keyword.to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some(detail.to_string()),
            documentation: None,
            deprecated: Some(false),
            preselect: Some(false),
            sort_text: Some(format!("{:04}", index)),
            filter_text: Some(keyword.to_string()),
            insert_text: Some(insert_text.to_string()),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            insert_text_mode: Some(lsp_types::InsertTextMode::ADJUST_INDENTATION),
            label_details: None,
            text_edit: None,
            additional_text_edits: None,
            command: None,
            commit_characters: None,
            data: None,
            tags: None,
        })
        .collect()
}

/// Get Qi type completions
fn get_type_completions() -> Vec<CompletionItem> {
    let types = vec![
        ("整数", "integer", "整数"),
        ("长整数", "long integer", "长整数"),
        ("短整数", "short integer", "短整数"),
        ("字节", "byte", "字节"),
        ("浮点数", "float", "浮点数"),
        ("布尔", "boolean", "布尔"),
        ("字符", "character", "字符"),
        ("字符串", "string", "字符串"),
        ("空", "void", "空"),
        ("数组", "array", "数组"),
        ("字典", "dictionary", "字典"),
        ("列表", "list", "列表"),
        ("集合", "set", "集合"),
        ("指针", "pointer", "指针"),
        ("引用", "reference", "引用"),
        ("可变引用", "mutable reference", "可变引用"),
    ];

    types
        .into_iter()
        .enumerate()
        .map(|(index, (type_name, detail, insert_text))| CompletionItem {
            label: type_name.to_string(),
            kind: Some(CompletionItemKind::TYPE_PARAMETER),
            detail: Some(detail.to_string()),
            documentation: None,
            deprecated: Some(false),
            preselect: Some(false),
            sort_text: Some(format!("type{:04}", index)),
            filter_text: Some(type_name.to_string()),
            insert_text: Some(insert_text.to_string()),
            insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
            insert_text_mode: None,
            label_details: None,
            text_edit: None,
            additional_text_edits: None,
            command: None,
            commit_characters: None,
            data: None,
            tags: None,
        })
        .collect()
}

/// Get context-aware completions from the AST
fn get_context_completions(
    ast: &qi_compiler::parser::Program,
    uri: &str,
    position: Position,
    document_manager: &DocumentManager,
) -> Vec<CompletionItem> {
    let mut completions = Vec::new();

    // Get current line and character position
    let line_content = document_manager.get_line_content(uri, position.line as usize)
        .unwrap_or_default();
    let char_pos = position.character as usize;
    let before_cursor = &line_content[..char_pos.min(line_content.len())];

    // Analyze context based on what was typed before cursor
    let context = analyze_completion_context(before_cursor, uri, position, document_manager);

    match context {
        CompletionContext::VariableDeclaration => {
            // Add type suggestions after `:`
            completions.extend(get_type_completions());
        }
        CompletionContext::FunctionCall => {
            // Add function suggestions
            completions.extend(get_function_completions(ast));
        }
        CompletionContext::StructFieldAccess { struct_name } => {
            // Add field completions for struct
            completions.extend(get_field_completions(ast, &struct_name));
        }
        CompletionContext::MethodAccess => {
            // Add method completions
            completions.extend(get_method_completions(ast));
        }
        CompletionContext::ImportStatement => {
            // Add module suggestions
            completions.extend(get_import_completions());
        }
        CompletionContext::PackageDeclaration => {
            // Add package name suggestions
            completions.extend(get_package_completions());
        }
        CompletionContext::VariableReference => {
            // Add local variables and parameters in scope
            completions.extend(get_scope_completions(ast, uri, position, document_manager));
        }
        CompletionContext::General => {
            // Add all appropriate completions
            completions.extend(get_keyword_completions());
            completions.extend(get_type_completions());
            completions.extend(get_scope_completions(ast, uri, position, document_manager));
        }
    }

    completions
}

/// Completion context types
#[derive(Debug, PartialEq)]
enum CompletionContext {
    VariableDeclaration,
    FunctionCall,
    StructFieldAccess { struct_name: String },
    MethodAccess,
    ImportStatement,
    PackageDeclaration,
    VariableReference,
    General,
}

/// Analyze the completion context from the code before cursor
fn analyze_completion_context(
    before_cursor: &str,
    _uri: &str,
    _position: Position,
    _document_manager: &DocumentManager,
) -> CompletionContext {
    // Trim whitespace
    let trimmed = before_cursor.trim_end();

    // Check for specific patterns
    if trimmed.ends_with(':') {
        CompletionContext::VariableDeclaration
    } else if trimmed.ends_with('.') {
        // Check if it's a struct field access or method call
        if let Some(struct_name) = extract_struct_name_before_dot(trimmed) {
            CompletionContext::StructFieldAccess { struct_name }
        } else {
            CompletionContext::MethodAccess
        }
    } else if trimmed.ends_with('(') {
        CompletionContext::FunctionCall
    } else if trimmed.starts_with("导入") || trimmed.starts_with("import") {
        CompletionContext::ImportStatement
    } else if trimmed.starts_with("包") || trimmed.starts_with("package") {
        CompletionContext::PackageDeclaration
    } else if is_identifier_context(trimmed) {
        CompletionContext::VariableReference
    } else {
        CompletionContext::General
    }
}

/// Extract struct name before a dot
fn extract_struct_name_before_dot(text: &str) -> Option<String> {
    let trimmed = text.trim_end_matches('.');
    if trimmed.is_empty() {
        return None;
    }

    // Find the last identifier before the dot
    let chars = trimmed.chars().rev();
    let mut identifier = String::new();

    for ch in chars {
        if ch.is_whitespace() || ch == '=' || ch == '(' || ch == ')' || ch == '{' || ch == '}' {
            break;
        }
        identifier.insert(0, ch);
    }

    if identifier.is_empty() {
        None
    } else {
        Some(identifier)
    }
}

/// Check if we're in an identifier context
fn is_identifier_context(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return true;
    }

    // Check if the last character suggests we're typing an identifier
    let last_char = trimmed.chars().last().unwrap();
    last_char.is_alphabetic() || last_char == '_' || (last_char as u32 >= 0x4E00 && last_char as u32 <= 0x9FFF)
}

/// Get function completions from AST
fn get_function_completions(ast: &qi_compiler::parser::Program) -> Vec<CompletionItem> {
    let mut completions = Vec::new();

    for statement in &ast.statements {
        if let qi_compiler::parser::AstNode::函数声明(func_decl) = statement {
            let params = func_decl.parameters
                .iter()
                .map(|p| format!("{}: {}", p.name,
                    p.type_annotation.as_ref()
                        .map(format_type_annotation)
                        .unwrap_or_else(|| "_".to_string())))
                .collect::<Vec<_>>()
                .join(", ");

            let return_type = func_decl.return_type.as_ref()
                .map(|t| format!(" -> {}", format_type_annotation(t)))
                .unwrap_or_default();

            completions.push(CompletionItem {
                label: func_decl.name.clone(),
                kind: Some(CompletionItemKind::FUNCTION),
                detail: Some(format!("函数{}{}", params, return_type)),
                documentation: None,
                deprecated: Some(false),
                preselect: Some(false),
                sort_text: Some(format!("func{}", func_decl.name)),
                filter_text: Some(func_decl.name.clone()),
                insert_text: Some(func_decl.name.clone()),
                insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
                insert_text_mode: None,
            label_details: None,
                text_edit: None,
                additional_text_edits: None,
                command: None,
                commit_characters: None,
                data: None,
                tags: None,
            });
        }
    }

    completions
}

/// Get field completions for a struct
fn get_field_completions(ast: &qi_compiler::parser::Program, struct_name: &str) -> Vec<CompletionItem> {
    let mut completions = Vec::new();

    for statement in &ast.statements {
        if let qi_compiler::parser::AstNode::结构体声明(struct_decl) = statement {
            if struct_decl.name == struct_name {
                for field in &struct_decl.fields {
                    completions.push(CompletionItem {
                        label: field.name.clone(),
                        kind: Some(CompletionItemKind::FIELD),
                        detail: Some(format!("字段: {}", format_type_annotation(&field.type_annotation))),
                        documentation: None,
                        deprecated: Some(false),
                        preselect: Some(false),
                        sort_text: Some(format!("field{}", field.name)),
                        filter_text: Some(field.name.clone()),
                        insert_text: Some(field.name.clone()),
                        insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
                        insert_text_mode: None,
            label_details: None,
                        text_edit: None,
                        additional_text_edits: None,
                        command: None,
                        commit_characters: None,
                        data: None,
                        tags: None,
                    });
                }

                // Also add methods
                for method in &struct_decl.methods {
                    completions.push(CompletionItem {
                        label: method.method_name.clone(),
                        kind: Some(CompletionItemKind::METHOD),
                        detail: Some(format!("方法: {}", method.method_name)),
                        documentation: None,
                        deprecated: Some(false),
                        preselect: Some(false),
                        sort_text: Some(format!("method{}", method.method_name)),
                        filter_text: Some(method.method_name.clone()),
                        insert_text: Some(method.method_name.clone()),
                        insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
                        insert_text_mode: None,
            label_details: None,
                        text_edit: None,
                        additional_text_edits: None,
                        command: None,
                        commit_characters: None,
                        data: None,
                        tags: None,
                    });
                }
            }
        }
    }

    completions
}

/// Get method completions (all methods across all structs)
fn get_method_completions(ast: &qi_compiler::parser::Program) -> Vec<CompletionItem> {
    let mut completions = Vec::new();

    for statement in &ast.statements {
        if let qi_compiler::parser::AstNode::结构体声明(struct_decl) = statement {
            for method in &struct_decl.methods {
                completions.push(CompletionItem {
                    label: method.method_name.clone(),
                    kind: Some(CompletionItemKind::METHOD),
                    detail: Some(format!("方法: {}.{}", struct_decl.name, method.method_name)),
                    documentation: None,
                    deprecated: Some(false),
                    preselect: Some(false),
                    sort_text: Some(format!("method{}", method.method_name)),
                    filter_text: Some(method.method_name.clone()),
                    insert_text: Some(method.method_name.clone()),
                    insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
                    insert_text_mode: None,
            label_details: None,
                    text_edit: None,
                    additional_text_edits: None,
                    command: None,
                    commit_characters: None,
                    data: None,
                    tags: None,
                });
            }
        }
    }

    completions
}

/// Get import completions
fn get_import_completions() -> Vec<CompletionItem> {
    let modules = vec![
        ("标准库", "标准库"),
        ("标准库.输入输出", "标准库输入输出"),
        ("标准库.集合", "标准库集合"),
        ("标准库.字符串", "标准库字符串"),
        ("标准库.数学", "标准库数学"),
        ("标准库.文件", "标准库文件"),
        ("标准库.网络", "标准库网络"),
        ("标准库.时间", "标准库时间"),
        ("标准库.并发", "标准库并发"),
    ];

    modules
        .into_iter()
        .map(|(module, detail)| CompletionItem {
            label: module.to_string(),
            kind: Some(CompletionItemKind::MODULE),
            detail: Some(detail.to_string()),
            documentation: None,
            deprecated: Some(false),
            preselect: Some(false),
            sort_text: Some(format!("module{}", module)),
            filter_text: Some(module.to_string()),
            insert_text: Some(module.to_string()),
            insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
            insert_text_mode: None,
            label_details: None,
            text_edit: None,
            additional_text_edits: None,
            command: None,
            commit_characters: None,
            data: None,
            tags: None,
        })
        .collect()
}

/// Get package completions
fn get_package_completions() -> Vec<CompletionItem> {
    let packages = vec![
        ("主程序", "main package"),
        ("库", "library package"),
        ("测试", "test package"),
        ("示例", "example package"),
    ];

    packages
        .into_iter()
        .map(|(package, detail)| CompletionItem {
            label: package.to_string(),
            kind: Some(CompletionItemKind::MODULE),
            detail: Some(detail.to_string()),
            documentation: None,
            deprecated: Some(false),
            preselect: Some(false),
            sort_text: Some(format!("pkg{}", package)),
            filter_text: Some(package.to_string()),
            insert_text: Some(package.to_string()),
            insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
            insert_text_mode: None,
            label_details: None,
            text_edit: None,
            additional_text_edits: None,
            command: None,
            commit_characters: None,
            data: None,
            tags: None,
        })
        .collect()
}

/// Get completions for symbols in scope
fn get_scope_completions(
    ast: &qi_compiler::parser::Program,
    _uri: &str,
    _position: Position,
    _document_manager: &DocumentManager,
) -> Vec<CompletionItem> {
    let mut completions = Vec::new();

    // Extract top-level symbols
    for statement in &ast.statements {
        match statement {
            qi_compiler::parser::AstNode::变量声明(var_decl) => {
                completions.push(CompletionItem {
                    label: var_decl.name.clone(),
                    kind: Some(CompletionItemKind::VARIABLE),
                    detail: var_decl.type_annotation.as_ref()
                        .map(|t| format!("变量: {}", format_type_annotation(t))),
                    documentation: None,
                    deprecated: Some(false),
                    preselect: Some(false),
                    sort_text: Some(format!("var{}", var_decl.name)),
                    filter_text: Some(var_decl.name.clone()),
                    insert_text: Some(var_decl.name.clone()),
                    insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
                    insert_text_mode: None,
            label_details: None,
                    text_edit: None,
                    additional_text_edits: None,
                    command: None,
                    commit_characters: None,
                    data: None,
                    tags: None,
                });
            }
            qi_compiler::parser::AstNode::结构体声明(struct_decl) => {
                completions.push(CompletionItem {
                    label: struct_decl.name.clone(),
                    kind: Some(CompletionItemKind::STRUCT),
                    detail: Some("结构体".to_string()),
                    documentation: None,
                    deprecated: Some(false),
                    preselect: Some(false),
                    sort_text: Some(format!("struct{}", struct_decl.name)),
                    filter_text: Some(struct_decl.name.clone()),
                    insert_text: Some(struct_decl.name.clone()),
                    insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
                    insert_text_mode: None,
            label_details: None,
                    text_edit: None,
                    additional_text_edits: None,
                    command: None,
                    commit_characters: None,
                    data: None,
                    tags: None,
                });
            }
            qi_compiler::parser::AstNode::枚举声明(enum_decl) => {
                completions.push(CompletionItem {
                    label: enum_decl.name.clone(),
                    kind: Some(CompletionItemKind::ENUM),
                    detail: Some("枚举".to_string()),
                    documentation: None,
                    deprecated: Some(false),
                    preselect: Some(false),
                    sort_text: Some(format!("enum{}", enum_decl.name)),
                    filter_text: Some(enum_decl.name.clone()),
                    insert_text: Some(enum_decl.name.clone()),
                    insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
                    insert_text_mode: None,
            label_details: None,
                    text_edit: None,
                    additional_text_edits: None,
                    command: None,
                    commit_characters: None,
                    data: None,
                    tags: None,
                });
            }
            _ => {}
        }
    }

    completions
}

/// Format type annotation for display
fn format_type_annotation(type_annotation: &qi_compiler::parser::TypeNode) -> String {
    use qi_compiler::parser::TypeNode;

    match type_annotation {
        TypeNode::基础类型(basic_type) => {
            match basic_type {
                qi_compiler::parser::BasicType::整数 => "整数".to_string(),
                qi_compiler::parser::BasicType::长整数 => "长整数".to_string(),
                qi_compiler::parser::BasicType::短整数 => "短整数".to_string(),
                qi_compiler::parser::BasicType::字节 => "字节".to_string(),
                qi_compiler::parser::BasicType::浮点数 => "浮点数".to_string(),
                qi_compiler::parser::BasicType::布尔 => "布尔".to_string(),
                qi_compiler::parser::BasicType::字符 => "字符".to_string(),
                qi_compiler::parser::BasicType::字符串 => "字符串".to_string(),
                qi_compiler::parser::BasicType::空 => "空".to_string(),
                qi_compiler::parser::BasicType::数组 => "数组".to_string(),
                qi_compiler::parser::BasicType::字典 => "字典".to_string(),
                qi_compiler::parser::BasicType::列表 => "列表".to_string(),
                qi_compiler::parser::BasicType::集合 => "集合".to_string(),
                qi_compiler::parser::BasicType::指针 => "指针".to_string(),
                qi_compiler::parser::BasicType::引用 => "引用".to_string(),
                qi_compiler::parser::BasicType::可变引用 => "可变引用".to_string(),
            }
        }
        TypeNode::自定义类型(name) => name.clone(),
        TypeNode::数组类型(array_type) => {
            format!("[{}]", format_type_annotation(&array_type.element_type))
        }
        _ => format!("{:?}", type_annotation), // Fallback
    }
}

/// Filter completions based on current context
fn filter_completions_by_context(
    items: Vec<CompletionItem>,
    uri: &str,
    position: Position,
    document_manager: &DocumentManager,
) -> Vec<CompletionItem> {
    // Get current line content to determine context
    let line_content = document_manager.get_line_content(uri, position.line as usize)
        .unwrap_or_default();

    let before_cursor = &line_content[..(position.character as usize).min(line_content.len())];

    // Simple context filtering based on what was typed before cursor
    if before_cursor.ends_with('.') {
        // Field/method access context - only show appropriate items
        items
            .into_iter()
            .filter(|item| {
                matches!(
                    item.kind,
                    Some(CompletionItemKind::METHOD | CompletionItemKind::FIELD | CompletionItemKind::PROPERTY)
                )
            })
            .collect()
    } else if before_cursor.ends_with(':') {
        // Type annotation context
        items
            .into_iter()
            .filter(|item| {
                matches!(
                    item.kind,
                    Some(CompletionItemKind::TYPE_PARAMETER | CompletionItemKind::CLASS | CompletionItemKind::INTERFACE)
                )
            })
            .collect()
    } else {
        // General context - return all items
        items
    }
}

/// Create a simple completion item
fn create_simple_completion(
    label: &str,
    kind: CompletionItemKind,
    detail: Option<&str>,
) -> CompletionItem {
    CompletionItem {
        label: label.to_string(),
        kind: Some(kind),
        detail: detail.map(|s| s.to_string()),
        documentation: None,
        deprecated: Some(false),
        preselect: Some(false),
        sort_text: Some(label.to_string()),
        filter_text: Some(label.to_string()),
        insert_text: Some(label.to_string()),
        insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
        insert_text_mode: None,
            label_details: None,
        text_edit: None,
        additional_text_edits: None,
        command: None,
        commit_characters: None,
        data: None,
        tags: None,
    }
}

/// Create a snippet completion item
fn create_snippet_completion(
    label: &str,
    kind: CompletionItemKind,
    detail: Option<&str>,
    insert_text: &str,
) -> CompletionItem {
    CompletionItem {
        label: label.to_string(),
        kind: Some(kind),
        detail: detail.map(|s| s.to_string()),
        documentation: None,
        deprecated: Some(false),
        preselect: Some(false),
        sort_text: Some(label.to_string()),
        filter_text: Some(label.to_string()),
        insert_text: Some(insert_text.to_string()),
        insert_text_format: Some(InsertTextFormat::SNIPPET),
        insert_text_mode: Some(lsp_types::InsertTextMode::ADJUST_INDENTATION),
            label_details: None,
        text_edit: None,
        additional_text_edits: None,
        command: None,
        commit_characters: None,
        data: None,
        tags: None,
    }
}