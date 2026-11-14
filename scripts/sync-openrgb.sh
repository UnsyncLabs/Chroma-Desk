#!/usr/bin/env bash
# Keeps the OpenRGB copies in this repository current (called by .github/workflows/sync-openrgb.yml).
#
#   master, electron-openrgb   fast-forwarded to upstream master (never forced)
#   new upstream tags          mirrored
#   openrgb-razer-master       the fix rebased on upstream master      -> candidate bundle
#   openrgb-razer              the fix rebased on a newer release_X.Y  -> candidate bundle
#
# Nothing is pushed here except the fast-forward/tag mirroring, and not even that with DRY_RUN=true.
# Rebased candidates are written to $WORK/candidates.bundle for the workflow to build and publish.
#
# Env: DRY_RUN (true|false, default true), SIMULATE_CONFLICT (true|false), WORK (default /tmp/sync),
#      GITHUB_OUTPUT, GITHUB_STEP_SUMMARY (optional).
set -euo pipefail

UPSTREAM_URL="${UPSTREAM_URL:-https://gitlab.com/CalcProgrammer1/OpenRGB.git}"
DRY_RUN="${DRY_RUN:-true}"
SIMULATE_CONFLICT="${SIMULATE_CONFLICT:-false}"
WORK="${WORK:-/tmp/sync}"
OUT="${GITHUB_OUTPUT:-/dev/null}"
SUM="${GITHUB_STEP_SUMMARY:-/dev/stdout}"
export GIT_AUTHOR_NAME="${GIT_AUTHOR_NAME:-github-actions[bot]}" GIT_AUTHOR_EMAIL="${GIT_AUTHOR_EMAIL:-41898282+github-actions[bot]@users.noreply.github.com}"
export GIT_COMMITTER_NAME="$GIT_AUTHOR_NAME" GIT_COMMITTER_EMAIL="$GIT_AUTHOR_EMAIL"

mkdir -p "$WORK"
: >"$WORK/summary.md"
say() { echo "$*" | tee -a "$WORK/summary.md"; }
out() { echo "$1=$2" >>"$OUT"; }
do_push() { # args for git push
  # Fail closed: only the exact value "false" allows a real push (any typo stays a dry run).
  if [ "$DRY_RUN" != false ]; then say "- (dry run) would run: git push $*"; else git push "$@"; fi
}

git remote get-url upstream >/dev/null 2>&1 || git remote add upstream "$UPSTREAM_URL"
git fetch -q upstream '+refs/heads/master:refs/remotes/upstream/master'
git fetch -q upstream 'refs/tags/*:refs/tags/*' || git fetch -q --tags upstream
git fetch -q origin '+refs/heads/*:refs/remotes/origin/*'
UP="$(git rev-parse upstream/master)"
say "### OpenRGB sync ($([ "$DRY_RUN" = true ] && echo dry run || echo real run))"
say "Upstream master: \`${UP:0:9}\`"

# ---- 1. fast-forward the plain copies ------------------------------------------------------
for br in master electron-openrgb; do
  if ! git rev-parse -q --verify "origin/$br" >/dev/null; then say "- $br: branch missing on origin, skipped"; continue; fi
  cur="$(git rev-parse "origin/$br")"
  if [ "$cur" = "$UP" ]; then say "- $br: up to date"
  elif git merge-base --is-ancestor "$cur" "$UP"; then
    say "- $br: fast-forward \`${cur:0:9}\` -> \`${UP:0:9}\`"
    do_push origin "$UP:refs/heads/$br"
  else
    say "- $br: **diverged from upstream, not touched** (needs a manual look)"
    out mirror_diverged "${br}"
  fi
done

# ---- 2. mirror new tags --------------------------------------------------------------------
git ls-remote --tags --refs origin | awk '{print $2}' | sort >"$WORK/origin-tags"
git for-each-ref --format='%(refname)' refs/tags | grep -E '^refs/tags/(release_|release_candidate_)' | sort >"$WORK/up-tags" || true
comm -13 "$WORK/origin-tags" "$WORK/up-tags" >"$WORK/new-tags" || true
if [ -s "$WORK/new-tags" ]; then
  say "- new upstream tags: $(sed 's#refs/tags/##' "$WORK/new-tags" | tr '\n' ' ')"
  while read -r t; do do_push origin "$t:$t"; done <"$WORK/new-tags"
else say "- no new upstream tags"; fi

# ---- 3. rebase the razer branches ----------------------------------------------------------
rebase_variant() { # variant branch onto base-for-rebase(optional upstream-base)
  local variant="$1" branch="$2" onto="$3" oldbase="${4:-}"
  local cand="cand-$variant" log="$WORK/conflict-$variant.log"
  git rev-parse -q --verify "origin/$branch" >/dev/null || { say "- $branch: branch missing on origin, skipped"; return 0; }
  git checkout -q -B "$cand" "origin/$branch"
  out "${variant}_old_sha" "$(git rev-parse "origin/$branch")"
  if [ "$SIMULATE_CONFLICT" = true ]; then
    printf 'simulated conflict (SIMULATE_CONFLICT=true)\n' >"$log"
    say "- $branch: **simulated conflict**"; out "${variant}_conflict" true; return 0
  fi
  local rc=0
  if [ -n "$oldbase" ]; then git rebase --onto "$onto" "$oldbase" >"$log" 2>&1 || rc=$?; else git rebase "$onto" >"$log" 2>&1 || rc=$?; fi
  if [ "$rc" -ne 0 ]; then
    { echo; echo "--- git status ---"; git status --short | head -40; } >>"$log"
    git rebase --abort >/dev/null 2>&1 || true
    say "- $branch: **rebase onto \`$onto\` has conflicts**"; out "${variant}_conflict" true; return 0
  fi
  if [ "$(git rev-parse "$cand")" = "$(git rev-parse "origin/$branch")" ]; then say "- $branch: already up to date"; return 0; fi
  say "- $branch: rebased cleanly onto \`$onto\`"
  out "${variant}_ready" true
  READY+=("$cand")
}
READY=()

rebase_variant master openrgb-razer-master upstream/master

# stable: newer upstream release than the one the branch is based on?
base_tag="$(git tag --merged origin/openrgb-razer --list 'release_[0-9]*' 2>/dev/null | sort -V | tail -1 || true)"
latest_tag="$(git tag --list 'release_[0-9]*' | sort -V | tail -1 || true)"
if [ -n "$base_tag" ] && [[ $latest_tag =~ ^release_[0-9]+(\.[0-9]+)*$ ]] && [ "$base_tag" != "$latest_tag" ] && [ "$(printf '%s\n%s\n' "$base_tag" "$latest_tag" | sort -V | tail -1)" = "$latest_tag" ]; then
  ver="${latest_tag#release_}"
  rtag="openrgb-razer-v${ver}-r1"
  if git ls-remote --exit-code --tags origin "refs/tags/$rtag" >/dev/null 2>&1; then
    say "- stable: $rtag already exists"
  else
    say "- stable: new upstream release \`$latest_tag\` (branch is on \`$base_tag\`) -> $rtag"
    out stable_tag "$rtag"
    rebase_variant stable openrgb-razer "$latest_tag" "$base_tag"
  fi
else
  say "- stable: openrgb-razer is on the newest upstream release (\`${base_tag:-none}\`)"
fi

if [ ${#READY[@]} -gt 0 ]; then git bundle create "$WORK/candidates.bundle" "${READY[@]}" >/dev/null 2>&1; out bundle true; fi
out date "$(date -u +%Y%m%d)"
cat "$WORK/summary.md" >>"$SUM" 2>/dev/null || true
