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
        // 声明
        ("包", "package", "包 主程序;"),
        ("函数", "function", "函数 ${1:名称}(${2:参数}: ${3:类型})${4:: ${5:返回类型}} {\n    ${6:返回 ${7:结果};}\n}"),
        ("异步", "async modifier", "异步"),
        ("变量", "variable", "变量 ${1:名称}: ${2:类型} = ${3:值};"),
        ("常量", "constant", "常量 ${1:名称}: ${2:类型} = ${3:值};"),
        ("导入", "import", "导入 标准库.${1:模块名};"),
        ("导出", "export", "导出"),
        ("作为", "as/alias", "作为 ${1:别名}"),
        ("模块", "module", "模块 ${1:名称};"),
        ("公开", "public", "公开"),
        ("私有", "private", "私有"),
        // 控制流
        ("如果", "if", "如果 ${1:条件} {\n    ${2:}\n} 否则 {\n    ${3:}\n}"),
        ("否则", "else", "否则 {\n    ${1:}\n}"),
        ("当", "while", "当 ${1:条件} {\n    ${2:}\n}"),
        ("对于", "for", "对于 ${1:元素} 在 ${2:集合} 中 {\n    ${3:}\n}"),
        ("在", "in", "在"),
        ("循环", "loop", "循环 {\n    ${1:}\n    如果 ${2:条件} {\n        跳出;\n    }\n}"),
        ("跳出", "break", "跳出;"),
        ("继续", "continue", "继续;"),
        ("返回", "return", "返回 ${1:值};"),
        ("匹配", "match", "匹配 ${1:值} {\n    情况 ${2:模式} => ${3:结果},\n    情况 _ => ${4:默认},\n}"),
        ("选择", "select/switch", "选择 ${1:值} {\n    情况 ${2:模式} => ${3:结果},\n    情况 _ => ${4:默认},\n}"),
        ("情况", "case", "情况 ${1:模式} => ${2:结果},"),
        // 类型定义
        ("类型", "type/struct", "类型 ${1:名称} {\n    ${2:字段}: ${3:类型};\n}"),
        ("结构体", "struct", "结构体 ${1:名称} {\n    ${2:字段}: ${3:类型};\n}"),
        ("枚举", "enum", "枚举 ${1:名称} {\n    ${2:变体},\n}"),
        ("联合体", "union", "联合体 ${1:名称} {\n    ${2:字段}: ${3:类型};\n}"),
        ("方法", "method", "方法 ${1:名称}(自己: ${2:类型})${3:: ${4:返回类型}} {\n    ${5:}\n}"),
        ("自己", "self", "自己"),
        ("新建", "new/instantiate", "新建 ${1:类型} { ${2:字段}: ${3:值} }"),
        ("闭包", "closure", "闭包(${1:参数}) {\n    ${2:}\n}"),
        ("内联", "inline", "内联"),
        // 并发
        ("启动", "spawn goroutine", "启动 ${1:函数名}();"),
        ("协程", "coroutine", "协程"),
        ("通道", "channel", "通道<${1:类型}>"),
        ("并发", "concurrent", "并发"),
        ("等待", "await", "等待 ${1:异步调用};"),
        ("等待组", "wait group", "等待组"),
        ("互斥锁", "mutex", "互斥锁"),
        ("读写锁", "rwlock", "读写锁"),
        ("条件变量", "condvar", "条件变量"),
        ("仅一次", "once", "仅一次"),
        // 错误处理
        ("尝试", "try", "尝试 {\n    ${1:}\n} 捕获 ${2:错误} {\n    ${3:}\n}"),
        ("捕获", "catch", "捕获 ${1:错误} {\n    ${2:}\n}"),
        ("抛出", "throw", "抛出 ${1:错误};"),
        ("最终", "finally", "最终 {\n    ${1:}\n}"),
        ("重试", "retry", "重试"),
        ("超时", "timeout", "超时(${1:时长})"),
        // 指针
        ("取地址", "address-of", "取地址 ${1:变量}"),
        ("解引用", "dereference", "解引用 ${1:指针}"),
        // 参数
        ("参数", "parameter", "参数"),
        ("与", "and", "与"),
        ("或", "or", "或"),
        // 内置函数
        ("打印行", "println", "打印行(${1:\"内容\"});"),
        ("打印", "print", "打印(${1:\"内容\"});"),
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
        // 基础
        ("标准库.输入输出", "I/O 输入输出"),
        ("标准库.字符串", "字符串处理"),
        ("标准库.数学", "数学运算"),
        ("标准库.日期", "日期时间"),
        ("标准库.随机", "随机数生成"),
        // 数据结构
        ("标准库.哈希表", "哈希表"),
        ("标准库.向量", "动态数组"),
        // 文件与系统
        ("标准库.文件", "文件读写（输入输出别名）"),
        ("标准库.路径", "路径处理"),
        ("标准库.操作系统", "操作系统接口"),
        ("标准库.进程", "进程管理"),
        ("标准库.环境", "环境变量"),
        ("标准库.配置", "配置文件读取"),
        // 网络
        ("标准库.网络", "TCP/UDP 网络"),
        ("标准库.HTTP", "HTTP 客户端/服务端"),
        // 并发
        ("标准库.并发", "协程与同步原语"),
        // 数据格式
        ("标准库.JSON", "JSON 序列化/反序列化"),
        ("标准库.正则", "正则表达式"),
        ("标准库.压缩", "数据压缩/解压"),
        // 安全
        ("标准库.加密", "加密与哈希"),
        // 数据库
        ("标准库.数据库", "数据库访问"),
        // AI / MCP
        ("标准库.大模型", "LLM 大语言模型接口"),
        ("标准库.MCP服务器", "MCP 协议服务器"),
        // 图形
        ("标准库.图形化", "GUI 图形界面"),
        // 命令行
        ("标准库.命令行", "命令行参数解析"),
        // 测试
        ("标准库.测试", "单元测试框架"),
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