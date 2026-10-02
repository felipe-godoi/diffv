use std::path::{Path, PathBuf};
use std::process::Command;
use anyhow::{Context, Result};

use crate::core::aligner::align_hunks_side_by_side;
use crate::core::models::{
    ChangeStats, CommitEntry, DiffKind, DiffLine, FileDiff, FileStatus, Hunk, RepoStats, StageStatus,
    StashEntry, WorktreeEntry,
};
use crate::git::patch::parse_unified_diff;

pub struct GitProvider {
    pub repo_root: PathBuf,
}

impl GitProvider {
    pub fn discover(start_dir: Option<&Path>) -> Result<Self> {
        let mut cmd = Command::new("git");
        cmd.args(["rev-parse", "--show-toplevel"]);
        if let Some(dir) = start_dir {
            cmd.current_dir(dir);
        }

        let output = cmd.output().context("Failed to execute git")?;
        if !output.status.success() {
            anyhow::bail!("Not a git repository");
        }

        let root_str = String::from_utf8(output.stdout)?.trim().to_string();
        Ok(Self {
            repo_root: PathBuf::from(root_str),
        })
    }

    pub fn get_branch(&self) -> String {
        let output = Command::new("git")
            .args(["branch", "--show-current"])
            .current_dir(&self.repo_root)
            .output();

        if let Ok(out) = output {
            let branch = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !branch.is_empty() {
                return branch;
            }
        }

        // Fallback for detached HEAD
        let output_ref = Command::new("git")
            .args(["rev-parse", "--short", "HEAD"])
            .current_dir(&self.repo_root)
            .output();

        if let Ok(out) = output_ref {
            let rev = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !rev.is_empty() {
                return format!("detached: {}", rev);
            }
        }

        "initial".to_string()
    }

    pub fn get_repo_name(&self) -> String {
        self.repo_root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("repo")
            .to_string()
    }

    pub fn has_head(&self) -> bool {
        Command::new("git")
            .args(["rev-parse", "--verify", "HEAD"])
            .current_dir(&self.repo_root)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    pub fn load_diffs(
        &self,
        target_ref: Option<&str>,
        staged_only: bool,
        include_untracked: bool,
        ignore_whitespace: bool,
    ) -> Result<(Vec<FileDiff>, RepoStats)> {
        let mut cmd = Command::new("git");
        cmd.current_dir(&self.repo_root);
        cmd.arg("diff");
        cmd.arg("-u");

        if ignore_whitespace {
            cmd.arg("--ignore-all-space");
        }

        if let Some(r) = target_ref {
            if self.repo_root.join(r).exists() || Path::new(r).exists() {
                if staged_only {
                    cmd.arg("--cached");
                } else if self.has_head() {
                    cmd.arg("HEAD");
                }
                cmd.arg("--");
                cmd.arg(r);
            } else {
                cmd.arg(r);
            }
        } else if staged_only {
            cmd.arg("--cached");
        } else if self.has_head() {
            cmd.arg("HEAD");
        } else {
            cmd.arg("--cached");
        }

        let output = cmd.output().context("Failed to run git diff")?;
        let diff_text = String::from_utf8_lossy(&output.stdout);
        let mut files = parse_unified_diff(&diff_text);

        // Fetch file statuses from `git status --porcelain -uall`
        let status_output = Command::new("git")
            .args(["status", "--porcelain=v1", "-uall"])
            .current_dir(&self.repo_root)
            .output();

        let mut status_map = std::collections::HashMap::new();
        let mut untracked_paths = Vec::new();

        if let Ok(out) = status_output {
            let status_text = String::from_utf8_lossy(&out.stdout);
            for line in status_text.lines() {
                if line.len() < 4 {
                    continue;
                }
                let index_status = line.as_bytes()[0] as char;
                let work_status = line.as_bytes()[1] as char;
                let path_str = line[3..].trim();
                let path = PathBuf::from(path_str);

                let stage_status = match (index_status, work_status) {
                    ('?', '?') => StageStatus::Untracked,
                    (' ', _) => StageStatus::Unstaged,
                    (_, ' ') => StageStatus::Staged,
                    _ => StageStatus::PartiallyStaged,
                };

                let file_status = match (index_status, work_status) {
                    ('?', '?') => FileStatus::Untracked,
                    ('A', _) | (_, 'A') => FileStatus::Added,
                    ('D', _) | (_, 'D') => FileStatus::Deleted,
                    ('R', _) | (_, 'R') => FileStatus::Renamed,
                    _ => FileStatus::Modified,
                };

                status_map.insert(path.clone(), (file_status, stage_status));

                if index_status == '?' && work_status == '?' {
                    untracked_paths.push(path);
                }
            }
        }

        // Apply statuses to parsed files
        for file in &mut files {
            if let Some((f_status, s_status)) = status_map.get(&file.new_path) {
                file.status = *f_status;
                file.stage_status = *s_status;
            }
        }

        // Add untracked files if requested and we are in default mode (not comparing commits)
        if include_untracked && target_ref.is_none() && !staged_only {
            for untracked_path in untracked_paths {
                // If not already in files
                if !files.iter().any(|f| f.new_path == untracked_path) {
                    if let Ok(untracked_diff) = self.create_untracked_diff(&untracked_path) {
                        files.push(untracked_diff);
                    }
                }
            }
        }

        // Precompute side-by-side aligned rows for each file
        for file in &mut files {
            file.aligned_rows = align_hunks_side_by_side(&file.hunks);
        }

        // Calculate repository stats
        let mut total_additions = 0;
        let mut total_deletions = 0;
        for file in &files {
            total_additions += file.stats.additions;
            total_deletions += file.stats.deletions;
        }

        let repo_stats = RepoStats {
            repo_name: self.get_repo_name(),
            branch: self.get_branch(),
            root_dir: self.repo_root.clone(),
            total_additions,
            total_deletions,
            file_count: files.len(),
        };

        Ok((files, repo_stats))
    }

    fn create_untracked_diff(&self, relative_path: &Path) -> Result<FileDiff> {
        let full_path = self.repo_root.join(relative_path);
        if !full_path.is_file() {
            anyhow::bail!("Path is not a regular file");
        }

        // Check file size (e.g. max 2MB)
        let metadata = std::fs::metadata(&full_path)?;
        if metadata.len() > 2 * 1024 * 1024 {
            return Ok(FileDiff {
                old_path: None,
                new_path: relative_path.to_path_buf(),
                status: FileStatus::Untracked,
                stage_status: StageStatus::Untracked,
                stats: ChangeStats::default(),
                hunks: Vec::new(),
                aligned_rows: Vec::new(),
                is_binary: true,
            });
        }

        let content = match std::fs::read_to_string(&full_path) {
            Ok(c) => c,
            Err(_) => {
                return Ok(FileDiff {
                    old_path: None,
                    new_path: relative_path.to_path_buf(),
                    status: FileStatus::Untracked,
                    stage_status: StageStatus::Untracked,
                    stats: ChangeStats::default(),
                    hunks: Vec::new(),
                    aligned_rows: Vec::new(),
                    is_binary: true,
                });
            }
        };

        let file_lines: Vec<&str> = content.lines().collect();
        let count = file_lines.len();

        let diff_lines: Vec<DiffLine> = file_lines
            .into_iter()
            .enumerate()
            .map(|(idx, line)| DiffLine {
                kind: DiffKind::Addition,
                content: line.to_string(),
                old_line_no: None,
                new_line_no: Some(idx + 1),
                spans: Vec::new(),
            })
            .collect();

        let hunk = Hunk {
            old_start: 0,
            old_lines: 0,
            new_start: 1,
            new_lines: count,
            header: format!("@@ -0,0 +1,{} @@", count),
            lines: diff_lines,
        };

        Ok(FileDiff {
            old_path: None,
            new_path: relative_path.to_path_buf(),
            status: FileStatus::Untracked,
            stage_status: StageStatus::Untracked,
            stats: ChangeStats {
                additions: count,
                deletions: 0,
            },
            aligned_rows: Vec::new(),
            hunks: vec![hunk],
            is_binary: false,
        })
    }

    pub fn get_file_history(&self, file_path: &Path, max_count: usize) -> Result<Vec<CommitEntry>> {
        let output = Command::new("git")
            .args([
                "log",
                "--follow",
                &format!("--format=%h\t%an\t%ar\t%s"),
                &format!("-n{}", max_count),
                "--",
            ])
            .arg(file_path)
            .current_dir(&self.repo_root)
            .output()?;

        if !output.status.success() {
            return Ok(Vec::new());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut entries = Vec::new();

        for line in stdout.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 4 {
                entries.push(CommitEntry {
                    hash: parts[0].to_string(),
                    author: parts[1].to_string(),
                    date: parts[2].to_string(),
                    message: parts[3].to_string(),
                });
            }
        }

        Ok(entries)
    }

    pub fn load_commit_diff_for_file(&self, commit_hash: &str, file_path: &Path) -> Result<Option<FileDiff>> {
        let output = Command::new("git")
            .args(["show", "--format=", "-p", commit_hash, "--"])
            .arg(file_path)
            .current_dir(&self.repo_root)
            .output()?;

        if !output.status.success() {
            anyhow::bail!("Failed to get commit diff for {}", commit_hash);
        }

        let diff_text = String::from_utf8_lossy(&output.stdout);
        let mut files = parse_unified_diff(&diff_text);
        if let Some(mut file) = files.pop() {
            file.aligned_rows = align_hunks_side_by_side(&file.hunks);
            Ok(Some(file))
        } else {
            Ok(None)
        }
    }

    pub fn get_repo_commits(&self, max_count: usize) -> Result<Vec<CommitEntry>> {
        let output = Command::new("git")
            .args([
                "log",
                &format!("--format=%h\t%an\t%ar\t%s"),
                &format!("-n{}", max_count),
            ])
            .current_dir(&self.repo_root)
            .output()?;

        if !output.status.success() {
            return Ok(Vec::new());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut entries = Vec::new();

        for line in stdout.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 4 {
                entries.push(CommitEntry {
                    hash: parts[0].to_string(),
                    author: parts[1].to_string(),
                    date: parts[2].to_string(),
                    message: parts[3].to_string(),
                });
            }
        }

        Ok(entries)
    }

    pub fn load_commit_full_diff(&self, commit_hash: &str) -> Result<Vec<FileDiff>> {
        let output = Command::new("git")
            .args(["show", "--format=", "-p", commit_hash])
            .current_dir(&self.repo_root)
            .output()?;

        if !output.status.success() {
            anyhow::bail!("Failed to get commit diff for {}", commit_hash);
        }

        let diff_text = String::from_utf8_lossy(&output.stdout);
        let mut files = parse_unified_diff(&diff_text);
        for file in &mut files {
            file.aligned_rows = align_hunks_side_by_side(&file.hunks);
        }
        Ok(files)
    }

    pub fn get_stashes(&self) -> Result<Vec<StashEntry>> {
        let output = Command::new("git")
            .args(["stash", "list", "--format=%gd\t%cr\t%gs"])
            .current_dir(&self.repo_root)
            .output()?;

        if !output.status.success() {
            return Ok(Vec::new());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut entries = Vec::new();

        for (idx, line) in stdout.lines().enumerate() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 3 {
                entries.push(StashEntry {
                    index: idx,
                    selector: parts[0].to_string(),
                    date: parts[1].to_string(),
                    message: parts[2].to_string(),
                });
            }
        }

        Ok(entries)
    }

    pub fn load_stash_diff(&self, stash_selector: &str) -> Result<Vec<FileDiff>> {
        let output = Command::new("git")
            .args(["stash", "show", "-p", stash_selector])
            .current_dir(&self.repo_root)
            .output()?;

        if !output.status.success() {
            anyhow::bail!("Failed to get stash diff for {}", stash_selector);
        }

        let diff_text = String::from_utf8_lossy(&output.stdout);
        let mut files = parse_unified_diff(&diff_text);
        for file in &mut files {
            file.aligned_rows = align_hunks_side_by_side(&file.hunks);
        }
        Ok(files)
    }

    pub fn get_worktrees(&self, current_pwd: &Path) -> Result<Vec<WorktreeEntry>> {
        let output = Command::new("git")
            .args(["worktree", "list", "--porcelain"])
            .current_dir(&self.repo_root)
            .output()?;

        if !output.status.success() {
            return Ok(Vec::new());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut entries = Vec::new();

        let mut curr_path: Option<PathBuf> = None;
        let mut curr_head = String::new();
        let mut curr_branch: Option<String> = None;
        let mut is_bare = false;

        for line in stdout.lines() {
            if line.is_empty() {
                if let Some(path) = curr_path.take() {
                    let is_current = current_pwd.starts_with(&path) || self.repo_root == path;
                    entries.push(WorktreeEntry {
                        path,
                        head: curr_head.clone(),
                        branch: curr_branch.take(),
                        is_bare,
                        is_current,
                    });
                }
                curr_head.clear();
                is_bare = false;
                continue;
            }

            if let Some(p) = line.strip_prefix("worktree ") {
                curr_path = Some(PathBuf::from(p.trim()));
            } else if let Some(h) = line.strip_prefix("HEAD ") {
                curr_head = h.trim().to_string();
            } else if let Some(b) = line.strip_prefix("branch ") {
                let b_clean = b.trim().strip_prefix("refs/heads/").unwrap_or(b.trim()).to_string();
                curr_branch = Some(b_clean);
            } else if line == "bare" {
                is_bare = true;
            }
        }

        if let Some(path) = curr_path.take() {
            let is_current = current_pwd.starts_with(&path) || self.repo_root == path;
            entries.push(WorktreeEntry {
                path,
                head: curr_head,
                branch: curr_branch,
                is_bare,
                is_current,
            });
        }

        Ok(entries)
    }

    pub fn add_worktree(&self, path: &str, branch: &str) -> Result<PathBuf> {
        let trimmed_path = path.trim();
        let trimmed_branch = branch.trim();
        if trimmed_path.is_empty() {
            anyhow::bail!("Worktree path cannot be empty");
        }

        let mut cmd = Command::new("git");
        cmd.current_dir(&self.repo_root);
        cmd.arg("worktree").arg("add");

        if trimmed_branch.is_empty() {
            cmd.arg(trimmed_path);
        } else {
            // Check if branch exists
            let branch_exists = Command::new("git")
                .args(["rev-parse", "--verify", trimmed_branch])
                .current_dir(&self.repo_root)
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);

            let remote_exists = Command::new("git")
                .args(["show-ref", "--verify", &format!("refs/remotes/{}", trimmed_branch)])
                .current_dir(&self.repo_root).output()
                .map(|o| o.status.success()).unwrap_or(false);
            if remote_exists {
                let local_name = trimmed_branch.split_once('/').map(|(_, name)| name).unwrap_or(trimmed_branch);
                cmd.arg("--track").arg("-b").arg(local_name).arg(trimmed_path).arg(trimmed_branch);
            } else if branch_exists {
                cmd.arg(trimmed_path).arg(trimmed_branch);
            } else {
                cmd.arg("-b").arg(trimmed_branch).arg(trimmed_path);
            }
        }

        let output = cmd.output().context("Failed to run git worktree add")?;
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("{}", err.trim());
        }

        let resolved = if Path::new(trimmed_path).is_absolute() {
            PathBuf::from(trimmed_path)
        } else {
            self.repo_root.join(trimmed_path)
        };

        Ok(resolved)
    }

    pub fn get_branches(&self) -> Vec<String> {
        let mut branches = Vec::new();
        if let Ok(out) = Command::new("git")
            .args(["branch", "--format=%(refname:short)"])
            .current_dir(&self.repo_root)
            .output()
        {
            if out.status.success() {
                for line in String::from_utf8_lossy(&out.stdout).lines() {
                    let b = line.trim();
                    if !b.is_empty() && !branches.contains(&b.to_string()) {
                        branches.push(b.to_string());
                    }
                }
            }
        }

        if let Ok(out) = Command::new("git")
            .args(["branch", "-r", "--format=%(refname:short)"])
            .current_dir(&self.repo_root)
            .output()
        {
            if out.status.success() {
                for line in String::from_utf8_lossy(&out.stdout).lines() {
                    let b = line.trim();
                    if !b.is_empty() && !b.ends_with("/HEAD") {
                        let clean = b;
                        if !branches.contains(&clean.to_string()) {
                            branches.push(clean.to_string());
                        }
                    }
                }
            }
        }

        branches.sort();
        branches.dedup();
        branches
    }
}

