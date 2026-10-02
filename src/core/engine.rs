use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use similar::{Algorithm, ChangeTag, TextDiff};
use walkdir::WalkDir;

use crate::core::aligner::align_hunks_side_by_side;
use crate::core::models::{
    ChangeStats, DiffKind, DiffLine, FileDiff, FileStatus, Hunk, RepoStats, StageStatus,
};
use crate::git::patch::parse_unified_diff;

pub struct DiffEngine {
    pub algorithm: Algorithm,
    pub context_lines: usize,
    pub ignore_whitespace: bool,
}

impl Default for DiffEngine {
    fn default() -> Self {
        Self {
            algorithm: Algorithm::Patience,
            context_lines: 3,
            ignore_whitespace: false,
        }
    }
}

impl DiffEngine {
    pub fn new(algo: &str, context_lines: usize, ignore_whitespace: bool) -> Self {
        let algorithm = match algo.to_lowercase().as_str() {
            "myers" => Algorithm::Myers,
            "patience" => Algorithm::Patience,
            "lcs" => Algorithm::Lcs,
            _ => Algorithm::Patience,
        };
        Self {
            algorithm,
            context_lines,
            ignore_whitespace,
        }
    }

    /// Compares two arbitrary files on disk.
    pub fn compare_files(&self, path_a: &Path, path_b: &Path) -> Result<FileDiff> {
        let content_a = fs::read_to_string(path_a)
            .with_context(|| format!("Failed to read file {}", path_a.display()))?;
        let content_b = fs::read_to_string(path_b)
            .with_context(|| format!("Failed to read file {}", path_b.display()))?;

        let mut diff = self.diff_texts(&content_a, &content_b, Some(path_a), path_b);
        diff.status = FileStatus::Modified;
        diff.aligned_rows = align_hunks_side_by_side(&diff.hunks);
        Ok(diff)
    }

    /// Compares two arbitrary directories recursively.
    pub fn compare_directories(&self, dir_a: &Path, dir_b: &Path) -> Result<Vec<FileDiff>> {
        let mut rel_paths = std::collections::BTreeSet::new();

        for entry in WalkDir::new(dir_a).into_iter().filter_map(|e| e.ok()) {
            if entry.file_type().is_file() {
                if let Ok(rel) = entry.path().strip_prefix(dir_a) {
                    rel_paths.insert(rel.to_path_buf());
                }
            }
        }

        for entry in WalkDir::new(dir_b).into_iter().filter_map(|e| e.ok()) {
            if entry.file_type().is_file() {
                if let Ok(rel) = entry.path().strip_prefix(dir_b) {
                    rel_paths.insert(rel.to_path_buf());
                }
            }
        }

        let mut results = Vec::new();

        for rel in rel_paths {
            let file_a = dir_a.join(&rel);
            let file_b = dir_b.join(&rel);

            let exists_a = file_a.exists();
            let exists_b = file_b.exists();

            if exists_a && exists_b {
                let content_a = fs::read_to_string(&file_a);
                let content_b = fs::read_to_string(&file_b);

                match (content_a, content_b) {
                    (Ok(ca), Ok(cb)) => {
                        if ca != cb {
                            let mut diff = self.diff_texts(&ca, &cb, Some(&file_a), &file_b);
                            diff.new_path = rel.clone();
                            diff.old_path = Some(rel.clone());
                            diff.status = FileStatus::Modified;
                            diff.aligned_rows = align_hunks_side_by_side(&diff.hunks);
                            results.push(diff);
                        }
                    }
                    _ => {
                        // Binary or read error
                    }
                }
            } else if !exists_a && exists_b {
                // Added file
                if let Ok(cb) = fs::read_to_string(&file_b) {
                    let mut diff = self.diff_texts("", &cb, None, &file_b);
                    diff.new_path = rel.clone();
                    diff.status = FileStatus::Added;
                    diff.aligned_rows = align_hunks_side_by_side(&diff.hunks);
                    results.push(diff);
                }
            } else if exists_a && !exists_b {
                // Deleted file
                if let Ok(ca) = fs::read_to_string(&file_a) {
                    let mut diff = self.diff_texts(&ca, "", Some(&file_a), &rel);
                    diff.new_path = rel.clone();
                    diff.status = FileStatus::Deleted;
                    diff.aligned_rows = align_hunks_side_by_side(&diff.hunks);
                    results.push(diff);
                }
            }
        }

        Ok(results)
    }

    /// Reads patch from standard input and parses into FileDiffs.
    pub fn compare_stdin(&self) -> Result<(Vec<FileDiff>, RepoStats)> {
        let mut buffer = String::new();
        io::stdin().read_to_string(&mut buffer)?;

        let mut files = parse_unified_diff(&buffer);
        for file in &mut files {
            file.aligned_rows = align_hunks_side_by_side(&file.hunks);
        }

        let mut total_additions = 0;
        let mut total_deletions = 0;
        for file in &files {
            total_additions += file.stats.additions;
            total_deletions += file.stats.deletions;
        }

        let repo_stats = RepoStats {
            repo_name: "stdin".to_string(),
            branch: "patch".to_string(),
            root_dir: PathBuf::from("."),
            total_additions,
            total_deletions,
            file_count: files.len(),
        };

        Ok((files, repo_stats))
    }

    /// Builds a FileDiff from two string contents using the similar crate.
    pub fn diff_texts(
        &self,
        old_text: &str,
        new_text: &str,
        old_path: Option<&Path>,
        new_path: &Path,
    ) -> FileDiff {
        let mut text_diff_builder = TextDiff::configure();
        text_diff_builder.algorithm(self.algorithm);

        let text_diff = text_diff_builder.diff_lines(old_text, new_text);

        let mut hunks = Vec::new();
        let mut additions = 0;
        let mut deletions = 0;

        for group in text_diff.grouped_ops(self.context_lines) {
            let mut hunk_lines = Vec::new();
            let mut old_start = 1;
            let mut old_count = 0;
            let mut new_start = 1;
            let mut new_count = 0;

            if let Some(first_op) = group.first() {
                old_start = first_op.old_range().start + 1;
                new_start = first_op.new_range().start + 1;
            }

            for op in &group {
                for change in text_diff.iter_changes(op) {
                    let text = change.value().trim_end_matches(&['\r', '\n'][..]).to_string();
                    match change.tag() {
                        ChangeTag::Equal => {
                            hunk_lines.push(DiffLine {
                                kind: DiffKind::Context,
                                content: text,
                                old_line_no: change.old_index().map(|i| i + 1),
                                new_line_no: change.new_index().map(|i| i + 1),
                                spans: Vec::new(),
                            });
                            old_count += 1;
                            new_count += 1;
                        }
                        ChangeTag::Delete => {
                            hunk_lines.push(DiffLine {
                                kind: DiffKind::Deletion,
                                content: text,
                                old_line_no: change.old_index().map(|i| i + 1),
                                new_line_no: None,
                                spans: Vec::new(),
                            });
                            old_count += 1;
                            deletions += 1;
                        }
                        ChangeTag::Insert => {
                            hunk_lines.push(DiffLine {
                                kind: DiffKind::Addition,
                                content: text,
                                old_line_no: None,
                                new_line_no: change.new_index().map(|i| i + 1),
                                spans: Vec::new(),
                            });
                            new_count += 1;
                            additions += 1;
                        }
                    }
                }
            }

            let header = format!(
                "@@ -{},{} +{},{} @@",
                old_start, old_count, new_start, new_count
            );

            hunks.push(Hunk {
                old_start,
                old_lines: old_count,
                new_start,
                new_lines: new_count,
                header,
                lines: hunk_lines,
            });
        }

        FileDiff {
            old_path: old_path.map(|p| p.to_path_buf()),
            new_path: new_path.to_path_buf(),
            status: FileStatus::Modified,
            stage_status: StageStatus::Unstaged,
            stats: ChangeStats {
                additions,
                deletions,
            },
            aligned_rows: Vec::new(),
            hunks,
            is_binary: false,
        }
    }
}
