use std::io::Write;
use std::process::{Command, Stdio};

pub fn is_fzf_available() -> bool {
    Command::new("fzf")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Key fzf is told to hand back (`--expect`) so diffv can switch to its built-in picker.
pub const SWITCH_ENGINE_KEY: &str = "ctrl-g";

/// How an fzf session ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FzfResult {
    Selected(String),
    /// Ctrl+G: continue the same search in the built-in picker with this query.
    SwitchEngine(String),
    Cancelled,
}

/// fzf output split into its parts (see `parse_fzf_output`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FzfOutput {
    pub query: String,
    /// Key that completed fzf; empty for the default Enter.
    pub key: String,
    pub selection: Option<String>,
}

/// Parses the output of `fzf --print-query --expect=ctrl-g`.
///
/// The fzf manual says `--print-query` prints the query "as the first line" and
/// `--expect` prints the key "as the first line ... (or as the second line if
/// --print-query is also used)", empty when Enter completed fzf; selections follow.
/// The parser takes that documented order (query, key, selection) but stays
/// defensive: if the second line is not an expected key while the first one is,
/// it accepts the key-first order too; missing lines mean an empty query/key and
/// no selection.
pub fn parse_fzf_output(stdout: &str) -> FzfOutput {
    let text = stdout.strip_suffix('\n').unwrap_or(stdout);
    let lines: Vec<&str> = if stdout.is_empty() {
        Vec::new()
    } else {
        text.split('\n').map(|l| l.trim_end_matches('\r')).collect()
    };
    let is_key = |line: &str| line.is_empty() || line == SWITCH_ENGINE_KEY;
    let first = lines.first().copied().unwrap_or_default();
    let second = lines.get(1).copied().unwrap_or_default();
    let (query, key) = if lines.len() >= 2 && !is_key(second) && first == SWITCH_ENGINE_KEY {
        (second, first)
    } else {
        (first, second)
    };
    let selection = lines
        .iter()
        .skip(2)
        .find(|l| !l.is_empty())
        .map(|l| l.to_string());
    FzfOutput {
        query: query.to_string(),
        key: key.to_string(),
        selection,
    }
}

pub fn search_files_fzf(files: &[String], header: &str, query: &str) -> anyhow::Result<FzfResult> {
    run_fzf("fzf", files, "󰈞 Files> ", header, query, &[])
}

/// Candidates are `path:line<TAB>content`; only the content is matched.
pub fn search_diff_text_fzf(
    lines: &[String],
    header: &str,
    query: &str,
) -> anyhow::Result<FzfResult> {
    run_fzf(
        "fzf",
        lines,
        "󰈞 Diff Text> ",
        header,
        query,
        &[
            "--delimiter=\t",
            "--nth=2",
            "--with-nth=1,2,3",
            "--tabstop=2",
        ],
    )
}

fn run_fzf(
    program: &str,
    items: &[String],
    prompt: &str,
    header: &str,
    query: &str,
    extra: &[&str],
) -> anyhow::Result<FzfResult> {
    let mut child = Command::new(program)
        .args(["--layout=reverse", "--border=rounded", "--info=inline"])
        .arg(format!("--prompt={}", prompt))
        .arg(format!("--header={} · Ctrl+G: built-in picker", header))
        .arg(format!("--query={}", query))
        .arg("--print-query")
        .arg(format!("--expect={}", SWITCH_ENGINE_KEY))
        .args(extra)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        for item in items {
            let _ = writeln!(stdin, "{}", item);
        }
    }

    let output = child.wait_with_output()?;
    match output.status.code() {
        // 0: selection made; 1: no match (an --expect key can still have been pressed)
        Some(0 | 1) => {}
        Some(130) => return Ok(FzfResult::Cancelled),
        _ => anyhow::bail!("fzf exited with {}", output.status),
    }
    let parsed = parse_fzf_output(&String::from_utf8_lossy(&output.stdout));
    Ok(if parsed.key == SWITCH_ENGINE_KEY {
        FzfResult::SwitchEngine(parsed.query)
    } else if let Some(selection) = parsed.selection {
        FzfResult::Selected(selection)
    } else {
        FzfResult::Cancelled
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_documented_order_query_key_selection() {
        let out = parse_fzf_output("mai\n\nsrc/main.rs\n");
        assert_eq!(out.query, "mai");
        assert_eq!(out.key, "");
        assert_eq!(out.selection.as_deref(), Some("src/main.rs"));

        let switch = parse_fzf_output("mai\nctrl-g\nsrc/main.rs\n");
        assert_eq!(
            (switch.query.as_str(), switch.key.as_str()),
            ("mai", "ctrl-g")
        );
    }

    #[test]
    fn parses_key_first_order_defensively() {
        let out = parse_fzf_output("ctrl-g\nmai\nsrc/main.rs\n");
        assert_eq!((out.query.as_str(), out.key.as_str()), ("mai", "ctrl-g"));
        assert_eq!(out.selection.as_deref(), Some("src/main.rs"));
    }

    #[test]
    fn handles_missing_selection_empty_query_and_empty_output() {
        // Ctrl+G with no match: query + key, nothing selected.
        let out = parse_fzf_output("zzz\nctrl-g\n");
        assert_eq!((out.query.as_str(), out.key.as_str()), ("zzz", "ctrl-g"));
        assert_eq!(out.selection, None);
        // Empty query, Ctrl+G, selection present.
        let out = parse_fzf_output("\nctrl-g\na.rs\n");
        assert_eq!((out.query.as_str(), out.key.as_str()), ("", "ctrl-g"));
        assert_eq!(out.selection.as_deref(), Some("a.rs"));
        // Query only (no key line) and nothing at all.
        assert_eq!(parse_fzf_output("abc\n").key, "");
        let empty = parse_fzf_output("");
        assert_eq!((empty.query.as_str(), empty.key.as_str()), ("", ""));
        assert_eq!(empty.selection, None);
        // Text candidates keep their tabs and CRLF is tolerated.
        let out = parse_fzf_output("x\r\n\r\nmain.rs:3\t+ let x = 1;\r\n");
        assert_eq!(out.selection.as_deref(), Some("main.rs:3\t+ let x = 1;"));
    }

    #[cfg(unix)]
    #[test]
    fn stub_fzf_round_trip_passes_query_and_reads_switch_key() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("args.log");
        let stub = dir.path().join("fzf");
        // Prints what the real fzf prints with --print-query --expect=ctrl-g after
        // the user pressed Ctrl+G with the given --query.
        std::fs::write(
            &stub,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\ncat > /dev/null\nfor a in \"$@\"; do case \"$a\" in --query=*) q=\"${{a#--query=}}\";; esac; done\nprintf '%s\\nctrl-g\\n' \"$q\"\n",
                log.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();
        let items = vec!["a.rs".to_string(), "b.rs".to_string()];
        let res = run_fzf(stub.to_str().unwrap(), &items, "> ", "h", "b r", &[]).unwrap();
        assert_eq!(res, FzfResult::SwitchEngine("b r".into()));
        let args = std::fs::read_to_string(&log).unwrap();
        for expected in ["--query=b r", "--print-query", "--expect=ctrl-g"] {
            assert!(
                args.lines().any(|l| l == expected),
                "missing {expected}: {args}"
            );
        }
    }
}
