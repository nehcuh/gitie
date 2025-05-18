#!/bin/bash

# 清理文档目录中的备份和临时文件
# 用法: 在 gitie/doc 目录下运行 ./cleanup_backups.sh

# 安全检查：确保在正确的目录中运行
if [[ "$(basename "$(pwd)")" != "doc" ]]; then
  echo "错误: 请在 gitie/doc 目录下运行此脚本"
  exit 1
fi

# 检查是否在 gitie 项目中
if [[ ! -d "../src" || ! -d "../.git" ]]; then
  echo "警告: 似乎不在 gitie 项目目录中，请确认当前位置"
  read -p "是否继续? (y/n) " -n 1 -r
  echo
  if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    exit 1
  fi
fi

echo "开始清理文档目录中的备份和临时文件..."

# 查找并删除备份文件
find . -type f -name "*.bak" -o -name "*.new" -o -name "*~" | while read file; do
  echo "删除: $file"
  rm "$file"
done

# 查找空目录并删除
find . -type d -empty | while read dir; do
  echo "删除空目录: $dir"
  rmdir "$dir"
done

echo "清理完成"
echo "文档目录已整理完毕"