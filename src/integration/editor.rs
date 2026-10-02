use std::path::Path;
use std::process::Command;
use crate::config::EditorConfig;

pub fn open_editor(
    file_path: &Path,
    line_no: usize,
    editor_cfg: &EditorConfig,
) -> anyhow::Result<()> {
    // 1. Check if nvr is requested and $NVIM is set
    if editor_cfg.use_nvr && std::env::var("NVIM").is_ok() {
        let nvr_status = Command::new("nvr")
            .arg(format!("+{}", line_no))
            .arg(file_path)
            .status();

        if let Ok(status) = nvr_status {
            if status.success() {
                return Ok(());
            }
        }
    }

    // 2. Format command and args
    let line_str = line_no.to_string();
    let file_str = file_path.to_string_lossy().to_string();

    let args: Vec<String> = editor_cfg
        .args
        .iter()
        .map(|arg| {
            arg.replace("{{line}}", &line_str)
                .replace("{{file}}", &file_str)
        })
        .collect();

    let mut cmd = Command::new(&editor_cfg.command);
    cmd.args(&args);

    let status = cmd.status()?;
    if !status.success() {
        anyhow::bail!("Editor exited with status: {}", status);
    }

    Ok(())
}
