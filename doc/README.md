# Gitie 项目文档中心

## 项目概述

Gitie 是一个基于 AI 的 Git 命令行增强工具，旨在通过智能功能简化和改进 Git 工作流程。项目整合了人工智能能力，帮助用户自动生成提交信息、解释复杂的 Git 命令，以及提供智能化的 Git 错误解释，让 Git 的使用更加简单高效。

## 文档结构

文档中心按以下目录组织：

### 产品文档

- **[产品需求文档 (PRD)](product/PRD.md)**: 详细的产品功能和需求描述
- **[项目路线图](product/roadmap/ROADMAP.md)**: 未来版本规划和功能开发计划
- **[用户故事](product/user_stories/)**: 功能的用户场景和价值描述，按版本组织

### 技术文档

- **[需求规格](requirements/)**: 详细的技术需求说明，按版本组织
- **[技术设计](design/)**: 系统架构和实现方案，按版本组织
- **[开发指南](development/)**: 面向开发者的指南、贡献流程和开发环境设置

### 版本与发布

- **[版本历史](releases/VERSION_HISTORY.md)**: 已发布版本的完整历史记录
- **[版本说明](releases/version_notes/)**: 各个版本的发布说明和变更记录

### 参考资料

- **[参考文档](reference/)**: 补充资料和外部链接

## 版本发展

Gitie 项目文档按版本演进组织，反映项目的发展历程：

- **v0.1 基础版本**: 初始功能集，包含 AI 辅助提交和 Git 命令解释
- **v0.2 功能优化**: AI 默认开启功能，添加 `--noai` 选项
- **v0.3 错误处理增强**: 新增 AI 驱动的 Git 错误解释功能
- **v0.4 中文本地化功能**: 中文化日志输出，错误提示本地化
- **v0.5 Tree-sitter语法分析增强**: 集成Tree-sitter进行代码语法分析，优化commit信息生成
- **v0.6 (开发中)**: 代码评审、安全审计、智能重构等高级功能

## 快速导航

### 开发者入门

- [开发环境设置](development/development_guide.md#开发工作流)
- [项目结构说明](development/development_guide.md#项目结构)
- [贡献指南](development/development_guide.md#贡献指南)
- [项目路线图](product/roadmap/ROADMAP.md)

### 功能了解

- [产品需求文档](product/PRD.md)
- [错误处理功能](requirements/v0.3/)
- [中文本地化功能](requirements/v0.4/)
- [Tree-sitter集成](requirements/v0.5/)
- [未来功能规划](product/roadmap/ROADMAP.md)

### 技术实现

- [技术设计概览](design/TechDesign.md)
- [错误处理实现](design/v0.3/)
- [Tree-sitter集成](design/v0.5/)

## 文档贡献

欢迎对文档进行完善和补充：

1. 遵循现有的版本和目录结构
2. 保持文档风格和格式一致性
3. 更新相关索引文件
4. 确保文档与代码实现同步更新

## 文档标准

- 需求文档应清晰描述功能目标和验收标准
- 设计文档应包含足够的技术细节以指导实现
- 用户故事应从用户视角描述功能价值
- 所有文档应保持最新并与实际实现同步
- 路线图应定期更新以反映项目发展方向和优先级