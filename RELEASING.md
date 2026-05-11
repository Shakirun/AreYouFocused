# Branching and releases

## Model

| Branch | Purpose | `package.json` version |
|--------|-----------|-------------------------|
| **`main`** | Stable releases; merges from `develop` only (via MR) | `0.x.y` (no prerelease suffix) |
| **`develop`** | Feature integration; unstable line | `0.x.y-dev.N` (see below) |
| **`feature/*`** | Branch from `develop`, MR back into `develop` | No global version policy change |

Features **must not** be pushed straight to `main`. Flow: `feature/...` → **`develop`** → when ready for a release, one MR **`develop` → `main`** with a written release summary.

## Day-to-day

1. From `develop`: `git fetch origin && git checkout develop && git pull`.
2. Create a branch: `git checkout -b feature/short-name`.
3. Commit with TDD, push: `git push -u origin feature/short-name`.
4. Open an **MR into `develop`**, wait for review/CI, merge.

To bump the unstable counter on `develop` after large merges: a separate commit `chore: bump dev pre-release` (e.g. `0.2.0-dev.0` → `0.2.0-dev.1`).

## Release to `main`

When `develop` has enough for a release:

1. Ensure `develop` is green (tests, build).
2. Open an **MR `develop` → `main`** (prefer merge commit or squash per team policy; avoid squashing all of `develop` history into one commit unless intentional).
3. In the MR body, use the **“Release → main”** template (`.github/PULL_REQUEST_TEMPLATE/release_to_main.md`): list **what ships**, version, risks, verification.
4. In the same release branch (or a follow-up right after merge to `main`): set **`package.json`** to a **stable** `0.x.y`, tag `v0.x.y` if you use tags.
5. After merge to `main`: merge or rebase `main` back into `develop` so `develop` does not fall behind; bump the dev version on `develop` for the next cycle (`0.(x+1).0-dev.0`).

## Branch protection (GitHub)

In **Settings → Branches → Branch protection rules** (or **Rulesets**):

### `main`

- Require a pull request before merging.
- Require approvals (e.g. minimum 1 when working with others — tune for solo vs team).
- **Do not allow bypass** for admins if you want strict enforcement.
- Restrict who can push: no direct pushes (MR only).
- Optional: required status checks.

### `develop`

- Require a pull request before merging (features via MR only).
- Allow direct push only if you deliberately want “owner only” hotfixes — default is MR-only as well.
- Disallow force-push and branch deletion.

Exact toggles depend on the GitHub UI; with **Rulesets**, combine rules for the `main` and `develop` name patterns.

## GitHub Rulesets — solo maintainer (exact setup)

Use **two rulesets** so you keep the `develop` → `main` flow without a second human approver. **Required approvals = 0** still forces a **PR + merge** (audit trail) while allowing self-merge.

### Ruleset A — `main` (release line)

| Field | Value |
|--------|--------|
| **Ruleset name** | `AreYouFocused — main (solo release gate)` |
| **Enforcement status** | Active |
| **Target branches** | Include by pattern: `main` (or “Default branch” if it is `main`) |
| **Bypass list** | Empty — *do not* add repository admin bypass if you want the same discipline as a team; add **yourself** (or Repository admins) only if you want emergency direct pushes without PR. |

**Branch rules**

1. **Restrict deletions** — ON  
2. **Restrict force pushes** — ON  
3. **Require a pull request before merging** — ON  
   - **Required approvals:** `0`  
   - **Dismiss stale pull request approvals when new commits are pushed:** optional (OFF is simpler for solo)  
   - **Require review from Code Owners:** OFF  
   - **Require approval of the most recent reviewable push:** OFF  
4. **Require linear history:** OFF (unless you explicitly want rebased linear `main`; merge commits are fine for `develop` → `main`)  
5. **Require deployments / status checks / signed commits:** OFF until CI exists; add **required status checks** when `npm run build` (or similar) is on PRs.

### Ruleset B — `develop` (integration line)

| Field | Value |
|--------|--------|
| **Ruleset name** | `AreYouFocused — develop (solo integration)` |
| **Enforcement status** | Active |
| **Target branches** | Include by pattern: `develop` |
| **Bypass list** | Optional: **Repository admin** only, if you want rare direct commits to `develop` without a PR. Otherwise empty. |

**Branch rules**

1. **Restrict deletions** — ON  
2. **Restrict force pushes** — ON  
3. **Require a pull request before merging** — ON  
   - **Required approvals:** `0`  
4. Same optional extras as on `main` when CI appears.

### Import JSON (API / “Import ruleset”)

If you manage rules as code, create each ruleset via [REST: Create a repository ruleset](https://docs.github.com/en/rest/repos/rules#create-a-repository-ruleset) or paste JSON where the UI supports import. Replace `YOUR_ORG_OR_USER` / repo name if needed; `bypass_actor_id` is your numeric user id — **omit `bypass_actors`** for strict solo (no bypass).

**`main` (strict, no bypass):**

```json
{
  "name": "AreYouFocused — main (solo release gate)",
  "target": "branch",
  "enforcement": "active",
  "conditions": {
    "ref_name": {
      "exclude": [],
      "include": ["refs/heads/main"]
    }
  },
  "rules": [
    { "type": "deletion" },
    { "type": "non_fast_forward" },
    {
      "type": "pull_request",
      "parameters": {
        "required_approving_review_count": 0,
        "dismiss_stale_reviews_on_push": false,
        "require_code_owner_reviews": false,
        "require_last_push_approval": false,
        "required_review_thread_resolution": false
      }
    }
  ],
  "bypass_actors": []
}
```

**`develop` (strict, no bypass):**

```json
{
  "name": "AreYouFocused — develop (solo integration)",
  "target": "branch",
  "enforcement": "active",
  "conditions": {
    "ref_name": {
      "exclude": [],
      "include": ["refs/heads/develop"]
    }
  },
  "rules": [
    { "type": "deletion" },
    { "type": "non_fast_forward" },
    {
      "type": "pull_request",
      "parameters": {
        "required_approving_review_count": 0,
        "dismiss_stale_reviews_on_push": false,
        "require_code_owner_reviews": false,
        "require_last_push_approval": false,
        "required_review_thread_resolution": false
      }
    }
  ],
  "bypass_actors": []
}
```

## CLI (optional)

With `gh` and repo permissions you can open PRs and configure rules via the API; for most teams the UI above is enough.
