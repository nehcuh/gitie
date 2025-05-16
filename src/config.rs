use crate::errors::ConfigError;
use serde::Deserialize;
use std::{
    collections::HashMap,
    fs::{self, create_dir_all},
    io::{self, ErrorKind},
    path::PathBuf,
};
use tracing::info;

const USER_CONFIG_DIR: &str = ".config/gitie";
const USER_CONFIG_FILE_NAME: &str = "config.toml";
const USER_COMMIT_PROMPT_FILE_NAME: &str = "commit-prompt";
const USER_EXPLANATION_PROMPT_FILE_NAME: &str = "commit-prompt";
const USER_GIT_MASTER_PROMPT_FILE_NAME: &str = "git-master-prompt";
const USER_COMMIT_SYNTAX_PROMPT_FILE_NAME: &str = "commit-syntax-prompt";
const CONFIG_EXAMPLE_FILE_NAME: &str = "assets/config.example.toml";
const COMMIT_PROMPT_EXAMPLE_FILE_NAME: &str = "assets/commit-prompt";
const EXPLANATION_PROMPT_EXAMPLE_FILE_NAME: &str = "assets/explanation-prompt";
const GIT_MASTER_PROMPT_EXAMPLE_FILE_NAME: &str = "assets/git-master-prompt";
const COMMIT_SYNTAX_PROMPT_EXAMPLE_FILE_NAME: &str = "assets/commit-syntax-prompt";

// AI 服务配置
#[derive(Deserialize, Debug, Clone, Default)]
pub struct AIConfig {
    pub api_url: String,
    pub model_name: String,
    pub temperature: f32,
    pub api_key: Option<String>,
}

// Tree-sitter 配置
#[derive(Deserialize, Debug, Clone)]
pub struct TreeSitterConfig {
    /// 是否启用语法树分析
    #[serde(default)]
    pub enabled: bool,
    
    /// 分析深度: "shallow", "medium", "deep"
    #[serde(default = "default_analysis_depth")]
    pub analysis_depth: String,
    
    /// 是否启用缓存
    #[serde(default = "default_cache_enabled")]
    pub cache_enabled: bool,
    
    /// 支持的语言列表
    #[serde(default = "default_languages")]
    pub languages: Vec<String>,
}

impl Default for TreeSitterConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            analysis_depth: default_analysis_depth(),
            cache_enabled: default_cache_enabled(),
            languages: default_languages(),
        }
    }
}

fn default_analysis_depth() -> String {
    "medium".to_string()
}

fn default_cache_enabled() -> bool {
    true
}

fn default_languages() -> Vec<String> {
    vec!["rust".to_string(), "javascript".to_string(), "python".to_string()]
}

// AI 配置的部分加载辅助结构体
#[derive(Deserialize, Debug, Default, Clone)]
struct PartialAIConfig {
    #[serde(default)]
    api_url: Option<String>,
    #[serde(default)]
    model_name: Option<String>,
    #[serde(default)]
    temperature: Option<f32>,
    #[serde(default)]
    api_key: Option<String>,
}

// Tree-sitter 配置的部分加载辅助结构体
#[derive(Deserialize, Debug, Default, Clone)]
struct PartialTreeSitterConfig {
    #[serde(default)]
    enabled: Option<bool>,
    #[serde(default)]
    analysis_depth: Option<String>,
    #[serde(default)]
    cache_enabled: Option<bool>,
    #[serde(default)]
    languages: Option<Vec<String>>,
}

// 应用总体配置
#[derive(Deserialize, Debug, Clone)]
pub struct AppConfig {
    #[serde(default)]
    pub ai: AIConfig,

    #[serde(default)]
    pub tree_sitter: TreeSitterConfig,

    #[serde(skip)] // System prompt is loaded separately
    pub prompts: HashMap<String, String>,
}

// 部分加载的配置辅助结构体
#[derive(Deserialize, Debug, Default)]
struct PartialAppConfig {
    ai: Option<PartialAIConfig>,
    tree_sitter: Option<PartialTreeSitterConfig>,
}

impl AppConfig {
    /// 初始化用户配置
    ///
    /// 此函数会检查用户配置目录是否存在配置文件，如果不存在，
    /// 则从assets目录复制默认配置文件
    pub fn initialize_config() -> Result<(PathBuf, HashMap<String, PathBuf>), ConfigError> {
        let user_config_path = Self::get_user_file_path(USER_CONFIG_FILE_NAME)?;
        let user_commit_prompt_path = Self::get_user_file_path(USER_COMMIT_PROMPT_FILE_NAME)?;
        let user_explanation_prompt_path =
            Self::get_user_file_path(USER_EXPLANATION_PROMPT_FILE_NAME)?;
        let user_git_master_prompt_path = 
            Self::get_user_file_path(USER_GIT_MASTER_PROMPT_FILE_NAME)?;
        let user_commit_syntax_prompt_path = 
            Self::get_user_file_path(USER_COMMIT_SYNTAX_PROMPT_FILE_NAME)?;

        let mut user_prompt_paths = HashMap::new();
        user_prompt_paths.insert("commit".to_string(), user_commit_prompt_path.clone());
        user_prompt_paths.insert(
            "explanation".to_string(),
            user_explanation_prompt_path.clone(),
        );
        user_prompt_paths.insert(
            "git-master".to_string(),
            user_git_master_prompt_path.clone(),
        );
        user_prompt_paths.insert(
            "commit-syntax".to_string(),
            user_commit_syntax_prompt_path.clone(),
        );

        // 如果用户配置已存在，则直接返回路径
        if user_config_path.exists()
            && user_commit_prompt_path.exists()
            && user_explanation_prompt_path.exists()
            && user_git_master_prompt_path.exists()
            && user_commit_syntax_prompt_path.exists()
        {
            info!(
                "用户配置已存在于: {:?}\n 用户 commit-prompt 已存在于: {:?}\n 用户 explanation-prompt 已存在于: {:?}\n 用户 git-master-prompt 已存在于: {:?}\n 用户 commit-syntax-prompt 已存在于: {:?}",
                user_config_path, user_commit_prompt_path, user_explanation_prompt_path, user_git_master_prompt_path, user_commit_syntax_prompt_path
            );
            return Ok((user_config_path, user_prompt_paths));
        }

        // 获取配置目录
        let user_config_dir = match user_config_path.parent() {
            Some(dir) => dir.to_path_buf(),
            None => {
                return Err(ConfigError::FileWrite(
                    user_config_path.to_string_lossy().to_string(),
                    std::io::Error::new(ErrorKind::Other, "Invalid user config path"),
                ));
            }
        };

        // 确保配置目录存在
        create_dir_all(&user_config_dir).map_err(|e| {
            ConfigError::FileWrite(user_config_dir.to_string_lossy().to_string(), e)
        })?;

        // 初始化配置文件
        if !user_config_path.exists() {
            info!("用户配置文件不存在。正在初始化...");
        }

        // 检查我们是否在测试环境中
        let in_test = std::env::current_dir()
            .map(|p| p.to_string_lossy().contains("target/test_temp_data"))
            .unwrap_or(false);

        // 获取配置文件源路径
        let assets_config_path = if in_test {
            // 在测试环境中，使用测试资源路径
            let test_dir = std::env::current_dir().unwrap_or_default();
            // 优先使用环境变量指定的路径
            if let Ok(path) = std::env::var("GITIE_ASSETS_CONFIG") {
                PathBuf::from(path)
            } else {
                // 否则使用当前目录下的测试资源
                test_dir.join(CONFIG_EXAMPLE_FILE_NAME)
            }
        } else {
            // 在正常环境中，使用标准资源路径
            PathBuf::from(
                std::env::var("GITIE_ASSETS_CONFIG")
                    .unwrap_or_else(|_| CONFIG_EXAMPLE_FILE_NAME.to_string()),
            )
        };

        // 获取提示文件源路径
        let assets_commit_prompt_path = if in_test {
            // 在测试环境中，使用测试资源路径
            let test_dir = std::env::current_dir().unwrap_or_default();
            // 优先使用环境变量指定的路径
            if let Ok(path) = std::env::var("GITIE_ASSETS_PROMPT") {
                PathBuf::from(path)
            } else {
                // 否则使用当前目录下的测试资源
                test_dir.join("test_assets/commit-prompt")
            }
        } else {
            // 在正常环境中，使用标准资源路径
            PathBuf::from(
                std::env::var("GITIE_ASSETS_COMMIT_PROMPT")
                    .unwrap_or_else(|_| COMMIT_PROMPT_EXAMPLE_FILE_NAME.to_string()),
            )
        };

        // 获取解释提示文件源路径
        let assets_explanation_prompt_path = if in_test {
            // 在测试环境中，使用测试资源路径
            let test_dir = std::env::current_dir().unwrap_or_default();
            // 优先使用环境变量指定的路径
            if let Ok(path) = std::env::var("GITIE_ASSETS_EXPLANATION_PROMPT") {
                PathBuf::from(path)
            } else {
                // 否则使用当前目录下的测试资源
                test_dir.join("test_assets/explanation-prompt")
            }
        } else {
            // 在正常环境中，使用标准资源路径
            PathBuf::from(
                std::env::var("GITIE_ASSETS_EXPLANATION_PROMPT")
                    .unwrap_or_else(|_| EXPLANATION_PROMPT_EXAMPLE_FILE_NAME.to_string()),
            )
        };
        
        // 获取 git-master-prompt 提示文件源路径
        let assets_git_master_prompt_path = if in_test {
            // 在测试环境中，使用测试资源路径
            let test_dir = std::env::current_dir().unwrap_or_default();
            // 优先使用环境变量指定的路径
            if let Ok(path) = std::env::var("GITIE_ASSETS_GIT_MASTER_PROMPT") {
                PathBuf::from(path)
            } else {
                // 否则使用当前目录下的测试资源
                test_dir.join("test_assets/git-master-prompt")
            }
        } else {
            // 在正常环境中，使用标准资源路径
            PathBuf::from(
                std::env::var("GITIE_ASSETS_GIT_MASTER_PROMPT")
                    .unwrap_or_else(|_| GIT_MASTER_PROMPT_EXAMPLE_FILE_NAME.to_string()),
            )
        };
        
        // 获取 commit-syntax-prompt 提示文件源路径
        let assets_commit_syntax_prompt_path = if in_test {
            // 在测试环境中，使用测试资源路径
            let test_dir = std::env::current_dir().unwrap_or_default();
            // 优先使用环境变量指定的路径
            if let Ok(path) = std::env::var("GITIE_ASSETS_COMMIT_SYNTAX_PROMPT") {
                PathBuf::from(path)
            } else {
                // 否则使用当前目录下的测试资源
                test_dir.join("test_assets/commit-syntax-prompt")
            }
        } else {
            // 在正常环境中，使用标准资源路径
            PathBuf::from(
                std::env::var("GITIE_ASSETS_COMMIT_SYNTAX_PROMPT")
                    .unwrap_or_else(|_| COMMIT_SYNTAX_PROMPT_EXAMPLE_FILE_NAME.to_string()),
            )
        };

        // 检查源文件是否存在
        if !assets_config_path.exists() {
            return Err(ConfigError::FileRead(
                format!(
                    "Config template not found at {}",
                    assets_config_path.display()
                ),
                std::io::Error::new(ErrorKind::NotFound, "Config template file not found"),
            ));
        }

        if !assets_commit_prompt_path.exists() {
            return Err(ConfigError::FileRead(
                format!(
                    "Commit prompt template not found at {}",
                    assets_commit_prompt_path.display()
                ),
                std::io::Error::new(ErrorKind::NotFound, "Prompt template file not found"),
            ));
        }

        if !assets_explanation_prompt_path.exists() {
            return Err(ConfigError::FileRead(
                format!(
                    "Explanation prompt template not found at {}",
                    assets_explanation_prompt_path.display()
                ),
                io::Error::new(
                    ErrorKind::NotFound,
                    "Explanation prompt template file not found",
                ),
            ));
        }

        if !assets_git_master_prompt_path.exists() {
            return Err(ConfigError::FileRead(
                format!(
                    "Git master prompt template not found at {}",
                    assets_git_master_prompt_path.display()
                ),
                io::Error::new(
                    ErrorKind::NotFound,
                    "Git master prompt template file not found",
                ),
            ));
        }
        
        if !assets_commit_syntax_prompt_path.exists() {
            return Err(ConfigError::FileRead(
                format!(
                    "Commit syntax prompt template not found at {}",
                    assets_commit_syntax_prompt_path.display()
                ),
                io::Error::new(
                    ErrorKind::NotFound,
                    "Commit syntax prompt template file not found",
                ),
            ));
        }

        // 复制配置文件
        fs::copy(&assets_config_path, &user_config_path).map_err(|e| {
            ConfigError::FileWrite(
                format!(
                    "Failed to copy source config file {} to target config file {}",
                    assets_config_path.display(),
                    user_config_path.display()
                ),
                e,
            )
        })?;

        // 复制提示文件
        fs::copy(&assets_commit_prompt_path, &user_commit_prompt_path).map_err(|e| {
            ConfigError::FileWrite(
                format!(
                    "Failed to copy source commit prompt file {} to target prompt file {}",
                    assets_commit_prompt_path.display(),
                    user_commit_prompt_path.display()
                ),
                e,
            )
        })?;

        // 复制解释提示文件
        fs::copy(
            &assets_explanation_prompt_path,
            &user_explanation_prompt_path,
        )
        .map_err(|e| {
            ConfigError::FileWrite(
                format!(
                    "Failed to copy source explanation prompt file {} to target prompt file {}",
                    assets_explanation_prompt_path.display(),
                    user_explanation_prompt_path.display()
                ),
                e,
            )
        })?;

        // 复制 git-master 提示文件
        fs::copy(
            &assets_git_master_prompt_path,
            &user_git_master_prompt_path,
        )
        .map_err(|e| {
            ConfigError::FileWrite(
                format!(
                    "Failed to copy source git-master prompt file {} to target prompt file {}",
                    assets_git_master_prompt_path.display(),
                    user_git_master_prompt_path.display()
                ),
                e,
            )
        })?;
        
        // 复制 commit-syntax 提示文件
        fs::copy(
            &assets_commit_syntax_prompt_path,
            &user_commit_syntax_prompt_path,
        )
        .map_err(|e| {
            ConfigError::FileWrite(
                format!(
                    "Failed to copy source commit-syntax prompt file {} to target prompt file {}",
                    assets_commit_syntax_prompt_path.display(),
                    user_commit_syntax_prompt_path.display()
                ),
                e,
            )
        })?;

        Ok((user_config_path, user_prompt_paths))
    }

    pub fn load() -> Result<Self, ConfigError> {
        // 1. 初始化配置
        let (user_config_path, user_prompt_paths) = Self::initialize_config()?;

        // 2. 从用户目录加载配置
        info!(
            "正在从用户目录加载配置: {:?}",
            user_config_path
        );
        Self::load_config_from_file(&user_config_path, &user_prompt_paths)
    }

    // 获取用户目录中指定文件路径
    fn get_user_file_path(filename: &str) -> Result<std::path::PathBuf, ConfigError> {
        let home_str = std::env::var("HOME").unwrap_or_else(|_| {
            dirs::home_dir()
                .expect("Could not determine home directory")
                .to_string_lossy()
                .to_string()
        });

        let home = PathBuf::from(home_str);
        Ok(home.join(USER_CONFIG_DIR).join(filename))
    }

    fn load_config_from_file(
        config_path: &std::path::Path,
        prompt_paths: &HashMap<String, PathBuf>,
    ) -> Result<Self, ConfigError> {
        // 读取配置文件
        let config_content = fs::read_to_string(config_path)
            .map_err(|e| ConfigError::FileRead(config_path.to_string_lossy().to_string(), e))?;

        // 解析 TOML
        let mut partial_config: PartialAppConfig = toml::from_str(&config_content)
            .map_err(|e| ConfigError::TomlParse(config_path.to_string_lossy().to_string(), e))?;

        // 处理 API 密钥占位符
        if let Some(ai) = &mut partial_config.ai {
            if let Some(api_key) = &ai.api_key {
                if api_key == "YOUR_API_KEY_IF_NEEDED" || api_key.is_empty() {
                    ai.api_key = None;
                    tracing::info!(
                        "发现 API 密钥占位符或空字符串。视为无 API 密钥。"
                    );
                }
            }
        }

        // 确保 AI 部分存在
        if partial_config.ai.is_none() {
            partial_config.ai = Some(PartialAIConfig::default());
        }

        // 确保 Tree-sitter 部分存在
        if partial_config.tree_sitter.is_none() {
            partial_config.tree_sitter = Some(PartialTreeSitterConfig::default());
        }

        // 加载所有提示文件
        let mut prompts = HashMap::new();

        for (prompt_type, prompt_path) in prompt_paths {
            let prompt_content = fs::read_to_string(prompt_path)
                .map_err(|e| ConfigError::FileRead(prompt_path.to_string_lossy().to_string(), e))?;
            prompts.insert(prompt_type.clone(), prompt_content);
        }

        // 验证并处理 AI 配置
        let partial_ai_config = partial_config.ai.unwrap_or_default();

        // 获取必填字段或使用默认值
        // 这里默认使用 ollama 的服务，模型使用 qwen3:32b-q8 量化模型
        let api_url = partial_ai_config
            .api_url
            .unwrap_or("http://localhost:11434/v1/chat/completions".to_string());
        let model_name = partial_ai_config
            .model_name
            .unwrap_or("qwen3:32b-q8_0".to_string());
        let temperature = partial_ai_config.temperature.unwrap_or(0.7);

        // 构建最终 AI 配置
        let ai_config = AIConfig {
            api_url,
            model_name,
            temperature,
            api_key: partial_ai_config.api_key,
        };

        // 构建 Tree-sitter 配置
        let partial_tree_sitter_config = partial_config.tree_sitter.unwrap_or_default();
        let tree_sitter_config = TreeSitterConfig {
            enabled: partial_tree_sitter_config.enabled.unwrap_or(false),
            analysis_depth: partial_tree_sitter_config.analysis_depth.unwrap_or_else(default_analysis_depth),
            cache_enabled: partial_tree_sitter_config.cache_enabled.unwrap_or(true),
            languages: partial_tree_sitter_config.languages.unwrap_or_else(default_languages),
        };

        Ok(Self {
            ai: ai_config,
            tree_sitter: tree_sitter_config,
            prompts,
        })
    }
}
