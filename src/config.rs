use crate::errors::ConfigError;
use serde::Deserialize;
use std::{
    collections::HashMap,
    fs::{self, create_dir_all},
    io::{self, ErrorKind},
    path::PathBuf,
};
use tracing::{debug, error, info, warn};

const USER_CONFIG_DIR: &str = ".config/gitie";
const USER_CONFIG_FILE_NAME: &str = "config.toml";
const USER_COMMIT_PROMPT_FILE_NAME: &str = "commit-message-generator.md";
const USER_EXPLANATION_PROMPT_FILE_NAME: &str = "git-ai-helper.md";
const USER_GIT_MASTER_PROMPT_FILE_NAME: &str = "expert-prompt.md";
const USER_COMMIT_SYNTAX_PROMPT_FILE_NAME: &str = "commit-syntax.md";
const CONFIG_EXAMPLE_FILE_NAME: &str = "assets/config.example.toml";
const COMMIT_PROMPT_EXAMPLE_FILE_NAME: &str = "assets/commit-message-generator.md";
const EXPLANATION_PROMPT_EXAMPLE_FILE_NAME: &str = "assets/git-ai-helper.md";
const GIT_MASTER_PROMPT_EXAMPLE_FILE_NAME: &str = "assets/expert-prompt.md";
const COMMIT_SYNTAX_PROMPT_EXAMPLE_FILE_NAME: &str = "assets/commit-syntax.md";

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
    /// 初始化用户配置
    ///
    /// 此函数会检查用户配置目录中的每个配置文件是否存在:
    /// 1. 对于已经存在的配置文件，直接使用
    /// 2. 对于不存在的配置文件，从模板创建新文件
    /// 
    /// 返回值为用户配置文件路径和所有提示文件路径的映射
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

        // 日志记录现有配置文件
        let mut existing_files = Vec::new();
        let mut existing_count = 0;
        let total_files = 5; // 总文件数：配置文件 + 4个提示文件
        
        if user_config_path.exists() {
            existing_count += 1;
            existing_files.push(format!("用户配置已存在于: {:?}", user_config_path));
        }
        if user_commit_prompt_path.exists() {
            existing_count += 1;
            existing_files.push(format!("用户 commit-message-generator.md 已存在于: {:?}", user_commit_prompt_path));
        }
        if user_explanation_prompt_path.exists() {
            existing_count += 1;
            existing_files.push(format!("用户 git-ai-helper.md 已存在于: {:?}", user_explanation_prompt_path));
        }
        if user_git_master_prompt_path.exists() {
            existing_count += 1;
            existing_files.push(format!("用户 expert-prompt.md 已存在于: {:?}", user_git_master_prompt_path));
        }
        if user_commit_syntax_prompt_path.exists() {
            existing_count += 1;
            existing_files.push(format!("用户 commit-syntax.md 已存在于: {:?}", user_commit_syntax_prompt_path));
        }
        
        if existing_count > 0 {
            if existing_count == total_files {
                info!("所有 {} 个配置文件已存在，将直接使用", total_files);
            } else {
                info!("发现 {}/{} 个配置文件已存在，将补充缺失的配置", existing_count, total_files);
            }
            if !existing_files.is_empty() {
                debug!("{}", existing_files.join("\n"));
            }
        } else {
            info!("未发现任何现有配置文件，将创建全新配置");
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

        // 只在必要时初始化配置文件
        let mut files_to_initialize = Vec::new();
        
        if !user_config_path.exists() {
            files_to_initialize.push("配置文件");
        }
        if !user_commit_prompt_path.exists() {
            files_to_initialize.push("commit-message-generator.md");
        }
        if !user_explanation_prompt_path.exists() {
            files_to_initialize.push("git-ai-helper.md");
        }
        if !user_git_master_prompt_path.exists() {
            files_to_initialize.push("expert-prompt.md");
        }
        if !user_commit_syntax_prompt_path.exists() {
            files_to_initialize.push("commit-syntax.md");
        }
        
        if files_to_initialize.is_empty() {
            // 所有文件都已存在，直接返回路径
            return Ok((user_config_path, user_prompt_paths));
        } else {
            info!("以下文件不存在，正在初始化: {}", files_to_initialize.join(", "));
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
                test_dir.join("test_assets/commit-message-generator.md")
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
                test_dir.join("test_assets/git-ai-helper.md")
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
                test_dir.join("test_assets/expert-prompt.md")
            }
        } else {
            // 在正常环境中，使用标准资源路径
            PathBuf::from(
                std::env::var("GITIE_ASSETS_GIT_MASTER_PROMPT")
                    .unwrap_or_else(|_| GIT_MASTER_PROMPT_EXAMPLE_FILE_NAME.to_string()),
            )
        };
        
        // 获取 commit-syntax.md 提示文件源路径
        let assets_commit_syntax_prompt_path = if in_test {
            // 在测试环境中，使用测试资源路径
            let test_dir = std::env::current_dir().unwrap_or_default();
            // 优先使用环境变量指定的路径
            if let Ok(path) = std::env::var("GITIE_ASSETS_COMMIT_SYNTAX_PROMPT") {
                PathBuf::from(path)
            } else {
                // 否则使用当前目录下的测试资源
                test_dir.join("test_assets/commit-syntax.md")
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
                    "Commit message generator template not found at {}",
                    assets_commit_prompt_path.display()
                ),
                std::io::Error::new(ErrorKind::NotFound, "Commit message generator template file not found"),
            ));
        }

        if !assets_explanation_prompt_path.exists() {
            return Err(ConfigError::FileRead(
                format!(
                    "Git AI helper template not found at {}",
                    assets_explanation_prompt_path.display()
                ),
                io::Error::new(
                    ErrorKind::NotFound,
                    "Git AI helper template file not found",
                ),
            ));
        }

        if !assets_git_master_prompt_path.exists() {
            return Err(ConfigError::FileRead(
                format!(
                    "Expert prompt template not found at {}",
                    assets_git_master_prompt_path.display()
                ),
                io::Error::new(
                    ErrorKind::NotFound,
                    "Expert prompt template file not found",
                ),
            ));
        }
        
        if !assets_commit_syntax_prompt_path.exists() {
            return Err(ConfigError::FileRead(
                format!(
                    "Commit syntax template not found at {}",
                    assets_commit_syntax_prompt_path.display()
                ),
                io::Error::new(
                    ErrorKind::NotFound,
                    "Commit syntax template file not found",
                ),
            ));
        }

        // 仅在文件不存在时复制配置文件
        if !user_config_path.exists() {
            debug!("复制配置模板 {:?} 到 {:?}", assets_config_path, user_config_path);
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
            info!("已成功初始化配置文件: {:?}", user_config_path);
        } else {
            debug!("配置文件已存在，跳过复制: {:?}", user_config_path);
        }

        // 仅在文件不存在时复制提示文件
        if !user_commit_prompt_path.exists() {
            fs::copy(&assets_commit_prompt_path, &user_commit_prompt_path).map_err(|e| {
                ConfigError::FileWrite(
                    format!(
                        "Failed to copy source commit-message-generator.md file {} to target prompt file {}",
                        assets_commit_prompt_path.display(),
                        user_commit_prompt_path.display()
                    ),
                    e,
                )
            })?;
            info!("已初始化 commit-message-generator.md: {:?}", user_commit_prompt_path);
        }

        // 仅在文件不存在时复制解释提示文件
        if !user_explanation_prompt_path.exists() {
            fs::copy(
                &assets_explanation_prompt_path,
                &user_explanation_prompt_path,
            )
            .map_err(|e| {
                ConfigError::FileWrite(
                    format!(
                        "Failed to copy source git-ai-helper.md file {} to target prompt file {}",
                        assets_explanation_prompt_path.display(),
                        user_explanation_prompt_path.display()
                    ),
                    e,
                )
            })?;
            info!("已初始化 git-ai-helper.md: {:?}", user_explanation_prompt_path);
        }

        // 仅在文件不存在时复制 git-master 提示文件
        if !user_git_master_prompt_path.exists() {
            fs::copy(
                &assets_git_master_prompt_path,
                &user_git_master_prompt_path,
            )
            .map_err(|e| {
                ConfigError::FileWrite(
                    format!(
                        "Failed to copy source expert-prompt.md file {} to target prompt file {}",
                        assets_git_master_prompt_path.display(),
                        user_git_master_prompt_path.display()
                    ),
                    e,
                )
            })?;
            info!("已初始化 expert-prompt.md: {:?}", user_git_master_prompt_path);
        }
        
        // 仅在文件不存在时复制 commit-syntax 提示文件
        if !user_commit_syntax_prompt_path.exists() {
            fs::copy(
                &assets_commit_syntax_prompt_path,
                &user_commit_syntax_prompt_path,
            )
            .map_err(|e| {
                ConfigError::FileWrite(
                    format!(
                        "Failed to copy source commit-syntax.md file {} to target prompt file {}",
                        assets_commit_syntax_prompt_path.display(),
                        user_commit_syntax_prompt_path.display()
                    ),
                    e,
                )
            })?;
            info!("已初始化 commit-syntax.md: {:?}", user_commit_syntax_prompt_path);
        }

        Ok((user_config_path, user_prompt_paths))
    }

    pub fn load() -> Result<Self, ConfigError> {
        // 1. 初始化配置 - 仅当必要时创建新的配置文件
        let start_time = std::time::Instant::now();
        let (user_config_path, user_prompt_paths) = match Self::initialize_config() {
            Ok(result) => {
                debug!("配置初始化完成，用时 {:?}", start_time.elapsed());
                result
            },
            Err(e) => {
                error!("配置初始化失败: {}", e);
                return Err(e);
            }
        };

        // 2. 从用户目录加载配置
        info!(
            "正在从用户目录加载配置: {:?}",
            user_config_path
        );
        
        // 记录所有加载的提示文件
        debug!("将加载以下提示文件:");
        for (prompt_type, path) in &user_prompt_paths {
            debug!("  - {} 提示文件: {:?}", prompt_type, path);
        }
        
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
        info!("正在读取配置文件: {:?}", config_path);
        let start_time = std::time::Instant::now();
        let config_content = match fs::read_to_string(config_path) {
            Ok(content) => {
                debug!("配置文件读取成功，大小: {} 字节", content.len());
                content
            },
            Err(e) => {
                error!("读取配置文件失败 {:?}: {}", config_path, e);
                return Err(ConfigError::FileRead(config_path.to_string_lossy().to_string(), e));
            }
        };

        // 解析 TOML
        debug!("正在解析配置文件 TOML 格式...");
        let mut partial_config: PartialAppConfig = match toml::from_str(&config_content) {
            Ok(config) => {
                debug!("TOML 解析成功，用时 {:?}", start_time.elapsed());
                config
            },
            Err(e) => {
                error!("解析配置文件失败 {:?}: {}", config_path, e);
                return Err(ConfigError::TomlParse(config_path.to_string_lossy().to_string(), e));
            }
        };

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
            info!("配置文件中未找到 AI 配置部分，使用默认值");
            partial_config.ai = Some(PartialAIConfig::default());
        }

        // 确保 Tree-sitter 部分存在
        if partial_config.tree_sitter.is_none() {
            info!("配置文件中未找到 Tree-sitter 配置部分，使用默认值");
            partial_config.tree_sitter = Some(PartialTreeSitterConfig::default());
        }

        // 加载所有提示文件
        let mut prompts = HashMap::new();
        let prompt_start_time = std::time::Instant::now();

        for (prompt_type, prompt_path) in prompt_paths {
            if !prompt_path.exists() {
                warn!("提示文件不存在: {:?}，跳过此文件", prompt_path);
                continue;
            }
            
            debug!("正在读取提示文件: {:?}", prompt_path);
            match fs::read_to_string(prompt_path) {
                Ok(content) => {
                    if content.trim().is_empty() {
                        warn!("提示文件 {:?} 内容为空，跳过", prompt_path);
                        continue;
                    }
                    debug!("提示文件 {:?} 读取成功，大小: {} 字节", prompt_path, content.len());
                    prompts.insert(prompt_type.clone(), content);
                },
                Err(e) => {
                    warn!("读取提示文件 {:?} 失败: {}, 跳过此文件", prompt_path, e);
                    // 非致命错误，继续加载其他提示文件
                }
            }
        }
        
        debug!("读取全部提示文件完成，用时 {:?}", prompt_start_time.elapsed());

        // 验证并处理 AI 配置
        let partial_ai_config = partial_config.ai.unwrap_or_default();

        // 获取必填字段或使用默认值
        // 这里默认使用 ollama 的服务，模型使用 qwen3:32b-q8 量化模型
        let default_api_url = "http://localhost:11434/v1/chat/completions".to_string();
        let default_model = "qwen3:32b-q8_0".to_string();
        let default_temperature = 0.7;
        
        let api_url = partial_ai_config.api_url.unwrap_or_else(|| {
            debug!("未指定 API URL，使用默认值: {}", default_api_url);
            default_api_url
        });
        
        let model_name = partial_ai_config.model_name.unwrap_or_else(|| {
            debug!("未指定模型名称，使用默认值: {}", default_model);
            default_model
        });
        
        let temperature = partial_ai_config.temperature.unwrap_or_else(|| {
            debug!("未指定温度参数，使用默认值: {}", default_temperature);
            default_temperature
        });

        // 构建最终 AI 配置
        let ai_config = AIConfig {
            api_url: api_url.clone(),
            model_name: model_name.clone(),
            temperature,
            api_key: partial_ai_config.api_key.clone(),
        };
        
        info!("AI 配置信息: API URL: {}, 模型: {}, 温度: {}, API密钥: {}",
            api_url,
            model_name,
            temperature,
            if partial_ai_config.api_key.is_some() { "已设置" } else { "未设置" }
        );

        // 构建 Tree-sitter 配置
        let partial_tree_sitter_config = partial_config.tree_sitter.unwrap_or_default();
        
        let enabled = partial_tree_sitter_config.enabled.unwrap_or(false);
        let analysis_depth = partial_tree_sitter_config.analysis_depth.unwrap_or_else(default_analysis_depth);
        let cache_enabled = partial_tree_sitter_config.cache_enabled.unwrap_or(true);
        let languages = partial_tree_sitter_config.languages.unwrap_or_else(default_languages);
        
        let tree_sitter_config = TreeSitterConfig {
            enabled,
            analysis_depth: analysis_depth.clone(),
            cache_enabled,
            languages: languages.clone(),
        };
        
        debug!("Tree-sitter 配置: 启用状态: {}, 分析深度: {}, 缓存启用: {}, 支持语言数量: {}",
            enabled,
            analysis_depth,
            cache_enabled,
            languages.len()
        );
        
        if enabled {
            debug!("Tree-sitter 支持的语言: {}", languages.join(", "));
        }

        // 检查是否成功加载了所有必要的提示文件
        if prompts.is_empty() {
            warn!("未能加载任何提示文件，配置可能不完整");
            // 继续运行，但功能可能受限
        } else if prompts.len() < prompt_paths.len() {
            warn!("只加载了部分提示文件 ({}/{})", prompts.len(), prompt_paths.len());
            debug!("已加载的提示文件类型: {}", prompts.keys().map(|k| k.as_str()).collect::<Vec<_>>().join(", "));
        } else {
            info!("成功加载全部 {} 个提示文件", prompts.len());
        }

        let config = Self {
            ai: ai_config,
            tree_sitter: tree_sitter_config,
            prompts,
        };
        
        info!("配置加载完成，Gitie 准备就绪");
        Ok(config)
    }
}
