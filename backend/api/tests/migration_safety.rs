// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Migration-safety CI gate.
//!
//! Scans every `db/migrations/*.sql` file for destructive schema operations that
//! break zero-downtime (rolling) deploys, where old and new application code run
//! against the same database simultaneously:
//!
//! - `DROP TABLE`  — removes a table the old code may still query.
//! - `DROP COLUMN` — removes a column the old code may still read or write.
//! - `ALTER COLUMN ... TYPE` (and `SET DATA TYPE`) — rewrites a column's type,
//!   which can break the old code's reads/writes and may take a table-rewrite lock.
//!
//! The test FAILS if any of these are found. These migrations are not strictly
//! forbidden forever, but they must be done with an expand/contract pattern across
//! multiple deploys rather than a single destructive migration — so this gate
//! exists to force a deliberate decision, not a silent break.
//!
//! Non-destructive operations that share keywords are explicitly allowed and must
//! not trip the detector: `CREATE TABLE`, `ADD COLUMN`, `DROP CONSTRAINT`,
//! `DROP INDEX`, `DROP NOT NULL`, `DROP DEFAULT`, and
//! `ALTER COLUMN ... SET/DROP NOT NULL|DEFAULT`.

use std::path::{Path, PathBuf};

/// A destructive statement found in a migration file.
#[derive(Debug, PartialEq, Eq)]
struct Finding {
    kind: &'static str,
    line: usize,
    text: String,
}

/// Locate the `db/migrations` directory relative to this crate's manifest.
fn migrations_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../db/migrations")
}

/// Strip SQL comments and the *contents* of string literals and dollar-quoted
/// bodies from a source string so none of them can cause false positives or hide
/// destructive statements.
///
/// Handles, in order of precedence at the top level:
/// - `--` line comments (removed; the trailing newline is preserved for line nums)
/// - `/* ... */` block comments (replaced by a single space, because Postgres
///   treats a block comment as whitespace — `DROP/* x */TABLE` is `DROP TABLE`)
/// - `'...'` single-quoted string literals (inner text blanked, quotes kept) with
///   `''` treated as an embedded quote, so a keyword like `drop table` inside a
///   default value (`DEFAULT 'drop table joke'`) does not trip the detector
/// - `$tag$ ... $tag$` dollar-quoted bodies (inner text blanked, delimiters kept):
///   their contents are opaque — apostrophes inside them must not flip string
///   mode, and `;` inside them must not split statements. This matters for
///   `DO $$ ... $$;` blocks (e.g. migration `0024_protocol_runs.sql`).
///
/// We do not need a full SQL parser here — migrations are authored by us — but
/// respecting comments, strings, and dollar quotes removes the false-positive and
/// false-negative sources an adversary (or a careless future author) could hit.
fn strip_comments(sql: &str) -> String {
    let bytes = sql.as_bytes();
    let mut out = String::with_capacity(sql.len());
    let mut i = 0;
    let mut in_string = false;
    let mut in_line_comment = false;
    let mut in_block_comment = false;
    // When inside a dollar-quoted body, this holds the closing tag (e.g. "$$" or
    // "$tag$"). `None` means we are not inside a dollar-quoted body.
    let mut dollar_tag: Option<String> = None;

    while i < bytes.len() {
        let c = bytes[i] as char;
        let next = bytes.get(i + 1).map(|b| *b as char);

        if in_line_comment {
            if c == '\n' {
                in_line_comment = false;
                out.push(c);
            }
            i += 1;
            continue;
        }

        if in_block_comment {
            if c == '*' && next == Some('/') {
                in_block_comment = false;
                // A block comment is whitespace: emit a space so it cannot glue
                // adjacent tokens together (`DROP/* */TABLE` -> `DROP TABLE`).
                out.push(' ');
                i += 2;
                continue;
            }
            // Preserve newlines inside block comments so line numbers stay accurate.
            if c == '\n' {
                out.push(c);
            }
            i += 1;
            continue;
        }

        if let Some(tag) = &dollar_tag {
            // Inside a dollar-quoted body: contents are opaque. Only the matching
            // closing tag ends it. Apostrophes and semicolons here are inert.
            if sql[i..].starts_with(tag.as_str()) {
                out.push_str(tag);
                i += tag.len();
                dollar_tag = None;
                continue;
            }
            out.push(if c == '\n' { '\n' } else { ' ' });
            i += 1;
            continue;
        }

        if in_string {
            // Handle escaped single quote: '' inside a string literal stays inside
            // the literal, so blank both quotes' content but keep length parity.
            if c == '\'' {
                if next == Some('\'') {
                    out.push(' ');
                    out.push(' ');
                    i += 2;
                    continue;
                }
                // Closing quote: preserve it so structure is intact.
                out.push('\'');
                in_string = false;
                i += 1;
                continue;
            }
            // Replace literal content with a space (preserve newlines for line nums).
            out.push(if c == '\n' { '\n' } else { ' ' });
            i += 1;
            continue;
        }

        // Not in any comment, string, or dollar-quoted body.
        if let Some(tag) = match_dollar_tag(&sql[i..]) {
            // Opening of a dollar-quoted body. Emit the delimiter and switch modes.
            out.push_str(&tag);
            i += tag.len();
            dollar_tag = Some(tag);
            continue;
        }
        if c == '\'' {
            in_string = true;
            out.push(c);
            i += 1;
            continue;
        }
        if c == '-' && next == Some('-') {
            in_line_comment = true;
            i += 2;
            continue;
        }
        if c == '/' && next == Some('*') {
            in_block_comment = true;
            i += 2;
            continue;
        }

        out.push(c);
        i += 1;
    }

    out
}

/// If `s` begins with a Postgres dollar-quote tag (`$$` or `$tag$`), return the
/// full tag including both `$` delimiters (e.g. `"$$"`, `"$body$"`). The tag name,
/// if present, must be a valid identifier: it starts with a letter or underscore
/// and continues with letters, digits, or underscores. Returns `None` otherwise.
fn match_dollar_tag(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    if bytes.first() != Some(&b'$') {
        return None;
    }
    let mut j = 1;
    while j < bytes.len() {
        let b = bytes[j];
        if b == b'$' {
            // Closing delimiter of the tag found at position j.
            return Some(s[..=j].to_string());
        }
        let is_ident = b == b'_' || b.is_ascii_alphabetic() || (j > 1 && b.is_ascii_digit());
        if !is_ident {
            return None;
        }
        j += 1;
    }
    None
}

/// Collapse all runs of ASCII whitespace into single spaces. This lets the
/// detector match statements that span multiple lines or use irregular spacing.
fn normalize_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Scan one migration's (comment-stripped) SQL for destructive operations.
///
/// Statements are split on `;`. Each statement is lowercased and whitespace-
/// normalized before matching so that case and formatting do not matter. The line
/// number reported is the line on which the statement *starts* in the original
/// (comment-stripped) text.
fn scan(sql_no_comments: &str) -> Vec<Finding> {
    let mut findings = Vec::new();

    // Split into statements on `;`, tracking each statement's starting byte offset
    // so we can report a line number.
    let mut stmt_start = 0usize;
    let mut statements: Vec<(usize, &str)> = Vec::new();
    for (idx, ch) in sql_no_comments.char_indices() {
        if ch == ';' {
            statements.push((stmt_start, &sql_no_comments[stmt_start..idx]));
            stmt_start = idx + ch.len_utf8();
        }
    }
    // Trailing statement without a terminating semicolon.
    if stmt_start < sql_no_comments.len() {
        statements.push((stmt_start, &sql_no_comments[stmt_start..]));
    }

    for (offset, raw_stmt) in statements {
        let normalized = normalize_whitespace(raw_stmt).to_ascii_lowercase();
        if normalized.is_empty() {
            continue;
        }

        if let Some(kind) = classify(&normalized) {
            // Skip leading whitespace so the reported line is where the statement
            // text actually begins, not the trailing `;` of the previous one.
            let leading_ws = raw_stmt.len() - raw_stmt.trim_start().len();
            let line = line_of(sql_no_comments, offset + leading_ws);
            let snippet: String = normalize_whitespace(raw_stmt).chars().take(120).collect();
            findings.push(Finding {
                kind,
                line,
                text: snippet,
            });
        }
    }

    findings
}

/// Classify a normalized (lowercased, single-spaced) statement. Returns the kind
/// of destructive operation, or `None` if the statement is safe.
fn classify(stmt: &str) -> Option<&'static str> {
    // DROP TABLE (optionally IF EXISTS). Must be a real table drop, not
    // DROP CONSTRAINT / DROP INDEX / DROP NOT NULL etc.
    if stmt.contains("drop table") {
        return Some("DROP TABLE");
    }

    // DROP COLUMN (optionally IF EXISTS). Distinct from DROP CONSTRAINT,
    // DROP INDEX, DROP NOT NULL, DROP DEFAULT.
    if stmt.contains("drop column") {
        return Some("DROP COLUMN");
    }

    // ALTER COLUMN ... TYPE  /  ALTER COLUMN ... SET DATA TYPE.
    // We only flag a type change, not SET/DROP NOT NULL|DEFAULT.
    // A single statement may contain multiple `alter column` clauses; flag if any
    // one of them is a type change.
    if stmt.contains("alter column") {
        for segment in stmt.split("alter column").skip(1) {
            if segment_changes_type(segment) {
                return Some("ALTER COLUMN ... TYPE");
            }
        }
    }

    None
}

/// Given the text immediately following an `alter column` keyword (lowercased,
/// single-spaced), determine whether that clause is a column type change.
///
/// Postgres spellings:
///   ALTER COLUMN col TYPE newtype
///   ALTER COLUMN col SET DATA TYPE newtype
///
/// Safe forms we must NOT flag:
///   ALTER COLUMN col SET NOT NULL
///   ALTER COLUMN col DROP NOT NULL
///   ALTER COLUMN col SET DEFAULT ...
///   ALTER COLUMN col DROP DEFAULT
fn segment_changes_type(segment: &str) -> bool {
    // A statement separates ALTER actions with commas. Look only at this clause,
    // up to the next comma, so a later clause is handled by its own segment.
    let segment = segment.split(',').next().unwrap_or(segment);
    let tokens: Vec<&str> = segment.split_whitespace().collect();

    // Token 0 is the column name. The type-change keyword must appear in the
    // action position, anchored to the column name — NOT anywhere the word
    // "type" happens to occur (e.g. `SET DEFAULT type 'a'` or a cast `'x'::type`).
    //
    //   <col> TYPE ...            -> tokens[1] == "type"
    //   <col> SET DATA TYPE ...   -> tokens[1..4] == ["set", "data", "type"]
    matches!(
        tokens.as_slice(),
        [_col, "type", ..] | [_col, "set", "data", "type", ..]
    )
}

/// 1-based line number containing the given byte offset.
fn line_of(text: &str, offset: usize) -> usize {
    text[..offset.min(text.len())]
        .bytes()
        .filter(|&b| b == b'\n')
        .count()
        + 1
}

/// Read every `*.sql` file in the migrations directory, sorted by name.
fn read_migrations() -> Vec<(String, String)> {
    let dir = migrations_dir();
    let mut entries: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", dir.display()))
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("sql"))
        .collect();
    entries.sort();

    entries
        .into_iter()
        .map(|p| {
            let name = p
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("<unknown>")
                .to_string();
            let body = std::fs::read_to_string(&p)
                .unwrap_or_else(|e| panic!("failed to read {}: {e}", p.display()));
            (name, body)
        })
        .collect()
}

/// The gate itself: no destructive operations may exist in any migration.
#[test]
fn migrations_are_non_destructive() {
    let migrations = read_migrations();
    assert!(
        !migrations.is_empty(),
        "no migration files found in {}",
        migrations_dir().display()
    );

    let mut all_findings: Vec<String> = Vec::new();
    for (name, body) in &migrations {
        let stripped = strip_comments(body);
        for f in scan(&stripped) {
            all_findings.push(format!("  {name}:{} — {} in: {}", f.line, f.kind, f.text));
        }
    }

    assert!(
        all_findings.is_empty(),
        "Destructive migration operations break zero-downtime deploys and must use \
         an expand/contract pattern instead. Found:\n{}",
        all_findings.join("\n")
    );
}

#[cfg(test)]
mod detector_tests {
    use super::*;

    fn scan_str(sql: &str) -> Vec<Finding> {
        scan(&strip_comments(sql))
    }

    fn kinds(sql: &str) -> Vec<&'static str> {
        scan_str(sql).into_iter().map(|f| f.kind).collect()
    }

    // ---- Positive cases: must be detected ----

    #[test]
    fn detects_drop_table() {
        assert_eq!(kinds("DROP TABLE foo;"), vec!["DROP TABLE"]);
    }

    #[test]
    fn detects_drop_table_if_exists() {
        assert_eq!(kinds("DROP TABLE IF EXISTS foo;"), vec!["DROP TABLE"]);
    }

    #[test]
    fn detects_drop_table_case_insensitive() {
        assert_eq!(kinds("dRoP   TaBlE   foo ;"), vec!["DROP TABLE"]);
    }

    #[test]
    fn detects_drop_column() {
        assert_eq!(
            kinds("ALTER TABLE foo DROP COLUMN bar;"),
            vec!["DROP COLUMN"]
        );
    }

    #[test]
    fn detects_drop_column_if_exists() {
        assert_eq!(
            kinds("ALTER TABLE foo DROP COLUMN IF EXISTS bar;"),
            vec!["DROP COLUMN"]
        );
    }

    #[test]
    fn detects_alter_column_type() {
        assert_eq!(
            kinds("ALTER TABLE foo ALTER COLUMN bar TYPE bigint;"),
            vec!["ALTER COLUMN ... TYPE"]
        );
    }

    #[test]
    fn detects_alter_column_set_data_type() {
        assert_eq!(
            kinds("ALTER TABLE foo ALTER COLUMN bar SET DATA TYPE bigint;"),
            vec!["ALTER COLUMN ... TYPE"]
        );
    }

    #[test]
    fn detects_multiline_statement() {
        let sql = "ALTER TABLE foo\n    ALTER COLUMN bar\n    TYPE bigint;";
        assert_eq!(kinds(sql), vec!["ALTER COLUMN ... TYPE"]);
    }

    #[test]
    fn detects_type_change_in_second_clause() {
        // First clause is safe (SET NOT NULL), second is a type change.
        let sql = "ALTER TABLE foo ALTER COLUMN a SET NOT NULL, ALTER COLUMN b TYPE text;";
        assert_eq!(kinds(sql), vec!["ALTER COLUMN ... TYPE"]);
    }

    #[test]
    fn detects_destructive_op_when_comment_precedes() {
        let sql = "-- this drops a table\nDROP TABLE foo;";
        assert_eq!(kinds(sql), vec!["DROP TABLE"]);
    }

    #[test]
    fn detects_drop_table_split_by_block_comment() {
        // A block comment glues tokens only if treated as nothing; Postgres treats
        // it as whitespace, so this still drops the table.
        assert_eq!(kinds("DROP/* x */TABLE users;"), vec!["DROP TABLE"]);
    }

    #[test]
    fn detects_alter_type_split_by_block_comment() {
        assert_eq!(
            kinds("ALTER TABLE t ALTER COLUMN v TYPE/* */bigint;"),
            vec!["ALTER COLUMN ... TYPE"]
        );
        assert_eq!(
            kinds("ALTER TABLE t ALTER COLUMN v SET DATA/* */TYPE bigint;"),
            vec!["ALTER COLUMN ... TYPE"]
        );
    }

    #[test]
    fn detects_drop_table_after_dollar_quoted_block_with_apostrophe() {
        // The apostrophe in `don't` lives inside a dollar-quoted body and must not
        // open a string literal that would blank the following real statement.
        let sql = "DO $$ BEGIN PERFORM $x$don't$x$; END $$;\n\nDROP TABLE users;";
        assert_eq!(kinds(sql), vec!["DROP TABLE"]);
    }

    #[test]
    fn dynamic_ddl_inside_dollar_quoted_body_is_a_known_limitation() {
        // KNOWN LIMITATION: destructive DDL constructed as a runtime string inside a
        // dollar-quoted body (dynamic SQL via EXECUTE) is opaque to this static
        // scanner and is NOT flagged. Detecting it would require evaluating
        // runtime-built strings. This is a deliberate evasion vector, not an
        // accidental authoring mistake, and is out of scope for the gate. The
        // important guarantee — that the dollar-quoted body cannot HIDE a following
        // real top-level statement — is covered by the test above.
        let sql = "DO $$ BEGIN EXECUTE 'DROP TABLE users'; END $$;";
        assert!(kinds(sql).is_empty());
    }

    // ---- Negative cases: must NOT be detected ----

    #[test]
    fn allows_create_table() {
        assert!(kinds("CREATE TABLE foo (id uuid);").is_empty());
    }

    #[test]
    fn allows_add_column() {
        assert!(kinds("ALTER TABLE foo ADD COLUMN bar TEXT;").is_empty());
    }

    #[test]
    fn allows_drop_constraint() {
        assert!(kinds("ALTER TABLE foo DROP CONSTRAINT foo_key;").is_empty());
    }

    #[test]
    fn allows_drop_constraint_if_exists() {
        assert!(kinds("ALTER TABLE foo DROP CONSTRAINT IF EXISTS foo_key;").is_empty());
    }

    #[test]
    fn allows_drop_index() {
        assert!(kinds("DROP INDEX idx_foo;").is_empty());
        assert!(kinds("DROP INDEX IF EXISTS idx_foo;").is_empty());
    }

    #[test]
    fn allows_alter_column_set_not_null() {
        assert!(kinds("ALTER TABLE foo ALTER COLUMN bar SET NOT NULL;").is_empty());
    }

    #[test]
    fn allows_alter_column_drop_not_null() {
        assert!(kinds("ALTER TABLE foo ALTER COLUMN bar DROP NOT NULL;").is_empty());
    }

    #[test]
    fn allows_alter_column_set_default() {
        assert!(kinds("ALTER TABLE foo ALTER COLUMN bar SET DEFAULT 'x';").is_empty());
    }

    #[test]
    fn allows_alter_column_drop_default() {
        assert!(kinds("ALTER TABLE foo ALTER COLUMN bar DROP DEFAULT;").is_empty());
    }

    #[test]
    fn allows_enable_row_level_security() {
        assert!(kinds("ALTER TABLE foo ENABLE ROW LEVEL SECURITY;").is_empty());
    }

    #[test]
    fn allows_set_default_containing_word_type() {
        // A bare `type` token in the DEFAULT value must not be read as a type change.
        assert!(kinds("ALTER TABLE t ALTER COLUMN status SET DEFAULT type 'a';").is_empty());
    }

    #[test]
    fn allows_cast_to_type_in_alter_column() {
        // A cast like `::type` tokenizes as `::type`, not a bare `type` keyword.
        assert!(
            kinds("ALTER TABLE t ALTER COLUMN v SET DEFAULT 0::int;").is_empty(),
            "cast in default should not be a type change"
        );
    }

    #[test]
    fn allows_column_literally_named_type() {
        // A column named `type` that is only having its nullability changed must
        // not be mistaken for a type change.
        assert!(kinds("ALTER TABLE foo ALTER COLUMN type SET NOT NULL;").is_empty());
        assert!(kinds("ALTER TABLE foo ALTER COLUMN type DROP DEFAULT;").is_empty());
    }

    #[test]
    fn detects_change_of_column_named_type() {
        // ...but actually changing the type of a column named `type` is still a
        // destructive type change.
        assert_eq!(
            kinds("ALTER TABLE foo ALTER COLUMN type TYPE bigint;"),
            vec!["ALTER COLUMN ... TYPE"]
        );
    }

    #[test]
    fn ignores_destructive_op_inside_line_comment() {
        assert!(kinds("-- DROP TABLE foo;\nCREATE TABLE bar (id uuid);").is_empty());
    }

    #[test]
    fn ignores_destructive_op_inside_block_comment() {
        let sql = "/* historical note: we used to DROP COLUMN bar here */\n\
                   ALTER TABLE foo ADD COLUMN bar TEXT;";
        assert!(kinds(sql).is_empty());
    }

    #[test]
    fn ignores_destructive_keywords_inside_string_literal() {
        // A column default containing the literal text should not trip the detector.
        let sql = "ALTER TABLE foo ADD COLUMN note TEXT NOT NULL DEFAULT 'drop table joke';";
        assert!(kinds(sql).is_empty());
    }

    #[test]
    fn reports_correct_line_number() {
        let sql = "CREATE TABLE a (id uuid);\nCREATE TABLE b (id uuid);\nDROP TABLE c;";
        let findings = scan_str(sql);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].line, 3);
    }
}
