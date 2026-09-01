#!/usr/bin/env bash
# Flags hardcoded calendar dates in test string literals. A test that embeds
# a literal date (e.g. asserting "as of 2026-03-01") silently rots the day
# that date is no longer "today" or "recent" relative to whatever the test
# implicitly assumes — CLAUDE.md's "never hardcode timestamps in tests" rule
# exists for exactly this. Prefer relative-time helpers (e.g. `now()`,
# `Utc::now() - Duration::days(n)`, `faker`-generated dates) over literals.
#
# Exemptions:
#   - paths under pact/contracts/, tests/fixtures/, or db/migrations/ (fixed
#     recorded fixtures / historical schema, not test assertions)
#   - a line annotated with a trailing `// date-ok` comment
#   - a multi-line string/JSON fixture block (opened by a bare `"""` or a
#     backtick template literal), where the line immediately *before* the
#     block starts is a standalone `// date-ok` comment — a trailing comment
#     can't be placed inside the literal itself without corrupting it, so
#     the annotation goes above
#   Both forms are judged case by case: prefer fixing the test to use a
#   relative-time helper; only annotate when the literal date is genuinely
#   the fixed subject under test (e.g. a deterministic scheduler fixture).
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
    {
      cur = $0
      cur_ok = (cur ~ /date-ok/)
      tmp = cur
      triple = gsub(/"""/, "\"\"\"", tmp)
      tmp2 = cur
      backtick = gsub(/`/, "`", tmp2)
      opens_or_closes = (triple % 2 == 1) || (backtick % 2 == 1)

      if (in_block) {
        if (is_date_line(cur) && !block_exempt) {
          printf "%s:%d: hardcoded date literal (inside fixture block) - %s\n", FILENAME, FNR, cur
          failures++
        }
        if (opens_or_closes) { in_block = 0 }
        prev_ok = cur_ok
        next
      }

      if (opens_or_closes) {
        in_block = 1
        block_exempt = prev_ok
        if (is_date_line(cur) && !block_exempt && !cur_ok) {
          printf "%s:%d: hardcoded date literal (fixture block opens here) - %s\n", FILENAME, FNR, cur
          failures++
        }
        prev_ok = cur_ok
        next
      }

      is_comment_line = (cur ~ /^[ \t]*\/\//)
      if (is_date_line(cur) && !cur_ok && !is_comment_line) {
        printf "%s:%d: hardcoded date literal - %s\n", FILENAME, FNR, cur
        failures++
      }
      prev_ok = cur_ok
    }
    END { exit (failures > 0) ? 1 : 0 }
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
  echo "dates), annotate the line with a trailing '// date-ok', or — for a multi-line"
  echo "fixture block — put a standalone '// date-ok' comment on the line above it."
  exit 1
fi

echo "check-test-dates.sh: no hardcoded date literals found"
