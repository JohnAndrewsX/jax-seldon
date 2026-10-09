#!/usr/bin/env bash
# The plugin as the Omarchy plugin store receives it (WP-042): the split of
# plugin/ is the jax-seldon-plugin repository, and the store's security
# baseline reads its root README (finding `curl-pipe-shell` for a
# downloader piped to a shell in a shell code block). Checked here, on
# plugin/ and on mutated copies:
#   - no command in plugin/README.md or plugin/SECURITY.md pipes curl or
#     wget into a shell or interpreter, or runs a download through
#     `<(…)` or `$(…)`. A command is a line in a fenced code block (any
#     language) or a line that starts with curl or wget (an indented code
#     block); continued lines (`\`, a trailing `|`, `&&` or `||`) join, as
#     in the baseline. A code span in prose may still name the panel's
#     one-liner (ADR-0024: the README says what *Install* runs);
#   - no agent or session files anywhere under plugin/ (AGENTS.md,
#     CLAUDE.md, HANDOFF.md, .claude/, .codex/, .agents/): store reviews
#     reject them in an installable plugin.
# Runs in `just check-packaging`; no network, no host tools.
#
# Usage: bash tests/release/store-readme.test.sh
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
plugin=$root/plugin
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
fails=0

pass() { echo "ok   $1"; }
fail() { echo "FAIL $1" >&2; fails=$((fails + 1)); }

# The commands of a Markdown file, one logical line each.
commands_of() { # file
  local line in_fence=0 fence="" text=""
  while IFS= read -r line || [[ -n $line ]]; do
    if [[ $line =~ ^[[:space:]]*(\`\`\`+|~~~+) ]]; then
      if ((in_fence == 0)); then
        in_fence=1 fence=${BASH_REMATCH[1]}
        continue
      elif [[ $line =~ ^[[:space:]]*${fence:0:1}{${#fence},}[[:space:]]*$ ]]; then
        [[ -n $text ]] && printf '%s\n' "$text"
        in_fence=0 text=""
        continue
      fi
    fi
    if ((in_fence == 0)) && [[ -z $text ]] && ! [[ $line =~ ^[[:space:]]*(\$[[:space:]]+)?(curl|wget)([[:space:]]|$) ]]; then
      continue
    fi
    text+="${text:+ }${line%\\}"
    if [[ $line =~ (\\|\||\&\&)[[:space:]]*$ ]]; then
      continue
    fi
    printf '%s\n' "$text"
    text=""
  done < "$1"
  [[ -z $text ]] || printf '%s\n' "$text"
}

# True when the first word of a pipeline stage, past sudo, doas, env,
# exec, command, nohup and timeout with their options, numbers and
# VAR=value words, is a shell or an interpreter.
runs_interpreter() { # stage
  local -a words
  read -r -a words <<< "$1"
  local i=0 word
  while ((i < ${#words[@]})); do
    word=${words[i]}
    word=${word#[\"\']}
    word=${word%[\"\']}
    case $word in
      sudo | doas | env | exec | command | nohup | timeout | -* | [0-9]* | [A-Za-z_]*=*) i=$((i + 1)) ;;
      *) break ;;
    esac
  done
  ((i < ${#words[@]})) || return 1
  word=${words[i]##*/}
  [[ $word =~ ^((ba|z|da|k|fi|a)?sh|python[0-9.]*|node|perl|ruby|php|deno)$ ]]
}

re_downloader='(^|[^[:alnum:]_.-])(curl|wget)([^[:alnum:]_.-]|$)'
re_process_sub='(sh|source|[.])[[:space:]]+([<][[:space:]]*)?[<][(][[:space:]]*(curl|wget)'
re_command_sub='(eval|sh[[:space:]]+-c)[[:space:]]+["'"'"']?[$][(][[:space:]]*(curl|wget)'

# True when a command runs downloaded content: curl or wget piped (in the
# same `;`, `&&` or `||` segment) to an interpreter, `sh <(curl …)`,
# `source <(curl …)` or `sh -c "$(curl …)"`.
downloads_to_shell() { # command
  local text=$1 segments segment stage seen
  local -a stages
  [[ $text =~ $re_downloader ]] || return 1
  [[ $text =~ $re_process_sub ]] && return 0
  [[ $text =~ $re_command_sub ]] && return 0
  segments=${text//'||'/$'\n'}
  segments=${segments//'&&'/$'\n'}
  segments=${segments//';'/$'\n'}
  segments=${segments//'|&'/'|'}
  while IFS= read -r segment; do
    IFS='|' read -r -a stages <<< "$segment"
    seen=0
    for stage in "${stages[@]}"; do
      if ((seen)) && runs_interpreter "$stage"; then
        return 0
      fi
      [[ $stage =~ $re_downloader ]] && seen=1
    done
  done <<< "$segments"
  return 1
}

# Prints each problem of a plugin tree; exit 1 when there is one.
check_tree() { # dir
  local dir=$1 doc command found=0 hit
  for doc in README.md SECURITY.md; do
    [[ -f $dir/$doc ]] || continue
    while IFS= read -r command; do
      if downloads_to_shell "$command"; then
        echo "$doc: a downloader piped to a shell: $command"
        found=1
      fi
    done < <(commands_of "$dir/$doc")
  done
  while IFS= read -r hit; do
    echo "agent file in the plugin: ${hit#"$dir"/}"
    found=1
  done < <(find "$dir" \( -type f \( -iname AGENTS.md -o -iname CLAUDE.md -o -iname HANDOFF.md \) \) \
    -o \( -type d \( -name .claude -o -name .codex -o -name .agents \) -prune \) | sort)
  return "$found"
}

# expect_clean NAME DIR / expect_problem NAME PATTERN DIR
expect_clean() {
  if check_tree "$2" > "$tmp/out"; then
    pass "$1"
  else
    fail "$1: $(cat "$tmp/out")"
  fi
}
expect_problem() {
  local rc=0
  check_tree "$3" > "$tmp/out" || rc=$?
  if [[ $rc == 1 ]] && grep -qF -- "$2" "$tmp/out"; then
    pass "$1"
  else
    fail "$1: exit $rc, output '$(cat "$tmp/out")'"
  fi
}

# mutant NAME: a fresh copy of plugin/ in $tmp/NAME, its path on stdout
mutant() {
  rm -rf "${tmp:?}/$1"
  cp -R "$plugin" "$tmp/$1"
  printf '%s\n' "$tmp/$1"
}

url=https://github.com/JohnAndrewsX/jax-seldon/releases/latest/download/install.sh

expect_clean "plugin/ as the store receives it" "$plugin"

# The README's own commands are found (the test does read the fences).
commands_of "$plugin/README.md" > "$tmp/commands"
if grep -qF "sha256sum -c --ignore-missing SHA256SUMS && bash install.sh" "$tmp/commands"; then
  pass "the README's verify-then-run line is read as a command"
else
  fail "the README's verify-then-run line is not read as a command"
fi

# Mutants: each must fail.
m=$(mutant old-one-liner)
printf '\nor in one line:\n\n```sh\ncurl -fsSL %s | bash\n```\n' "$url" >> "$m/README.md"
expect_problem "the removed one-liner fence" "README.md: a downloader piped to a shell: curl -fsSL" "$m"

m=$(mutant old-uninstall)
printf '\n```sh\ncurl -fsSL %s | bash -s -- --uninstall\n```\n' "$url" >> "$m/README.md"
expect_problem "the removed uninstall fence" "| bash -s -- --uninstall" "$m"

m=$(mutant wget-sudo)
printf '\n```bash\nwget -qO- %s | sudo -E /usr/bin/bash\n```\n' "$url" >> "$m/README.md"
expect_problem "wget into sudo bash" "wget -qO-" "$m"

m=$(mutant unlabelled-fence)
printf '\n~~~\ncurl -fsSL %s | sh\n~~~\n' "$url" >> "$m/README.md"
expect_problem "a fence without a language" "| sh" "$m"

m=$(mutant continued)
printf '\n```sh\ncurl -fsSL %s \\\n  | bash\n```\n' "$url" >> "$m/README.md"
expect_problem "a pipe on a continued line" "| bash" "$m"

m=$(mutant trailing-pipe)
printf '\n```sh\ncurl -fsSL %s |\n  env FOO=1 python3 -\n```\n' "$url" >> "$m/README.md"
expect_problem "a trailing pipe into python" "python3" "$m"

m=$(mutant process-substitution)
printf '\n```sh\nbash <(curl -fsSL %s)\n```\n' "$url" >> "$m/README.md"
expect_problem "process substitution" "bash <(curl" "$m"

m=$(mutant command-substitution)
printf '\n```sh\nsh -c "$(curl -fsSL %s)"\n```\n' "$url" >> "$m/README.md"
expect_problem "command substitution" 'sh -c "$(curl' "$m"

m=$(mutant indented)
printf '\nInstall:\n\n    curl -fsSL %s | zsh\n' "$url" >> "$m/README.md"
expect_problem "an indented code line" "| zsh" "$m"

m=$(mutant list-fence)
printf '\n- Install:\n\n  ```sh\n  curl -fsSL %s | bash\n  ```\n' "$url" >> "$m/README.md"
expect_problem "a fence inside a list item" "| bash" "$m"

m=$(mutant security-md)
printf '\n```sh\ncurl -fsSL %s | bash\n```\n' "$url" >> "$m/SECURITY.md"
expect_problem "plugin/SECURITY.md" "SECURITY.md: a downloader piped to a shell" "$m"

m=$(mutant agents-md)
echo "# rules" > "$m/AGENTS.md"
expect_problem "AGENTS.md" "agent file in the plugin: AGENTS.md" "$m"

m=$(mutant claude-md)
echo "# notes" > "$m/components/claude.md"
expect_problem "claude.md in a subfolder" "agent file in the plugin: components/claude.md" "$m"

m=$(mutant handoff-md)
echo "# handoff" > "$m/HANDOFF.md"
expect_problem "HANDOFF.md" "agent file in the plugin: HANDOFF.md" "$m"

m=$(mutant claude-dir)
mkdir -p "$m/.claude"
echo '{}' > "$m/.claude/settings.json"
expect_problem ".claude/" "agent file in the plugin: .claude" "$m"

m=$(mutant codex-dir)
mkdir -p "$m/sections/.codex"
expect_problem ".codex/, even empty" "agent file in the plugin: sections/.codex" "$m"

m=$(mutant agents-dir)
mkdir -p "$m/.agents/skills"
expect_problem ".agents/" "agent file in the plugin: .agents" "$m"

# Controls: each must pass.
m=$(mutant prose-span)
printf '\nThe panel runs `curl -fsSL %s | bash` in a terminal you see.\n' "$url" >> "$m/README.md"
expect_clean "a code span in prose names the one-liner" "$m"

m=$(mutant pipe-to-tools)
printf '\n```sh\ncurl -fsSL %s | sha256sum\ncurl -fsSL %s | less\ncurl -fsSLO %s && bash -n install.sh\n```\n' "$url" "$url" "$url" >> "$m/README.md"
expect_clean "curl piped to sha256sum or less, a download then a file" "$m"

m=$(mutant other-pipe)
printf '\n```sh\ncurl -fsSLO %s; echo ok | bash\n```\n' "$url" >> "$m/README.md"
expect_clean "a later pipe after a finished download" "$m"

if ((fails > 0)); then
  echo "store-readme.test: $fails failed" >&2
  exit 1
fi
echo "store-readme.test: ok"
