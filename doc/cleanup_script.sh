#!/bin/bash

# Gitie 文档清理脚本
# 此脚本用于清理旧的文档结构，确保所有文档都已被正确迁移到新的版本化目录中

# 设置颜色
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
NC='\033[0m' # 无颜色

echo -e "${YELLOW}Gitie 文档清理工具${NC}"
echo "此脚本将清理旧的文档结构，确保文档已正确迁移到版本化目录"
echo ""

# 检查是否在 gitie/doc 目录下执行
if [[ "$(basename "$PWD")" != "doc" || "$(basename "$(dirname "$PWD")")" != "gitie" ]]; then
    echo -e "${RED}错误: 此脚本必须在 gitie/doc 目录下执行${NC}"
    exit 1
fi

# 确认提示
echo -e "${YELLOW}警告: 此操作将删除已迁移到版本目录的旧文档，请确保所有文档已正确迁移${NC}"
read -p "是否继续? (y/n): " confirm
if [[ "$confirm" != "y" && "$confirm" != "Y" ]]; then
    echo "操作已取消"
    exit 0
fi

echo ""
echo "开始文档清理..."

# 1. 删除已迁移的旧PRD文件
old_prd_files=(
    "Error_Handling_PRD.md"
    "requirements/original_requirements.md"
    "requirements/error_handling_requirements.md"
    "requirements/ai_options_optimization_requirements.md"
    "requirements/devops_integration_requirements.md"
)

for file in "${old_prd_files[@]}"; do
    if [[ -f "$file" ]]; then
        echo -e "删除旧PRD文件: ${YELLOW}$file${NC}"
        rm "$file"
    fi
done

# 2. 删除已迁移的旧设计文档
old_design_files=(
    "Error_Handling_Implementation_Plan.md"
    "Error_Handling_Implementation_Summary.md"
    "design/error_handling_implementation_plan.md"
    "design/error_handling_implementation_summary.md"
)

for file in "${old_design_files[@]}"; do
    if [[ -f "$file" ]]; then
        echo -e "删除旧设计文档: ${YELLOW}$file${NC}"
        rm "$file"
    fi
done

# 3. 删除已迁移的旧用户故事
old_story_files=(
    "Error_Handling_Technical_Stories.md"
    "user_stories/ai_command_explanation.md"
    "user_stories/ai_commit.md"
    "user_stories/commit_auto_stage.md"
    "user_stories/devops_integration.md"
    "user_stories/optimize_ai_default.md"
    "user_stories/error_handling_technical_stories.md"
)

for file in "${old_story_files[@]}"; do
    if [[ -f "$file" ]]; then
        echo -e "删除旧用户故事: ${YELLOW}$file${NC}"
        rm "$file"
    fi
done

# 4. 移动旧的DEVELOPMENT.md
if [[ -f "DEVELOPMENT.md" && -f "development/development_guide.md" ]]; then
    echo -e "删除旧开发指南: ${YELLOW}DEVELOPMENT.md${NC}"
    rm "DEVELOPMENT.md"
fi

# 5. 检查并删除旧的空目录
old_dirs=("prds" "stories")
for dir in "${old_dirs[@]}"; do
    if [[ -d "$dir" ]]; then
        if [[ -z "$(ls -A "$dir")" ]]; then
            echo -e "删除空目录: ${YELLOW}$dir${NC}"
            rmdir "$dir"
        else
            echo -e "${RED}警告: 目录 $dir 不为空，请手动检查${NC}"
            ls -la "$dir"
        fi
    fi
done

echo ""
echo -e "${GREEN}文档清理完成!${NC}"
echo "新的文档结构已整理为按版本组织的格式"
echo "请检查确认所有文档都已正确迁移"