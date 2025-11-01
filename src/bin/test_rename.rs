//! Test rename functionality

use qi_lsp::rename;
use qi_lsp::document::DocumentManager;
use lsp_types::{Position};

#[tokio::main]
async fn main() {
    // Create test content
    let test_content = r#"
包 测试包;

函数 旧函数名(参数1: 整数, 参数2: 字符串) : 整数 {
    变量 旧变量名 = 参数1 + 10;
    打印行("旧变量名:", 旧变量名);
    打印行("调用旧函数名:", 旧函数名(5, "测试"));

    返回 旧变量名;
}

函数 主函数() {
    变量 结果 = 旧函数名(42, "hello");
    打印行("结果:", 结果);
    返回 0;
}
"#;

    println!("🧪 测试重命名功能...\n");

    // Create document manager
    let document_manager = DocumentManager::new();
    let uri = "file:///test_rename.qi";

    // Open document
    document_manager.open_document(uri, test_content.to_string());

    // Test 1: Rename function "旧函数名" to "新函数名"
    println!("📝 测试 1: 重命名函数 '旧函数名' → '新函数名'");

    let position = Position {
        line: 4,  // 在 "旧函数名" 所在行
        character: 8,  // 在函数名位置
    };
    let new_name = "新函数名";

    if let Some(workspace_edit) = rename::perform_rename(
        uri,
        position,
        new_name,
        &document_manager,
    ) {
        if let Some(changes) = workspace_edit.changes {
            println!("✅ 找到 {} 个文件需要修改", changes.len());
            for (doc_uri, edits) in changes {
                println!("📄 文件: {:?}", doc_uri);
                for (i, edit) in edits.iter().enumerate() {
                    println!("  {}. 行 {}-{}: '{}' → '{}'",
                        i + 1,
                        edit.range.start.line + 1,
                        edit.range.end.line + 1,
                        get_text_in_range(test_content, &edit.range),
                        edit.new_text
                    );
                }
            }
        } else {
            println!("ℹ️ 没有找到需要修改的引用");
        }
    } else {
        println!("❌ 重命名失败");
    }

    println!("\n{}", "=".repeat(50));

    // Test 2: Rename variable "旧变量名" to "新变量名"
    println!("📝 测试 2: 重命名变量 '旧变量名' → '新变量名'");

    let position2 = Position {
        line: 5,  // 在 "旧变量名" 所在行
        character: 11,  // 在变量名位置
    };
    let new_name2 = "新变量名";

    if let Some(workspace_edit) = rename::perform_rename(
        uri,
        position2,
        new_name2,
        &document_manager,
    ) {
        if let Some(changes) = workspace_edit.changes {
            println!("✅ 找到 {} 个文件需要修改", changes.len());
            for (doc_uri, edits) in changes {
                println!("📄 文件: {:?}", doc_uri);
                for (i, edit) in edits.iter().enumerate() {
                    println!("  {}. 行 {}-{}: '{}' → '{}'",
                        i + 1,
                        edit.range.start.line + 1,
                        edit.range.end.line + 1,
                        get_text_in_range(test_content, &edit.range),
                        edit.new_text
                    );
                }
            }
        } else {
            println!("ℹ️ 没有找到需要修改的引用");
        }
    } else {
        println!("❌ 重命名失败");
    }

    println!("\n{}", "=".repeat(50));

    // Test 3: Invalid rename (same name)
    println!("📝 测试 3: 无效重命名 (相同名称)");

    let position3 = Position {
        line: 4,
        character: 8,
    };
    let new_name3 = "旧函数名"; // 相同名称

    if let Some(workspace_edit) = rename::perform_rename(
        uri,
        position3,
        new_name3,
        &document_manager,
    ) {
        if let Some(changes) = workspace_edit.changes {
            if changes.is_empty() {
                println!("✅ 正确处理相同名称：没有修改");
            } else {
                println!("⚠️ 意外修改：{} 个变更", changes.len());
            }
        }
    } else {
        println!("❌ 重命名失败");
    }

    println!("\n{}", "=".repeat(50));

    // Test 4: Invalid identifier (keyword)
    println!("📝 测试 4: 无效重命名 (关键字)");

    let position4 = Position {
        line: 4,
        character: 8,
    };
    let new_name4 = "函数"; // 关键字

    if let Some(workspace_edit) = rename::perform_rename(
        uri,
        position4,
        new_name4,
        &document_manager,
    ) {
        println!("❌ 应该拒绝关键字重命名");
    } else {
        println!("✅ 正确拒绝关键字重命名");
    }

    // Close document
    document_manager.close_document(uri);
    println!("\n🎉 重命名功能测试完成！");
}

/// Helper function to get text in a range
fn get_text_in_range(content: &str, range: &lsp_types::Range) -> String {
    let lines: Vec<&str> = content.lines().collect();

    if range.start.line == range.end.line {
        // Single line
        if let Some(line) = lines.get(range.start.line as usize) {
            let start_char = range.start.character as usize;
            let end_char = range.end.character as usize;
            if start_char <= line.len() && end_char <= line.len() {
                return line[start_char..end_char].to_string();
            }
        }
    } else {
        // Multi-line (simplified)
        if let Some(start_line) = lines.get(range.start.line as usize) {
            let start_char = range.start.character as usize;
            if start_char <= start_line.len() {
                return start_line[start_char..].to_string() + " ...";
            }
        }
    }

    "[无法提取文本]".to_string()
}