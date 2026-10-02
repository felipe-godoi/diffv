use std::path::Path;
use arboard::Clipboard;
use crate::core::models::{DiffKind, Hunk};

pub fn copy_hunk_as_markdown(file_path: &Path, hunk: &Hunk) -> anyhow::Result<()> {
    let mut text = format!(
        "### File: `{}` (Hunk @@ -{},{} +{},{} @@)\n```diff\n",
        file_path.display(),
        hunk.old_start,
        hunk.old_lines,
        hunk.new_start,
        hunk.new_lines
    );

    for line in &hunk.lines {
        match line.kind {
            DiffKind::Context => {
                text.push(' ');
                text.push_str(&line.content);
                text.push('\n');
            }
            DiffKind::Addition => {
                text.push('+');
                text.push_str(&line.content);
                text.push('\n');
            }
            DiffKind::Deletion => {
                text.push('-');
                text.push_str(&line.content);
                text.push('\n');
            }
            DiffKind::Virtual => {}
        }
    }
    text.push_str("```\n");

    let mut clipboard = Clipboard::new()?;
    clipboard.set_text(text)?;
    Ok(())
}

pub fn copy_file_diff_as_markdown(file_path: &Path, hunks: &[Hunk]) -> anyhow::Result<()> {
    let mut text = format!("### File: `{}`\n```diff\n", file_path.display());

    for hunk in hunks {
        text.push_str(&hunk.header);
        text.push('\n');
        for line in &hunk.lines {
            match line.kind {
                DiffKind::Context => {
                    text.push(' ');
                    text.push_str(&line.content);
                    text.push('\n');
                }
                DiffKind::Addition => {
                    text.push('+');
                    text.push_str(&line.content);
                    text.push('\n');
                }
                DiffKind::Deletion => {
                    text.push('-');
                    text.push_str(&line.content);
                    text.push('\n');
                }
                DiffKind::Virtual => {}
            }
        }
    }
    text.push_str("```\n");

    let mut clipboard = Clipboard::new()?;
    clipboard.set_text(text)?;
    Ok(())
}
