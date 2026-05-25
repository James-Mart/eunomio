// SPDX-License-Identifier: Apache-2.0

use anyhow::{anyhow, Result};
use eunomio_core::{FileLineRanges, LineRanges, SynthesizedRanges};
use std::collections::BTreeMap;
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
        parse_side(repo, &raw, child_tree).await?
    };

    let parent = if parent_tree == before_ref {
        Vec::new()
    } else {
        let raw = run_word_diff(repo, parent_tree, before_ref).await?;
        parse_side(repo, &raw, parent_tree).await?
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

async fn parse_side(
    repo: &Path,
    raw: &str,
    side_tree: &str,
) -> Result<Vec<FileLineRanges>> {
    let mut files = Vec::new();
    for tokenized in tokenize_porcelain(raw) {
        let Some(content) = crate::git::fetch_blob_text(repo, side_tree, &tokenized.path).await?
        else {
            continue;
        };
        let lines = anchor_file(&content, &tokenized.hunks);
        if !lines.is_empty() {
            files.push(FileLineRanges {
                path: tokenized.path,
                lines,
            });
        }
    }
    Ok(files)
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

enum OldToken {
    Context,
    Deleted(String),
}

struct TokenizedHunk {
    old_start: u32,
    tokens: Vec<OldToken>,
}

struct TokenizedFile {
    path: String,
    hunks: Vec<TokenizedHunk>,
}

fn tokenize_porcelain(raw: &str) -> Vec<TokenizedFile> {
    let mut files: Vec<TokenizedFile> = Vec::new();
    let mut current_path: Option<String> = None;
    let mut current_hunks: Vec<TokenizedHunk> = Vec::new();
    let mut binary = false;
    let mut in_hunk = false;
    let mut current_hunk: Option<TokenizedHunk> = None;

    let push_file = |files: &mut Vec<TokenizedFile>,
                     path: &mut Option<String>,
                     hunks: &mut Vec<TokenizedHunk>| {
        if let Some(p) = path.take() {
            if !hunks.is_empty() {
                files.push(TokenizedFile {
                    path: p,
                    hunks: std::mem::take(hunks),
                });
            }
        }
    };

    for line in raw.lines() {
        if line.starts_with("diff --git ") {
            if let Some(hunk) = current_hunk.take() {
                if !hunk.tokens.is_empty() {
                    current_hunks.push(hunk);
                }
            }
            push_file(&mut files, &mut current_path, &mut current_hunks);
            in_hunk = false;
            binary = false;
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
                if let Some(hunk) = current_hunk.take() {
                    if !hunk.tokens.is_empty() {
                        current_hunks.push(hunk);
                    }
                }
                current_hunk = Some(TokenizedHunk {
                    old_start: h.old_start,
                    tokens: Vec::new(),
                });
                continue;
            }
            continue;
        }

        if let Some(h) = parse_hunk_header(line) {
            if let Some(hunk) = current_hunk.take() {
                if !hunk.tokens.is_empty() {
                    current_hunks.push(hunk);
                }
            }
            current_hunk = Some(TokenizedHunk {
                old_start: h.old_start,
                tokens: Vec::new(),
            });
            continue;
        }

        if line == "~" {
            continue;
        }

        let Some(hunk) = current_hunk.as_mut() else {
            continue;
        };

        let (prefix, word) = match line.as_bytes().first() {
            Some(b' ') => (b' ', &line[1..]),
            Some(b'-') => (b'-', &line[1..]),
            Some(b'+') => (b'+', &line[1..]),
            _ => continue,
        };
        match prefix {
            b' ' => hunk.tokens.push(OldToken::Context),
            b'-' => hunk.tokens.push(OldToken::Deleted(word.to_string())),
            b'+' => {}
            _ => {}
        }
    }

    if let Some(hunk) = current_hunk.take() {
        if !hunk.tokens.is_empty() {
            current_hunks.push(hunk);
        }
    }
    push_file(&mut files, &mut current_path, &mut current_hunks);

    files
}

fn utf16_len(s: &str) -> u32 {
    s.chars().map(|c| c.len_utf16() as u32).sum()
}

fn file_lines(content: &str) -> Vec<&str> {
    if content.is_empty() {
        return vec![""];
    }
    let mut lines: Vec<&str> = content.split('\n').collect();
    if content.ends_with('\n') {
        lines.pop();
    }
    lines
}

fn utf16_to_byte_idx(s: &str, utf16_col: u32) -> usize {
    let mut pos = 0u32;
    for (byte_idx, c) in s.char_indices() {
        if pos >= utf16_col {
            return byte_idx;
        }
        pos += c.len_utf16() as u32;
    }
    s.len()
}

fn byte_idx_to_utf16(s: &str, byte_idx: usize) -> u32 {
    s[..byte_idx].chars().map(|c| c.len_utf16() as u32).sum()
}

fn find_in_line_from_col(line_text: &str, col: u32, text: &str) -> Option<u32> {
    if text.is_empty() {
        return Some(col);
    }
    let byte_start = utf16_to_byte_idx(line_text, col);
    let rel = line_text[byte_start..].find(text)?;
    Some(byte_idx_to_utf16(line_text, byte_start + rel))
}

fn find_next_at_or_after(
    lines: &[&str],
    after_line: u32,
    after_col: u32,
    text: &str,
) -> Option<(u32, u32)> {
    if text.is_empty() {
        return Some((after_line, after_col));
    }
    for (li, line_text) in lines.iter().enumerate().skip(after_line.saturating_sub(1) as usize) {
        let lnum = (li + 1) as u32;
        let start_col = if lnum == after_line { after_col } else { 0 };
        if let Some(col) = find_in_line_from_col(line_text, start_col, text) {
            return Some((lnum, col));
        }
    }
    None
}

fn anchor_file(file_text: &str, hunks: &[TokenizedHunk]) -> Vec<LineRanges> {
    let lines = file_lines(file_text);
    let mut spans_by_line: BTreeMap<u32, Vec<(u32, u32)>> = BTreeMap::new();

    for hunk in hunks {
        let mut after_line = hunk.old_start;
        let mut after_col = 0u32;

        for token in &hunk.tokens {
            let OldToken::Deleted(text) = token else {
                continue;
            };
            let Some((found_line, found_col)) =
                find_next_at_or_after(&lines, after_line, after_col, text)
            else {
                continue;
            };
            let end_col = found_col + utf16_len(text);
            spans_by_line
                .entry(found_line)
                .or_default()
                .push((found_col, end_col));
            after_line = found_line;
            after_col = end_col;
        }
    }

    spans_by_line
        .into_iter()
        .map(|(line, spans)| LineRanges { line, spans })
        .collect()
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

    fn anchor_porcelain(raw: &str, side_content: &str) -> Vec<FileLineRanges> {
        tokenize_porcelain(raw)
            .into_iter()
            .map(|tf| {
                let lines = anchor_file(side_content, &tf.hunks);
                file(&tf.path, lines)
            })
            .filter(|f| !f.lines.is_empty())
            .collect()
    }

    #[test]
    fn parses_pure_deletions_and_modifies() {
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
            anchor_porcelain(raw, "A\nB\nC\nD\nE"),
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
            anchor_porcelain(raw, "old"),
            vec![file("a.txt", vec![line(1, vec![(0, 3)])])]
        );
    }

    #[test]
    fn intra_line_word_marks() {
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
            anchor_porcelain(raw, " foo bar  baz"),
            vec![file("a.txt", vec![line(1, vec![(5, 8)])])]
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
        assert_eq!(tokenize_porcelain(raw)[0].path, "old/path.txt");
    }

    #[test]
    fn empty_for_no_changes() {
        assert!(tokenize_porcelain("").is_empty());
    }

    #[test]
    fn handles_dev_null_old_side() {
        let raw = concat!(
            "diff --git a/new.txt b/new.txt\n",
            "new file mode 100644\n",
            "--- /dev/null\n",
            "+++ b/new.txt\n",
            "@@ -0,0 +1 @@\n",
            "+hello\n",
            "~\n",
        );
        assert!(tokenize_porcelain(raw).is_empty());
    }

    #[test]
    fn merges_spans_on_same_line() {
        let raw = concat!(
            "diff --git a/a.txt b/a.txt\n",
            "--- a/a.txt\n",
            "+++ b/a.txt\n",
            "@@ -1 +1 @@\n",
            "-aa\n",
            "-bb\n",
            "~\n",
        );
        let result = anchor_porcelain(raw, "aabb");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].lines.len(), 1);
        assert_eq!(result[0].lines[0].spans, vec![(0, 2), (2, 4)]);
    }

    #[test]
    fn anchors_deletions_when_porcelain_interleaves_additions() {
        let side = concat!(
            "        use host:types/types.{error, claim};\n",
            "        use types.{proof};\n",
            "        on-user-auth-claim: func();\n",
        );
        let raw = concat!(
            "diff --git a/world.wit b/world.wit\n",
            "--- a/world.wit\n",
            "+++ b/world.wit\n",
            "@@ -1,3 +1,1 @@\n",
            "-        use host:types/types.{error, claim};\n",
            "~\n",
            "-        use types.{proof};\n",
            "+login\n",
            "~\n",
            "-        on-user-auth-claim: func();\n",
            "+done\n",
            "~\n",
        );
        let result = anchor_porcelain(raw, side);
        let lines: Vec<_> = result[0].lines.iter().map(|l| (l.line, l.spans.clone())).collect();
        assert_eq!(lines[0], (1, vec![(0, 44)]));
        assert_eq!(lines[1], (2, vec![(0, 26)]));
        assert_eq!(lines[2], (3, vec![(0, 35)]));
    }

    #[test]
    fn anchors_spans_on_structural_rewrite() {
        let side = concat!(
            "world hook-user-auth {\n",
            "    export transact-hook-user-auth: interface {\n",
            "        use host:types/types.{error, claim};\n",
            "        use types.{proof};\n",
            "    }\n",
            "}\n",
        );
        let raw = concat!(
            "diff --git a/world.wit b/world.wit\n",
            "--- a/world.wit\n",
            "+++ b/world.wit\n",
            "@@ -1,6 +1,2 @@\n",
            "-world hook-user-auth {\n",
            "~\n",
            "-    export transact-hook-user-auth: interface {\n",
            "~\n",
            "-        use host:types/types.{error, claim};\n",
            "~\n",
            "-        use types.{proof};\n",
            "+interface login {\n",
            "~\n",
            "-    }\n",
            "-}\n",
            "+}\n",
            "~\n",
        );
        let result = anchor_porcelain(raw, side);
        let proof_line = result[0]
            .lines
            .iter()
            .find(|l| {
                side.lines()
                    .nth(l.line as usize - 1)
                    .is_some_and(|text| text.contains("use types.{proof}"))
            })
            .expect("proof line marked");
        assert_eq!(
            proof_line.spans,
            vec![(0, 26)],
            "full proof import line should be marked"
        );
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

    #[tokio::test]
    async fn structural_rewrite_via_compute() {
        let repo = TestRepo::new();
        let reference = repo.commit_tree([("world.wit", "interface login {\n}\n")], "reference");
        let child = repo.commit_tree(
            [(
                "world.wit",
                concat!(
                    "world hook-user-auth {\n",
                    "    export transact-hook-user-auth: interface {\n",
                    "        use host:types/types.{error, claim};\n",
                    "        use types.{proof};\n",
                    "    }\n",
                    "}\n",
                ),
            )],
            "child",
        );

        let result = compute(&repo.path, &reference, &child, &reference, &reference)
            .await
            .expect("compute");
        assert!(result.parent.is_empty());
        let file = result
            .child
            .iter()
            .find(|f| f.path == "world.wit")
            .expect("world.wit marks");
        let proof_line = file
            .lines
            .iter()
            .find(|l| l.spans.iter().any(|(s, e)| e - s == 26))
            .expect("full proof import line marked");
        assert_eq!(proof_line.spans, vec![(0, 26)]);
    }
}
