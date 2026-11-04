# ADR 0006: Login and Permissions

- Status: adopted (2026-10-30); questions Q1–Q5 at the end are with IT, and their answers will be recorded in this file
- Scope: login, sessions, permission checks, plant isolation, identities for AI and background jobs
- Related TipTop: `lib/4gl/cl_user.4gl` (user and plant), `lib/4gl/cl_setup.4gl` `cl_set_priv` (merging program permissions),
  `lib/4gl/cl_chk_act_auth.4gl` (action and data permission checks); tables zx, zxy, zxw, zw, zy, zyw, gbl, gbt

## Background

### How TipTop does it

| Topic | Approach | Source |
|---|---|---|
| Identity | The Unix login name `LOGNAME` is the user (`WEBUSER` / `WSUSER` for web and background jobs); passwords are managed by the OS | cl_user.4gl:195–210 |
| User | Must exist in `zx_file`, with active code `zxacti` = Y and not past expiry `zx18`; department `zx03`, permission class `zx04`, language `zx06` | cl_user.4gl:254–268 |
| Plant | Default `zx08`; must be in the allowed list `zxy_file`; multi-plant users `zx07` = Y can switch | cl_user.4gl:505–549 |
| Program permission | With no user-specific permission `zxw`: class `zx04` × program in `zy_file` (action list `zy03`, user data `zy04`, department data `zy05`, print `zy06`, detail `zy07`) | cl_setup.4gl:551–567 |
| Multiple permissions | `zxw03` = 1 grants by class, = 2 grants a program directly; **actions are unioned and data permissions take the most permissive** | cl_setup.4gl:576–744 |
| Action check | The action ID must be in the action list; for modify/delete on **someone else's record** (`xxuser` differs) apply `zy04`, and on **another department's record** (`xxgrup` differs) apply `zy05` | cl_chk_act_auth.4gl:77–122 |
| Data permission levels | 0 query/modify/delete, 1 no delete, 2 no modify, 3 query only, 4 no access (`zy05` also has 5–8, tied to permission groups `zyw`) | zy_file.sch, cl_setup.4gl:710–744 |
| Permission groups | Departments in the same `zyw_file` group, or a group's manager, may act on each other's department data | cl_chk_act_auth.4gl:268–391 |
| Custom actions | `gbl_file` maps custom actions to query/modify/delete so they follow the same data-permission rules | cl_chk_act_auth.4gl:57–70 |
| Account policy | `gbt_file` password length, change cycle, failed attempts; `zx16`/`zx17`/`zx19`/`zx20` password state | gbt_file.sch, zx_file.sch |

### The new system today

The API has no login: every request runs as `actor_from_env()` (environment variable `TTRS_ACTOR_*`), and permission class only affects new program grants.

## Decisions

### 1. Authentication: company SSO (OIDC)

> **Superseded by ADR 0009 (2026-11-04):** TTRS signs in with user ID + Web password; §1, §2 (company account binding) and the password fields in §3 no longer apply.

TTRS doesn't store passwords; the company identity system (IdP) verifies identity. Each client type uses a different flow:

| Client | Flow | Credential held by the API |
|---|---|---|
| Web (Dioxus wasm) | **Backend-for-frontend (BFF)**: the API does Authorization Code + PKCE (`/auth/login`, `/auth/callback`, `/auth/logout`) | Server-side session (PostgreSQL); browser only holds an `HttpOnly; Secure; SameSite=Lax` cookie; tokens never reach the browser |
| Desktop app | Authorization Code + PKCE (native app, loopback redirect) | Bearer access token (JWT) |
| Warehouse PDA | Device code flow (enter a code on another device), or treat as desktop | Bearer access token |
| External systems, background jobs | Client credentials (service principal) | Bearer access token, mapped to a TTRS **service account** |
| AI agents (MCP) | On behalf of a user (OBO / token exchange); see §6 | Bearer access token, marked as AI |

- The API validates JWTs: signature (JWKS cache), `iss`, `aud`, `exp`, `nbf`.
- Identity claims used: `sub` (or Entra `oid`), `preferred_username` / UPN, `name`, `email`.
- **IdP groups are not used for authorization**: the IdP only answers "who you are"; "what you may do" stays in TTRS, compatible with TipTop permission data (see §3).

### 2. User mapping

- `user_account` (TipTop `zx_file`) keeps `user_id` (zx01). It is still the identifier stored in every table's `created_by` and in audit, so TipTop data imports directly.
- New fields `idp_subject` (unique) and `idp_username` bind the company account.
  When an administrator creates a user in p_zx they enter the company account (UPN); the first login fills in `idp_subject`. They can also be bound in bulk by rule (see question Q2).
- **No automatic account creation**: a company account not bound in TTRS gets `USER_NOT_REGISTERED` on login.
- Active code (zxacti) and expiry (zx18) still apply: checked at login, and sessions re-check periodically (default every 10 minutes).
  A user disabled in the IdP can't log in again; existing sessions end when the token can't be refreshed.

### 3. Fields and tables not ported

Handled by the IdP (including MFA, lockout, password policy), not ported: `zx10` Web password, `zx16` password changed on, `zx17` password use count,
`zx19` force change at next login, `zx20` failed attempts, and the whole `gbt_file` account policy. legacy-map marks them "not ported (IdP handles it)".

### 4. Authorization: keep TipTop's model

- Data imports after program-code mapping (ADR 0008 E7): permission class (zw), program permission (zy), user permission (zxw), department permission group (zyw), custom action mapping (gbl). TipTop program codes (zz01) are translated to English program codes via `program.tiptop_program_no`; rows for programs not ported are skipped and listed.
- **Action vocabulary** uses TipTop action IDs (`insert`, `modify`, `delete`, `query`, `output`, `detail`, `reproduce`, `invalid`, `confirm`, …), so the comma-separated `zy03` list imports as is.
- **Effective permission** = the `cl_set_priv` merge rules (union of actions, most permissive data permission), implemented as a pure domain function and covered by the p_zy spec BDD.
- **Enforced in the app layer**: each use case declares its (program code, action); `Authorizer` (an app port) loads the effective permission;
  modify/delete compare the record's `created_by` / `created_dept` (TipTop `xxuser` / `xxgrup`) with data permissions; queries turn
  "can't query others' data" (level 4) into query conditions.
- Denial returns `PERMISSION_DENIED` (HTTP 403, with program code and action) and is written to audit.
- The UI hides disallowed actions and menus (p_zm menus are filtered by permission), but **the server always checks**; the UI is only a convenience.

### 5. Plant isolation

> Revised 2026-11-03 by ADR 0007: one PostgreSQL schema per plant (`ds01`, `ds02`, …) plus the shared schema `public`.
> Isolation is a per-plant database role and `search_path` set at the start of each transaction, replacing the RLS
> design below. Plant switching (allowed plants, session / `X-TTRS-Plant`) stays as written.

- Allowed plants come from `zxy_file`, the default from `zx08`, and only multi-plant users (`zx07`) can switch.
- Current plant: chosen at login or switched later; web keeps it in the session, bearer clients send the `X-TTRS-Plant` header; every request is checked against the allowed list.
- **PostgreSQL Row-Level Security**: business tables enable RLS with the policy `plant_id = current_setting('ttrs.plant_id')`;
  each transaction first runs `SET LOCAL ttrs.plant_id` (from the Actor). The application's database role has no BYPASSRLS.
- System-level tables without `plant_id` (programs, permissions, data dictionary, …) are not under RLS and are protected by program permissions.
- Cross-plant reports are designed separately later (allowed-plant array `ttrs.plant_ids`).

### 6. AI and service identities

- AI agents (MCP, in-screen assistant) **act on behalf of a user**, with no more permissions than that user, and the Actor is marked `kind = ai`.
- AI may only use queries and "create draft" actions; `confirm`, `post`, `approve`, `delete` and similar actions are denied by default for `kind = ai`, so a person must perform them in the UI (roadmap principle "AI doesn't post directly").
- Background jobs and integrations use service accounts (`kind = service`), each with its own permission class, never borrowing a person's account. This replaces TipTop's `WSUSER` / `BGJOB` environment-variable identities.

### 7. Audit

Written to audit events: login success/failure, logout, plant switch, permission denials, changes to permissions and user data.
Each record includes the actor type (person, AI, service) and the user being acted for. The event-table design comes with the audit ADR; until then `audit_log` is used.

### 8. Development and testing

- `TTRS_AUTH_MODE`: `oidc` (default) or `dev` (local development only; keeps `actor_from_env()`).
  The API refuses to start in `dev` mode when `TTRS_ENV=production`.
- BDD calls the app layer directly with an `Actor`, unaffected by login; API contract tests use `dev` mode;
  OIDC gets integration tests with a test IdP (self-signed JWKS).

## Implementation order

1. p_zx user maintenance spec (including company-account binding) → `user_account`, `user_plant`
2. p_zw, p_zy, p_zxw, p_zyw permission specs → effective-permission domain rules (cl_set_priv, cl_chk_act_auth)
3. app `Authorizer` + `PERMISSION_DENIED`; add permission checks to existing p_zz, p_zta, aimi100
   — done (2026-11-03): `specs/features/azz/authorization.feature` (AQ1–AQ13). Every use case of the ported programs
   declares (program, action); related departments (zy05 5–8) are only the user's own department until p_zyw is ported;
   custom-action mapping `gbl_file` (p_auth_as) and item data permissions come later; denials go to the application log until the audit ADR
4. OIDC login endpoints, server-side sessions, web login and user menu (switch plant, log out)
5. Plant switching and plant isolation (business tables first: item, inventory_txn) — schema per plant, see ADR 0007
6. p_zm menus: the left-hand menu comes from data and is filtered by permission

## Questions for IT

| # | Question | Recommendation |
|---|---|---|
| Q1 | Which IdP: Entra ID (Microsoft 365) or on-premises AD FS? Who registers the TTRS application (redirect URIs) and manages the client secret? | If it's Entra ID, IT registers the application; TTRS only needs tenant ID, client ID and a secret (in `.env`) |
| Q2 | How do TipTop user codes `zx01` map to company accounts (for example `zx01` = employee ID, UPN = `employee-id@company.com`)? | If there's a rule, bind in bulk; otherwise fill in on p_zx |
| Q3 | Assign permission classes from IdP groups? | Not for now; keep permissions in TTRS so they stay TipTop-compatible |
| Q4 | Session lifetime (idle and maximum)? | Idle 30 minutes, maximum 12 hours; TipTop's per-class idle control (zw05/zw06) is decided when porting p_zw |
| Q5 | Can the PDA log in with a company account (device code)? | Confirm once PDA models and network are settled; until then use a shared warehouse service account with limited permissions |

## Consequences

- No passwords stored in TTRS; MFA and account policy are managed centrally by the company.
- TipTop permission data imports directly, and the permission behaviour users know is preserved.
- Login depends on the IdP; an IdP outage stops logins (existing sessions continue until they expire).
- `zy05` levels 5–8 combined with permission groups `zyw` are confirmed with the user and specced when porting p_zy (not guessed now).
