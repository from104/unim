---
name: git-sync
description: Skill for summarizing changes in English, committing, and syncing with GitHub.
---

# Git Sync Skill

This skill provides guidelines for summarizing core changes in English, committing them, and synchronizing with the remote GitHub repository.

## 1. Analysis Process

Before committing, the AI must:

1. Check the current branch: `git branch --show-current`
2. Review changed files: `git status`
3. Analyze detailed changes: `git diff --cached` (after adding) or `git diff`

## 2. Commit Message Guidelines

Commit messages MUST be written in **English** and follow the **Conventional Commits** format.

### Template

`[type]: [Subject in lowercase]`

**Types:**

- `feat`: New feature
- `fix`: Bug fix
- `refactor`: Code change that neither fixes a bug nor adds a feature
- `docs`: Documentation only changes
- `chore`: Update build tasks, package manager configs, etc.
- `perf`: Code change that improves performance
- `test`: Adding missing tests or correcting existing tests

### Summarization Principles

- Focus on **why** and **what** was changed.
- Be concise but descriptive.
- Summarize multiple related changes into a single logical point if possible.

## 3. Sync Procedure

When the user commands to sync:

1. **Stage changes**: `git add -A`
2. **Generate message**: Analyze staged changes and generate a summary in English.
3. **Commit**: `git commit -m "[Generated Message]"`
4. **Push**: `git push origin [current_branch]`

## 4. Usage Example

- **User**: "Sync the changes."
- **Assistant**: [Analyzes diff] -> "Generated commit message: `feat: add per-window mode setting`" -> [Executes add, commit, push]
