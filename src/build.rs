//! Build functionality for Qi Language Server
//!
//! Provides compilation and build services through LSP

use anyhow::Result;
use log::{debug, info, error};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use qi_compiler::QiCompiler;

/// Build request parameters
#[derive(Debug, Deserialize)]
pub struct BuildParams {
    /// URI of the file to build
    pub uri: String,
    /// Build mode: "debug" or "release"
    #[serde(default = "default_build_mode")]
    pub mode: String,
}

fn default_build_mode() -> String {
    "debug".to_string()
}

/// Build result
#[derive(Debug, Serialize)]
pub struct BuildResult {
    /// Whether the build succeeded
    pub success: bool,
    /// Executable path if successful
    pub executable_path: Option<String>,
    /// Build duration in milliseconds
    pub duration_ms: u64,
    /// Compilation warnings
    pub warnings: Vec<String>,
    /// Error message if failed
    pub error: Option<String>,
}

/// Execute a build task
pub fn build_file(uri: &str, mode: &str) -> Result<BuildResult> {
    info!("Building file: {} (mode: {})", uri, mode);

    // Convert URI to file path
    let file_path = uri_to_path(uri)?;

    debug!("File path: {:?}", file_path);

    // Check if file exists
    if !file_path.exists() {
        return Ok(BuildResult {
            success: false,
            executable_path: None,
            duration_ms: 0,
            warnings: Vec::new(),
            error: Some(format!("文件不存在: {:?}", file_path)),
        });
    }

    // Create compiler
    // Note: Configuration is stored separately, optimization level can be set in config file
    let compiler = QiCompiler::new();

    // Compile the file
    let start_time = std::time::Instant::now();
    match compiler.compile(file_path) {
        Ok(result) => {
            let duration = start_time.elapsed().as_millis() as u64;

            info!("Build successful: {:?}", result.executable_path);

            Ok(BuildResult {
                success: true,
                executable_path: Some(result.executable_path.to_string_lossy().to_string()),
                duration_ms: duration,
                warnings: result.warnings,
                error: None,
            })
        }
        Err(e) => {
            let duration = start_time.elapsed().as_millis() as u64;

            error!("Build failed: {}", e);

            Ok(BuildResult {
                success: false,
                executable_path: None,
                duration_ms: duration,
                warnings: Vec::new(),
                error: Some(format!("{}", e)),
            })
        }
    }
}

/// Convert URI to file path
fn uri_to_path(uri: &str) -> Result<PathBuf> {
    // Remove file:// prefix
    let path_str = uri.strip_prefix("file://")
        .ok_or_else(|| anyhow::anyhow!("Invalid URI format"))?;

    #[cfg(target_os = "windows")]
    {
        // Windows: file:///C:/path -> C:\path
        let path_str = path_str.strip_prefix('/').unwrap_or(path_str);
        let path_str = path_str.replace('/', "\\");
        Ok(PathBuf::from(path_str))
    }

    #[cfg(not(target_os = "windows"))]
    {
        // Unix: file:///path -> /path
        Ok(PathBuf::from(path_str))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uri_to_path_unix() {
        #[cfg(not(target_os = "windows"))]
        {
            let uri = "file:///home/user/project/main.qi";
            let path = uri_to_path(uri).unwrap();
            assert_eq!(path, PathBuf::from("/home/user/project/main.qi"));
        }
    }

    #[test]
    fn test_uri_to_path_windows() {
        #[cfg(target_os = "windows")]
        {
            let uri = "file:///C:/Users/user/project/main.qi";
            let path = uri_to_path(uri).unwrap();
            assert_eq!(path, PathBuf::from("C:\\Users\\user\\project\\main.qi"));
        }
    }
}
