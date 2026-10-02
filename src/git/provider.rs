use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::core::aligner::align_hunks_side_by_side;
use crate::core::models::{
    ChangeStats, CommitEntry, DiffKind, DiffLine, DiffSection, FileDiff, FileStatus, Hunk,
    RepoStats, StageStatus, StashEntry, WorktreeEntry,
};
use crate::git::patch::parse_unified_diff;

pub struct GitProvider {
    pub repo_root: PathBuf,
    pub context_lines: usize,
}

/// Reads never rewrite the index (GIT_OPTIONAL_LOCKS=0), so the watcher's
/// `.git/index` trigger can't loop on diffv's own `git status`.
fn git_command() -> Command {
    let mut cmd = Command::new("git");
    cmd.args(["-c", "core.quotePath=false"])
        .env("GIT_OPTIONAL_LOCKS", "0");
    cmd
}

impl GitProvider {
    pub fn discover(start_dir: Option<&Path>) -> Result<Self> {
        let mut cmd = git_command();
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
            context_lines: 3,
        })
    }

    fn unified_arg(&self) -> String {
        format!("-U{}", self.context_lines)
    }

    pub fn get_branch(&self) -> String {
        let output = git_command()
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
        let output_ref = git_command()
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
        git_command()
            .args(["rev-parse", "--verify", "HEAD"])
            .current_dir(&self.repo_root)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    pub fn is_ref(&self, r: &str) -> bool {
        git_command()
            .args(["rev-parse", "--verify", &format!("{}^{{commit}}", r)])
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
        // Working-tree mode splits HEAD → index (staged) from index → worktree
        // (changes) so every hunk is exactly the patch git needs to (un)stage it.
        let split = target_ref.is_none() && !staged_only;
        let mut files = if split {
            let mut staged = self.run_git_diff(&["--cached".to_string()], ignore_whitespace)?;
            for file in &mut staged {
                file.section = DiffSection::Staged;
            }
            staged.extend(self.run_git_diff(&[], ignore_whitespace)?);
            staged
        } else {
            let mut args = Vec::new();
            match target_ref {
                Some(r)
                    if !self.is_ref(r)
                        && (self.repo_root.join(r).exists() || Path::new(r).exists()) =>
                {
                    if staged_only {
                        args.push("--cached".to_string());
                    } else if self.has_head() {
                        args.push("HEAD".to_string());
                    }
                    args.push("--".to_string());
                    args.push(r.to_string());
                }
                Some(r) => args.push(r.to_string()),
                None => args.push("--cached".to_string()),
            }
            self.run_git_diff(&args, ignore_whitespace)?
        };

        // Fetch file statuses from `git status --porcelain -uall`
        // `-z` keeps paths verbatim (spaces, unicode) and lists rename sources separately.
        let status_output = git_command()
            .args(["status", "--porcelain=v1", "-z", "-uall"])
            .current_dir(&self.repo_root)
            .output();

        let mut status_map = std::collections::HashMap::new();
        let mut untracked_paths = Vec::new();

        if let Ok(out) = status_output {
            let status_text = String::from_utf8_lossy(&out.stdout);
            let mut entries = status_text.split('\0');
            while let Some(entry) = entries.next() {
                let bytes = entry.as_bytes();
                if bytes.len() < 4 {
                    continue;
                }
                let index_status = bytes[0] as char;
                let work_status = bytes[1] as char;
                if matches!(index_status, 'R' | 'C') || matches!(work_status, 'R' | 'C') {
                    entries.next();
                }
                let path = PathBuf::from(&entry[3..]);

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

                if index_status == '?' && work_status == '?' {
                    untracked_paths.push(path.clone());
                }
                status_map.insert(path, (file_status, stage_status));
            }
        }

        // Apply statuses to parsed files
        for file in &mut files {
            if let Some((f_status, s_status)) = status_map.get(&file.new_path) {
                file.status = *f_status;
                file.stage_status = match (split, file.section) {
                    (true, DiffSection::Staged) => StageStatus::Staged,
                    (true, DiffSection::Changes) => StageStatus::Unstaged,
                    (false, _) => *s_status,
                };
            }
        }

        // Add untracked files if requested and we are in working tree mode (not comparing commit ranges)
        let is_commit_range = target_ref.is_some_and(|r| r.contains(".."));
        if include_untracked && !is_commit_range && !staged_only {
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
            file_count: files
                .iter()
                .map(|f| &f.new_path)
                .collect::<std::collections::HashSet<_>>()
                .len(),
        };

        Ok((files, repo_stats))
    }

    fn run_git_diff(&self, args: &[String], ignore_whitespace: bool) -> Result<Vec<FileDiff>> {
        let mut cmd = git_command();
        cmd.current_dir(&self.repo_root)
            .arg("diff")
            .arg(self.unified_arg());
        if ignore_whitespace {
            cmd.arg("--ignore-all-space");
        }
        let output = cmd.args(args).output().context("Failed to run git diff")?;
        Ok(parse_unified_diff(&String::from_utf8_lossy(&output.stdout)))
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
                section: DiffSection::Changes,
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
                    section: DiffSection::Changes,
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
            section: DiffSection::Changes,
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
        let output = git_command()
            .args([
                "log",
                "--follow",
                "--format=%h\t%an\t%ar\t%s",
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

    pub fn load_commit_diff_for_file(
        &self,
        commit_hash: &str,
        file_path: &Path,
    ) -> Result<Option<FileDiff>> {
        let output = git_command()
            .args([
                "show",
                "--format=",
                "-p",
                &self.unified_arg(),
                commit_hash,
                "--",
            ])
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
        let output = git_command()
            .args([
                "log",
                "--format=%h\t%an\t%ar\t%s",
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
        let output = git_command()
            .args(["show", "--format=", "-p", &self.unified_arg(), commit_hash])
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
        let output = git_command()
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
        let output = git_command()
            .args(["stash", "show", "-p", &self.unified_arg(), stash_selector])
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
        let output = git_command()
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
                let b_clean = b
                    .trim()
                    .strip_prefix("refs/heads/")
                    .unwrap_or(b.trim())
                    .to_string();
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

        let mut cmd = git_command();
        cmd.current_dir(&self.repo_root);
        cmd.arg("worktree").arg("add");

        if trimmed_branch.is_empty() {
            cmd.arg(trimmed_path);
        } else {
            // Check if branch exists
            let branch_exists = git_command()
                .args(["rev-parse", "--verify", trimmed_branch])
                .current_dir(&self.repo_root)
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);

            let remote_exists = git_command()
                .args([
                    "show-ref",
                    "--verify",
                    &format!("refs/remotes/{}", trimmed_branch),
                ])
                .current_dir(&self.repo_root)
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);
            if remote_exists {
                let local_name = trimmed_branch
                    .split_once('/')
                    .map(|(_, name)| name)
                    .unwrap_or(trimmed_branch);
                cmd.arg("--track")
                    .arg("-b")
                    .arg(local_name)
                    .arg(trimmed_path)
                    .arg(trimmed_branch);
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
        if let Ok(out) = git_command()
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

        if let Ok(out) = git_command()
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
