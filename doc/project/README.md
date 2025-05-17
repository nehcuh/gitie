# Gitie 项目文档

## 项目概述

Gitie 是一个基于 AI 的 Git 命令行增强工具，旨在通过智能功能简化和改进 Git 工作流程。Gitie 集成了 AI 能力，帮助用户自动生成提交信息、解释复杂的 Git 命令，以及提供智能化的 Git 错误解释，让 Git 的使用更加简单高效。

## 核心功能

- **AI 辅助提交**：自动分析代码变更，生成清晰、规范的提交信息
- **Git 命令智能解释**：通过 AI 获取对 Git 命令的详细解释（默认启用）
- **错误智能处理**：当 Git 命令执行失败时，提供友好的错误解释和解决方案
- **代码结构分析**：使用 Tree-sitter 分析代码变更的结构，生成更准确的提交信息
- **无缝集成**：与现有 Git 工作流程完全兼容，零学习成本

## 文档结构

本项目文档分为以下几个部分：

- **项目文档** (`/doc/project/`)：基本项目信息和概述
- **需求文档** (`/doc/requirements/`)：详细的产品需求说明
- **设计文档** (`/doc/design/`)：技术设计方案和实现细节
- **用户故事** (`/doc/user_stories/`)：功能的用户故事和场景描述
- **开发指南** (`/doc/development/`)：面向开发者的指南和贡献说明
  - `development_guide.md`：详细的项目架构和开发指南
  - `module_reference.md`：模块参考指南
  - `quickstart.md`：开发者快速入门指南

## 快速入门

如需快速了解 Gitie：

1. 查看 [需求文档](/doc/requirements/original_requirements.md) 了解完整功能列表
2. 参考 [开发指南](/doc/development/development_guide.md) 了解如何参与开发
3. 阅读 [设计文档](/doc/design/) 了解技术实现细节

## 项目架构

Gitie 采用模块化架构设计，主要组件包括：

- **核心模块** (`core/`)：基础类型和错误定义
- **AI 模块** (`ai_module/`)：AI 交互和解释功能
- **Git 模块** (`git_module/`)：Git 命令执行与处理
- **命令行界面** (`cli_interface/`)：命令行参数解析
- **命令处理** (`command_processing/`)：处理不同命令的逻辑
- **配置管理** (`config_management/`)：加载和管理配置
- **代码分析** (`tree_sitter_analyzer/`)：使用 Tree-sitter 分析代码结构

## 发展路线

Gitie 项目正在积极开发中，核心功能集已经实现，未来将计划添加更多功能：

- 扩展对更多编程语言的代码分析支持
- AI 辅助分支管理和合并冲突解决
- 更多智能化的 Git 操作辅助
- 本地 AI 模型支持，减少对网络的依赖
- 集成到 Git 钩子系统，提供自动化工作流

## 贡献指南

Gitie 是一个开源项目，欢迎社区贡献。详细的贡献指南请参考 [开发指南](/doc/development/development_guide.md)。