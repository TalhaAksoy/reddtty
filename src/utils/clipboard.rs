//! Cross-platform clipboard support for permalink copying.
//!
//! The text is written through stdin, never interpolated into a shell command.

use std::io::Write;
use std::process::{Command, Stdio};

fn candidates() -> &'static [(&'static str, &'static [&'static str])] {
    #[cfg(target_os = "macos")]
    {
        &[("pbcopy", &[])]
    }
    #[cfg(target_os = "windows")]
    {
        &[("clip", &[])]
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        &[
            ("wl-copy", &[]),
            ("xclip", &["-selection", "clipboard"]),
            ("xsel", &["--clipboard", "--input"]),
        ]
    }
}

pub fn copy(text: &str) -> Result<(), String> {
    let mut last_error = String::from("no clipboard tool is available");
    for (program, args) in candidates() {
        match Command::new(program)
            .args(*args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(mut child) => {
                if let Some(mut stdin) = child.stdin.take()
                    && let Err(error) = stdin.write_all(text.as_bytes())
                {
                    last_error = format!("could not write to {program}: {error}");
                    continue;
                }
                match child.wait() {
                    Ok(status) if status.success() => return Ok(()),
                    Ok(status) => last_error = format!("{program} exited with {status}"),
                    Err(error) => last_error = format!("could not wait for {program}: {error}"),
                }
            }
            Err(error) => last_error = format!("could not run {program}: {error}"),
        }
    }
    Err(format!("Could not copy to the clipboard ({last_error})."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn at_least_one_clipboard_candidate_is_configured() {
        assert!(!candidates().is_empty());
        assert!(candidates().iter().all(|(program, _)| !program.is_empty()));
    }
}
