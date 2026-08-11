#!/bin/bash
#
# PreToolUse: gate Edit/Write by branch and path.
#
# Policy:
#   - main/master branch          -> DENY (integration branch is merge-only)
#   - inside a worktree           -> ALLOW (agent/supervisor code work is isolated there)
#   - orchestrator doc surface     -> ALLOW (CLAUDE.md, .planning/**, .research/**, memory/)
#   - anything else in the main dir -> fall through to normal permission (prompt)
#
# WHY the explicit allow(): a PreToolUse hook only suppresses the permission prompt when it
# emits {"permissionDecision":"allow"}. A silent `exit 0` merely means "no opinion" and defers
# to the normal permission system, which then prompts. The previous version only ever emitted a
# decision for deny-on-main, so every other write prompted. This is deliberately NOT a blanket
# "allow any non-main branch": writing source into the main working tree (instead of a worktree)
# still prompts, which is the backstop that keeps code work in worktrees.
#

INPUT=$(cat)
TOOL_NAME=$(echo "$INPUT" | jq -r '.tool_name // empty')
[[ "$TOOL_NAME" != "Edit" ]] && [[ "$TOOL_NAME" != "Write" ]] && exit 0

allow() { echo "{\"hookSpecificOutput\":{\"hookEventName\":\"PreToolUse\",\"permissionDecision\":\"allow\",\"permissionDecisionReason\":\"$1\"}}"; exit 0; }

FILE_PATH=$(echo "$INPUT" | jq -r '.tool_input.file_path // empty')

# Worktrees + plan files: always fine.
[[ "$FILE_PATH" == *"/.worktrees/"* ]] && allow "Worktree path (isolated from main)."
[[ "$(pwd)" == *"/.worktrees/"* ]]     && allow "Inside a worktree (isolated from main)."
[[ "$FILE_PATH" == *"/.claude/plans/"* ]] && allow "Plan-mode file."

# Orchestrator documentation surface in the main dir.
[[ "$FILE_PATH" == *"/CLAUDE.md" ]]                       && allow "CLAUDE.md (orchestrator doc surface)."
[[ "$FILE_PATH" == *"/.planning/"* ]]                     && allow "Planning docs (orchestrator doc surface)."
[[ "$FILE_PATH" == *"/.research/"* ]]                     && allow "Research docs (orchestrator doc surface)."
[[ "$FILE_PATH" == *"/.claude/projects/"*"/memory/"* ]]   && allow "Orchestrator memory."

# Protected branches: hard deny.
CURRENT_BRANCH=$(git branch --show-current 2>/dev/null)
if [[ "$CURRENT_BRANCH" == "main" ]] || [[ "$CURRENT_BRANCH" == "master" ]]; then
  echo "{\"hookSpecificOutput\":{\"hookEventName\":\"PreToolUse\",\"permissionDecision\":\"deny\",\"permissionDecisionReason\":\"Cannot edit on $CURRENT_BRANCH; main is merge-only. Work in a worktree (.worktrees/bd-{BEAD_ID}/).\"}}"
  exit 0
fi

# Non-protected branch, non-doc write in the main dir: defer to normal permission (prompt).
exit 0
