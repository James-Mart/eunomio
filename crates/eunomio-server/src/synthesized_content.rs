// SPDX-License-Identifier: Apache-2.0

use anyhow::{anyhow, Result};
use eunomio_core::{FileLineRanges, LineRanges, SynthesizedRanges};
use std::path::Path;
use tokio::process::Command;

/// Compute synthesized-content word ranges along the displayed `parent → child`
/// Edge, judged against the Reference pair `(before_ref, after_ref)`.
pub async fn compute(
    repo: &Path,
    parent_tree: &str,
    child_tree: &str,
    before_ref: &str,
    after_ref: &str,
) -> Result<SynthesizedRanges> {
    let child = if child_tree == after_ref {
        Vec::new()
    } else {
        let raw = run_word_diff(repo, child_tree, after_ref).await?;
        parse_porcelain(&raw)
    };

    let parent = if parent_tree == before_ref {
        Vec::new()
    } else {
        let raw = run_word_diff(repo, parent_tree, before_ref).await?;
        parse_porcelain(&raw)
    };

    Ok(SynthesizedRanges { child, parent })
}

async fn run_word_diff(repo: &Path, a: &str, b: &str) -> Result<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args([
            "diff",
            "--word-diff=porcelain",
            "--no-color",
            "--no-ext-diff",
            "--end-of-options",
            a,
            b,
        ])
        .output()
        .await?;
    if !out.status.success() {
        return Err(anyhow!(
            "git diff --word-diff {} {}: {}",
            a,
            b,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

struct HunkHeader {
    old_start: u32,
}

fn parse_hunk_header(line: &str) -> Option<HunkHeader> {
    let rest = line.strip_prefix("@@ -")?;
    let (old_part, rest) = rest.split_once(' ')?;
    rest.strip_prefix('+')?;
    let parse_start = |part: &str| -> Option<u32> {
        part.split(',').next()?.parse::<u32>().ok()
    };
    Some(HunkHeader {
        old_start: parse_start(old_part)?,
    })
}

fn utf16_len(s: &str) -> u32 {
    s.chars().map(|c| c.len_utf16() as u32).sum()
}

fn parse_porcelain(raw: &str) -> Vec<FileLineRanges> {
    let mut files: Vec<FileLineRanges> = Vec::new();
    let mut current_path: Option<String> = None;
    let mut current_lines: Vec<LineRanges> = Vec::new();
    let mut binary = false;

    // `compute` always runs `git diff <side> <reference>`, so the side we want
    // to mark is the "old" side of the porcelain output. We track only old
    // line/column.
    let mut in_hunk = false;
    let mut old_line: u32 = 0;
    let mut old_col: u32 = 0;
    let mut old_active = false;
    let mut current_line_spans: Vec<(u32, u32)> = Vec::new();

    let push_file =
        |files: &mut Vec<FileLineRanges>, path: &mut Option<String>, lines: &mut Vec<LineRanges>| {
            let taken_path = path.take();
            let taken_lines = std::mem::take(lines);
            if let Some(p) = taken_path {
                if !taken_lines.is_empty() {
                    files.push(FileLineRanges {
                        path: p,
                        lines: taken_lines,
                    });
                }
            }
        };

    for line in raw.lines() {
        if line.starts_with("diff --git ") {
            push_file(&mut files, &mut current_path, &mut current_lines);
            in_hunk = false;
            binary = false;
            current_line_spans.clear();
            old_active = false;
            continue;
        }

        if !in_hunk {
            if let Some(rest) = line.strip_prefix("--- ") {
                current_path = if rest == "/dev/null" {
                    None
                } else {
                    Some(rest.strip_prefix("a/").unwrap_or(rest).to_string())
                };
                continue;
            }
            if line.starts_with("Binary files ") {
                binary = true;
                continue;
            }
            if let Some(h) = parse_hunk_header(line) {
                if binary || current_path.is_none() {
                    continue;
                }
                in_hunk = true;
                old_line = h.old_start;
                old_col = 0;
                old_active = false;
                current_line_spans.clear();
                continue;
            }
            continue;
        }

        if let Some(h) = parse_hunk_header(line) {
            if !current_line_spans.is_empty() {
                current_lines.push(LineRanges {
                    line: old_line,
                    spans: std::mem::take(&mut current_line_spans),
                });
            }
            old_line = h.old_start;
            old_col = 0;
            old_active = false;
            continue;
        }

        if line == "~" {
            if !current_line_spans.is_empty() {
                current_lines.push(LineRanges {
                    line: old_line,
                    spans: std::mem::take(&mut current_line_spans),
                });
            }
            if old_active {
                old_line += 1;
            }
            old_col = 0;
            old_active = false;
            continue;
        }

        let (prefix, word) = match line.as_bytes().first() {
            Some(b' ') => (b' ', &line[1..]),
            Some(b'-') => (b'-', &line[1..]),
            Some(b'+') => (b'+', &line[1..]),
            _ => continue,
        };
        let len = utf16_len(word);
        match prefix {
            b' ' => {
                old_col += len;
                old_active = true;
            }
            b'-' => {
                current_line_spans.push((old_col, old_col + len));
                old_col += len;
                old_active = true;
            }
            b'+' => {}
            _ => {}
        }
    }

    if !current_line_spans.is_empty() {
        current_lines.push(LineRanges {
            line: old_line,
            spans: current_line_spans,
        });
    }
    push_file(&mut files, &mut current_path, &mut current_lines);

    files
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(line: u32, spans: Vec<(u32, u32)>) -> LineRanges {
        LineRanges { line, spans }
    }

    fn file(path: &str, lines: Vec<LineRanges>) -> FileLineRanges {
        FileLineRanges {
            path: path.to_string(),
            lines,
        }
    }

    #[test]
    fn parses_pure_deletions_and_modifies() {
        // Rust's `\<newline>+whitespace` continuation eats the leading-space
        // prefix that porcelain uses to mark context words, so fixtures use
        // `concat!` with explicit newlines.
        let raw = concat!(
            "diff --git a/a.txt b/a.txt\n",
            "index aaa..bbb 100644\n",
            "--- a/a.txt\n",
            "+++ b/a.txt\n",
            "@@ -1,5 +1,3 @@\n",
            " A\n",
            "~\n",
            "-B\n",
            "~\n",
            "-C\n",
            "~\n",
            " D\n",
            "~\n",
            "-E\n",
            "+F\n",
            "~\n",
        );
        assert_eq!(
            parse_porcelain(raw),
            vec![file(
                "a.txt",
                vec![
                    line(2, vec![(0, 1)]),
                    line(3, vec![(0, 1)]),
                    line(5, vec![(0, 1)]),
                ],
            )]
        );
    }

    #[test]
    fn skips_binary_files() {
        let raw = concat!(
            "diff --git a/a.txt b/a.txt\n",
            "index aaa..bbb 100644\n",
            "--- a/a.txt\n",
            "+++ b/a.txt\n",
            "@@ -1 +1 @@\n",
            "-old\n",
            "+new\n",
            "~\n",
            "diff --git a/b.dat b/b.dat\n",
            "index ccc..ddd 100644\n",
            "Binary files a/b.dat and b/b.dat differ\n",
        );
        assert_eq!(
            parse_porcelain(raw),
            vec![file("a.txt", vec![line(1, vec![(0, 3)])])]
        );
    }

    #[test]
    fn intra_line_word_marks() {
        // Mirror real `--word-diff=porcelain` output: each line's first byte is
        // the prefix; the rest is the word. Leading whitespace inside the word
        // (e.g. ` baz`) is preserved, so the fixture has `  baz` for prefix
        // space + word " baz".
        let raw = concat!(
            "diff --git a/a.txt b/a.txt\n",
            "--- a/a.txt\n",
            "+++ b/a.txt\n",
            "@@ -1 +1 @@\n",
            " foo \n",
            "-bar\n",
            "+qux\n",
            "  baz\n",
            "~\n",
        );
        assert_eq!(
            parse_porcelain(raw),
            vec![file("a.txt", vec![line(1, vec![(4, 7)])])]
        );
    }

    #[test]
    fn keys_left_path_under_rename() {
        let raw = concat!(
            "diff --git a/old/path.txt b/new/path.txt\n",
            "similarity index 80%\n",
            "rename from old/path.txt\n",
            "rename to new/path.txt\n",
            "--- a/old/path.txt\n",
            "+++ b/new/path.txt\n",
            "@@ -1 +1 @@\n",
            "-gone\n",
            "+stay\n",
            "~\n",
        );
        assert_eq!(parse_porcelain(raw)[0].path, "old/path.txt");
    }

    #[test]
    fn empty_for_no_changes() {
        assert_eq!(parse_porcelain(""), vec![]);
    }

    #[test]
    fn handles_dev_null_old_side() {
        // New file: old side is /dev/null. We don't key by it; resulting file
        // has no old-side path so we drop it (no left-side spans possible).
        let raw = concat!(
            "diff --git a/new.txt b/new.txt\n",
            "new file mode 100644\n",
            "--- /dev/null\n",
            "+++ b/new.txt\n",
            "@@ -0,0 +1 @@\n",
            "+hello\n",
            "~\n",
        );
        assert!(parse_porcelain(raw).is_empty());
    }

    struct TestRepo {
        _dir: tempfile::TempDir,
        path: std::path::PathBuf,
    }

    impl TestRepo {
        fn new() -> Self {
            let dir = tempfile::tempdir().expect("tempdir");
            let path = dir.path().to_path_buf();
            run_git(&path, &["init", "-q", "-b", "main"]);
            run_git(&path, &["config", "user.email", "t@t"]);
            run_git(&path, &["config", "user.name", "t"]);
            Self { _dir: dir, path }
        }

        fn commit_tree<I, P, C>(&self, files: I, message: &str) -> String
        where
            I: IntoIterator<Item = (P, C)>,
            P: AsRef<std::path::Path>,
            C: AsRef<str>,
        {
            let entries: Vec<_> = std::fs::read_dir(&self.path)
                .expect("readdir")
                .filter_map(|e| e.ok())
                .filter(|e| e.file_name() != ".git")
                .collect();
            for entry in entries {
                let path = entry.path();
                if path.is_dir() {
                    std::fs::remove_dir_all(&path).expect("rmdir");
                } else {
                    std::fs::remove_file(&path).expect("rm");
                }
            }
            for (rel, contents) in files {
                let full = self.path.join(rel.as_ref());
                if let Some(parent) = full.parent() {
                    std::fs::create_dir_all(parent).expect("mkdir");
                }
                std::fs::write(&full, contents.as_ref()).expect("write");
            }
            run_git(&self.path, &["add", "-A"]);
            run_git(&self.path, &["commit", "-q", "--allow-empty", "-m", message]);
            run_git(&self.path, &["rev-parse", "HEAD^{tree}"])
        }
    }

    fn run_git(repo: &std::path::Path, args: &[&str]) -> String {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(args)
            .output()
            .expect("git");
        assert!(
            out.status.success(),
            "git {:?}: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    #[tokio::test]
    async fn reference_pair_marks_slice_and_leftover_edges() {
        let repo = TestRepo::new();
        let base = repo.commit_tree([("a.txt", "a\n"), ("b.txt", "b\n")], "base");
        let slice = repo.commit_tree([("a.txt", "a\n"), ("b.txt", "x\n")], "slice");
        let final_tree = repo.commit_tree([("a.txt", "z\n"), ("b.txt", "x\n")], "final");

        let slice_edge =
            compute(&repo.path, &base, &slice, &base, &final_tree).await.expect("slice edge");
        assert!(
            slice_edge.parent.is_empty(),
            "slice edge: parent == beforeRef → no parent marks"
        );
        let child_paths: Vec<&str> = slice_edge.child.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(
            child_paths,
            vec!["a.txt"],
            "slice edge: child differs from after_ref on a.txt only"
        );

        let leftover_edge =
            compute(&repo.path, &slice, &final_tree, &base, &final_tree).await.expect("leftover");
        assert!(leftover_edge.child.is_empty());
        let parent_paths: Vec<&str> = leftover_edge.parent.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(parent_paths, vec!["b.txt"]);
    }
}
