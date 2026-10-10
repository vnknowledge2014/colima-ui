//! A Dockerfile reader narrow enough to be trustworthy.
//!
//! This is not an AST and deliberately never becomes one. It answers two
//! questions — *which lines is each instruction on* and *what does it say* —
//! and it can hand the file back byte-for-byte. That is the whole contract,
//! because the thing being edited belongs to the user: their comments, their
//! blank lines, their `\r\n` if they are on Windows, their missing final
//! newline. A patch that corrects one instruction and silently reformats the
//! other forty is not a fix.
//!
//! Byte-identical round-tripping is structural rather than careful: the file is
//! held as the exact list produced by splitting on `\n`, and rendering joins it
//! back. Carriage returns stay inside the line contents, and a trailing newline
//! survives as a final empty element. There is no code path that can lose a
//! byte, which is a stronger guarantee than a test could give.
//!
//! # Continuations
//!
//! An instruction spans further lines while each ends in a backslash. Comment
//! lines interleaved inside a continuation are Docker's rule, not an edge case
//! worth being clever about: they are skipped when joining the argument and
//! kept in the source.

/// One instruction, located in the file it came from.
#[derive(Debug, Clone)]
pub struct Instruction {
    /// Upper-cased for matching: `from`, `From` and `FROM` are one keyword.
    pub keyword: String,
    /// Everything after the keyword, with continuations joined and interleaved
    /// comments dropped. Whitespace is collapsed at the joins only.
    pub argument: String,
    /// Index of the line the keyword is on.
    pub start_line: usize,
    /// Index of the last line this instruction occupies, inclusive.
    pub end_line: usize,
}

/// A Dockerfile that can always be written back exactly as it was read.
#[derive(Debug, Clone)]
pub struct Dockerfile {
    /// The file split on `\n`; a trailing newline shows up as a final empty
    /// element, which is what makes rendering lossless.
    lines: Vec<String>,
    pub instructions: Vec<Instruction>,
}

/// True for a line that is blank or a comment once indentation is ignored.
fn is_skippable(line: &str) -> bool {
    let t = line.trim();
    t.is_empty() || t.starts_with('#')
}

/// True when this line hands the instruction on to the next one.
fn continues(line: &str) -> bool {
    // A trailing `\r` from CRLF sits after the backslash, so trim it first.
    line.trim_end_matches(['\r', ' ', '\t']).ends_with('\\')
}

impl Dockerfile {
    pub fn parse(source: &str) -> Self {
        let lines: Vec<String> = source.split('\n').map(String::from).collect();
        let mut instructions = Vec::new();

        let mut i = 0;
        while i < lines.len() {
            if is_skippable(&lines[i]) {
                i += 1;
                continue;
            }

            let start = i;
            let mut parts: Vec<String> = Vec::new();
            let (keyword, first_arg) = split_keyword(&lines[i]);

            parts.push(first_arg);
            let mut end = i;
            // A comment between continued lines does not end the continuation,
            // so whether more lines follow is tracked separately from whatever
            // the most recent line happened to be.
            let mut expecting_more = continues(&lines[i]);
            while expecting_more && end + 1 < lines.len() {
                end += 1;
                if is_skippable(&lines[end]) {
                    // Part of the span, not part of the argument.
                    continue;
                }
                parts.push(lines[end].trim().to_string());
                expecting_more = continues(&lines[end]);
            }

            let argument = parts
                .iter()
                .map(|p| p.trim_end_matches(['\r', ' ', '\t']).trim_end_matches('\\').trim())
                .filter(|p| !p.is_empty())
                .collect::<Vec<_>>()
                .join(" ");

            instructions.push(Instruction {
                keyword,
                argument,
                start_line: start,
                end_line: end,
            });
            i = end + 1;
        }

        Self {
            lines,
            instructions,
        }
    }

    /// Render the file. Identical to the input unless a mutator was called.
    pub fn to_source(&self) -> String {
        self.lines.join("\n")
    }

    pub fn line(&self, index: usize) -> Option<&str> {
        self.lines.get(index).map(|s| s.as_str())
    }

    /// Replace one line, leaving every other byte untouched.
    pub fn replace_line(&mut self, index: usize, replacement: &str) {
        if index < self.lines.len() {
            self.lines[index] = replacement.to_string();
            self.instructions = Self::parse(&self.to_source()).instructions;
        }
    }

    /// Insert lines directly after `index`.
    ///
    /// Used to append an instruction at the end of a stage rather than the end
    /// of the file, which is the difference between a `USER` that applies and
    /// one that lands after the wrong `FROM`.
    pub fn insert_after(&mut self, index: usize, new_lines: &[String]) {
        let at = (index + 1).min(self.lines.len());
        for (offset, line) in new_lines.iter().enumerate() {
            self.lines.insert(at + offset, line.clone());
        }
        self.instructions = Self::parse(&self.to_source()).instructions;
    }

    /// Index of the last instruction belonging to the final build stage.
    ///
    /// Anything appended for hardening must land in the stage that produces the
    /// image; appending after an earlier `FROM` changes a builder stage nobody
    /// ships.
    pub fn last_stage_end(&self) -> Option<usize> {
        let last_from = self
            .instructions
            .iter()
            .rposition(|ins| ins.keyword == "FROM")?;
        self.instructions.last().map(|_| {
            self.instructions[last_from..]
                .iter()
                .map(|ins| ins.end_line)
                .max()
                .unwrap_or(0)
        })
    }
}

/// Split a line into its keyword and the rest.
fn split_keyword(line: &str) -> (String, String) {
    let trimmed = line.trim_start();
    match trimmed.find(char::is_whitespace) {
        Some(pos) => (
            trimmed[..pos].to_uppercase(),
            trimmed[pos..].trim_start().to_string(),
        ),
        None => (trimmed.to_uppercase(), String::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The gate for this module. Every corpus file must survive a parse and a
    /// render with not one byte changed.
    #[test]
    fn every_corpus_file_round_trips_byte_identically() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("tests/dockerfile-corpus");
        let mut checked = 0;
        for entry in std::fs::read_dir(&dir).expect("dockerfile corpus must exist") {
            let path = entry.unwrap().path();
            if !path.is_file() {
                continue;
            }
            let source = std::fs::read_to_string(&path).unwrap();
            let rendered = Dockerfile::parse(&source).to_source();
            assert_eq!(
                rendered,
                source,
                "round trip changed {:?}",
                path.file_name().unwrap()
            );
            checked += 1;
        }
        assert!(checked >= 20, "corpus has only {} files", checked);
    }

    #[test]
    fn a_file_without_a_trailing_newline_keeps_not_having_one() {
        let src = "FROM alpine\nRUN true";
        assert_eq!(Dockerfile::parse(src).to_source(), src);
    }

    #[test]
    fn crlf_survives() {
        let src = "FROM alpine\r\nRUN true\r\n";
        assert_eq!(Dockerfile::parse(src).to_source(), src);
    }

    #[test]
    fn continuations_join_into_one_instruction() {
        let src = "RUN apt-get update \\\n    && apt-get install -y curl\n";
        let df = Dockerfile::parse(src);
        assert_eq!(df.instructions.len(), 1);
        assert_eq!(df.instructions[0].keyword, "RUN");
        assert_eq!(
            df.instructions[0].argument,
            "apt-get update && apt-get install -y curl"
        );
        assert_eq!(df.instructions[0].end_line, 1);
    }

    #[test]
    fn a_comment_inside_a_continuation_is_not_part_of_the_argument() {
        let src = "RUN echo a \\\n# explain\n    && echo b\n";
        let df = Dockerfile::parse(src);
        assert_eq!(df.instructions.len(), 1);
        assert_eq!(df.instructions[0].argument, "echo a && echo b");
        assert_eq!(df.instructions[0].end_line, 2);
    }

    #[test]
    fn keywords_are_matched_regardless_of_case() {
        let df = Dockerfile::parse("from alpine\nUser app\n");
        let keywords: Vec<&str> = df.instructions.iter().map(|i| i.keyword.as_str()).collect();
        assert_eq!(keywords, vec!["FROM", "USER"]);
    }

    #[test]
    fn last_stage_end_points_past_the_final_from() {
        let src = "FROM golang AS build\nRUN go build\n\nFROM alpine\nCOPY --from=build /app /app\n";
        let df = Dockerfile::parse(src);
        // The COPY is on line 4, and that is the end of the shipped stage.
        assert_eq!(df.last_stage_end(), Some(4));
    }

    #[test]
    fn inserting_after_a_line_leaves_the_rest_alone() {
        let src = "# header\nFROM alpine\nRUN true\n";
        let mut df = Dockerfile::parse(src);
        df.insert_after(2, &["USER app".to_string()]);
        assert_eq!(df.to_source(), "# header\nFROM alpine\nRUN true\nUSER app\n");
    }

    #[test]
    fn replacing_a_line_reparses_the_instructions() {
        let mut df = Dockerfile::parse("FROM alpine\nADD . /app\n");
        df.replace_line(1, "COPY . /app");
        assert_eq!(df.instructions[1].keyword, "COPY");
        assert_eq!(df.to_source(), "FROM alpine\nCOPY . /app\n");
    }
}
