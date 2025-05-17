use crate::config_management::settings::AppConfig;
use crate::core::errors::{AIError, AppError};
use reqwest::Client;
use serde_json::{json, Value};
use tracing;

/// 从提示目录列表中加载提示文件
pub fn load_prompt_file(filename: &str, prompt_dirs: &[String]) -> Result<String, AppError> {
    use std::fs;
    use std::path::Path;

    for dir in prompt_dirs {
        let path = Path::new(dir).join(filename);
        if path.exists() {
            match fs::read_to_string(&path) {
                Ok(content) => {
                    tracing::debug!("从 {} 加载提示文件", path.display());
                    return Ok(content);
                }
                Err(e) => {
                    tracing::warn!("无法读取提示文件 {}: {}", path.display(), e);
                    // 继续尝试下一个目录
                }
            }
        }
    }

    // 查找内置资源目录
    let asset_path = Path::new("assets").join(filename);
    if asset_path.exists() {
        match fs::read_to_string(&asset_path) {
            Ok(content) => {
                tracing::debug!("从内置资源加载提示文件: {}", asset_path.display());
                return Ok(content);
            }
            Err(e) => {
                tracing::warn!("无法读取内置提示文件 {}: {}", asset_path.display(), e);
            }
        }
    }

    Err(AppError::AI(AIError::PromptFileNotFound {
        filename: filename.to_string(),
        search_paths: prompt_dirs.to_vec(),
    }))
}

/// 向AI发送提示并获取响应
pub async fn send_prompt_and_get_response(
    config: &AppConfig,
    prompt: &str,
    system_message: &str,
) -> Result<String, AppError> {
    // 创建HTTP客户端
    let client = Client::new();

    // 准备请求数据
    let messages = json!([
        {
            "role": "system",
            "content": system_message
        },
        {
            "role": "user",
            "content": prompt
        }
    ]);

    let request_data = json!({
        "model": config.ai.model_name,
        "messages": messages,
        "temperature": config.ai.temperature,
    });

    tracing::debug!("发送AI请求，提示长度: {} 字符", prompt.len());

    // 发送API请求
    let response = client
        .post(&config.ai.api_url)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", config.ai.api_key))
        .json(&request_data)
        .send()
        .await
        .map_err(|e| {
            AppError::AI(AIError::RequestFailed {
                reason: format!("HTTP请求失败: {}", e),
            })
        })?;

    // 检查响应状态
    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_else(|_| "无法读取错误消息".to_string());
        return Err(AppError::AI(AIError::RequestFailed {
            reason: format!("API响应错误: {} - {}", status, error_text),
        }));
    }

    // 解析JSON响应
    let response_json: Value = response.json().await.map_err(|e| {
        AppError::AI(AIError::ResponseParsingFailed {
            reason: format!("JSON解析失败: {}", e),
        })
    })?;

    // 提取AI生成的文本
    let ai_response = response_json
        .get("choices")
        .and_then(|choices| choices.get(0))
        .and_then(|first_choice| first_choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(|content| content.as_str())
        .ok_or_else(|| {
            AppError::AI(AIError::ResponseParsingFailed {
                reason: "无法从响应中提取AI生成的内容".to_string(),
            })
        })?
        .to_string();

    tracing::debug!("收到AI响应，长度: {} 字符", ai_response.len());

    // 清理AI输出中的标签
    let cleaned_response = crate::ai_module::utils::clean_ai_output(&ai_response);
    Ok(cleaned_response)
}

/// 获取当前系统中可用的提示目录列表
pub fn get_prompt_directories(config: &AppConfig) -> Vec<String> {
    let mut prompt_dirs = Vec::new();

    // 添加配置文件中指定的提示目录
    if !config.prompts.is_empty() {
        prompt_dirs.extend(config.prompts.clone());
    }

    // 添加当前目录下的assets目录
    prompt_dirs.push("assets".to_string());

    prompt_dirs
}