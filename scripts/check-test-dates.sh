#!/usr/bin/env bash
# Flags hardcoded calendar dates in test string literals. A test that embeds
# a literal date (e.g. asserting "as of 2026-03-01") silently rots the day
# that date is no longer "today" or "recent" relative to whatever the test
# implicitly assumes — CLAUDE.md's "never hardcode timestamps in tests" rule
# exists for exactly this. Prefer relative-time helpers (e.g. `now()`,
# `Utc::now() - Duration::days(n)`, `faker`-generated dates) over literals.
#
# Deliberately parser-free: this only ever looks at individual lines, never
# tracks whether a line is inside an open string/comment/block. Earlier
# versions tried to track that (Swift `"""` blocks, backtick template
# literals, and — the one that actually broke something — Rust's plain
# `"..."` strings, which can legally contain a literal newline) and kept
# growing new special cases. One of them appended an annotation *inside* a
# multi-line SQL string, which reached Postgres as a literal `//` and broke
# a test at runtime. Not worth it for a lint. The rule now:
#
# Exemptions:
#   - paths under pact/contracts/, tests/fixtures/, or db/migrations/ (fixed
#     recorded fixtures / historical schema, not test assertions)
#   - a standalone `// date-ok` (or `# date-ok`) comment line — nothing else
#     on that line — appearing on any of the 3 lines immediately above the
#     flagged line. No trailing/same-line annotation is supported: a
#     same-line comment can land inside a multi-line string literal and
#     corrupt it, which is exactly what happened above. For a multi-line
#     fixture (JSON payload, SQL query, ...), put the comment above the
#     statement that opens it, close enough that the date-bearing line
#     falls within the 3-line lookback.
#   Judged case by case: prefer fixing the test to use a relative-time
#   helper; only annotate when the literal date is genuinely the fixed
#   subject under test (e.g. a deterministic scheduler fixture).
set -euo pipefail

cd "$(dirname "$0")/.."

SEARCH_DIRS=(backend/api/tests web/tests ios/OwnPulseTests)
EXEMPT_PATH_RE='(^|/)(pact/contracts|tests/fixtures|db/migrations)/'

existing_dirs=()
for d in "${SEARCH_DIRS[@]}"; do
  [[ -d "$d" ]] && existing_dirs+=("$d")
done

if [[ ${#existing_dirs[@]} -eq 0 ]]; then
  echo "check-test-dates.sh: none of the search directories exist, nothing to check"
  exit 0
fi

total_failures=0
while IFS= read -r file; do
  [[ -z "$file" ]] && continue
  [[ "$file" =~ $EXEMPT_PATH_RE ]] && continue

  out="$(awk '
    function is_date_line(s) {
      return (s ~ /["\x27`][^"\x27`]*[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9][^"\x27`]*["\x27`]/)
    }
    function is_bare_date_ok(s) {
      return (s ~ /^[ \t]*(\/\/|#)[ \t]*date-ok[ \t]*$/)
    }
    { lines[NR] = $0; last = NR }
    END {
      for (i = 1; i <= last; i++) {
        cur = lines[i]
        if (!is_date_line(cur)) { continue }

        exempt = 0
        for (back = 1; back <= 3; back++) {
          j = i - back
          if (j < 1) { break }
          if (is_bare_date_ok(lines[j])) { exempt = 1; break }
        }
        if (exempt) { continue }

        is_comment_line = (cur ~ /^[ \t]*\/\//)
        if (is_comment_line) { continue }

        printf "%s:%d: hardcoded date literal - %s\n", FILENAME, i, cur
        failures++
      }
      exit (failures > 0) ? 1 : 0
    }
  ' "$file")" && rc=0 || rc=$?

  if [[ -n "$out" ]]; then
    echo "$out"
    total_failures=$((total_failures + $(echo "$out" | wc -l)))
  fi
done < <(grep -rlE '[0-9]{4}-[0-9]{2}-[0-9]{2}' "${existing_dirs[@]}" --include='*.rs' --include='*.ts' --include='*.tsx' --include='*.swift' 2>/dev/null || true)

if [[ $total_failures -gt 0 ]]; then
  echo
  echo "check-test-dates.sh: found $total_failures hardcoded date literal(s) in test files."
  echo "Use a relative-time helper instead (now(), Duration-based offsets, faker-generated"
  echo "dates), or put a standalone '// date-ok' comment on one of the 3 lines above it."
  exit 1
fi

echo "check-test-dates.sh: no hardcoded date literals found"
