# Testing Flows

Manual verification flows for OwnPulse. These supplement the automated test suites and are intended for pre-release QA or onboarding new contributors.

## Prerequisites

```bash
# Start backend + web locally (Option B from CLAUDE.md)
docker run -d -e POSTGRES_PASSWORD=dev -p 5432:5432 --name pg postgres:17
export DATABASE_URL=postgres://postgres:dev@localhost:5432/health
cd db && sqlx migrate run
cd backend && cargo run -p api         # API on :8080
cd web && npm install && npm run dev   # Web on :5173
```

Set these environment variables for the backend:
```bash
export JWT_SECRET=$(openssl rand -hex 32)
export ENCRYPTION_KEY=$(openssl rand -hex 32)
export WEB_ORIGIN=http://localhost:5173
# For Google OAuth (optional for local login):
export GOOGLE_CLIENT_ID=your-client-id
export GOOGLE_CLIENT_SECRET=your-client-secret
export GOOGLE_REDIRECT_URI=http://localhost:8080/api/v1/auth/google/callback
```

---

## Flow 1: Local Login

1. Open http://localhost:5173/login
2. Enter username and password
3. Verify redirect to Timeline (/)
4. Verify nav bar shows: Timeline, Data Entry, Sources, Settings, Logout
5. Click Logout → redirects to /login
6. Verify Timeline is not accessible without login (redirects to /login)

**What's tested automatically:** JWT encode/decode, login endpoint (401 on wrong password, 200 on correct), refresh token rotation, logout invalidation.

---

## Flow 2: Google OAuth Login

1. Open http://localhost:5173/login
2. Click "Sign in with Google"
3. Complete Google consent flow
4. Verify redirect back to Timeline with token in URL
5. Verify URL is cleaned (no `?token=` visible)
6. Refresh the page → verify you stay logged in (refresh token in cookie)

**What's tested automatically:** Token exchange mock (WireMock in CI), JWT issuance, cookie handling.

---

## Flow 3: Daily Check-in

1. Navigate to Data Entry → Check-in tab
2. Select today's date
3. Enter scores: Energy 7, Mood 8, Focus 6, Recovery 5, Libido 7
4. Add optional notes
5. Click Save → verify "Saved" confirmation
6. Navigate to Timeline → verify check-in appears
7. Go back to Data Entry, enter same date with different scores → verify upsert (overwrites)

**What's tested automatically:** Upsert ON CONFLICT, score range validation (1-10), list/get endpoints.

---

## Flow 4: Log an Intervention

1. Navigate to Data Entry → Intervention tab
2. Enter: Substance "Caffeine", Dose 200, Unit "mg", Route "oral"
3. Set time to now, check "Fasted"
4. Click Save → verify confirmation
5. Navigate to Timeline → verify intervention appears

**What's tested automatically:** Create, list, get, delete endpoints.

---

## Flow 5: Record Health Data

1. Navigate to Data Entry → Health Record tab
2. Enter: Source "manual", Type "heart_rate", Value 72, Unit "bpm"
3. Click Save
4. Create another: Source "manual", Type "heart_rate", Value 73, Unit "bpm", same time
5. Verify deduplication warning in backend logs (second record should have `duplicate_of` set)

**What's tested automatically:** Create with dedup, list, get, delete, HealthKit write-back guard.

---

## Flow 6: Add an Observation

1. Navigate to Data Entry → Observation tab
2. Select Type "event_instant", Name "Sauna", set time
3. Click Save
4. Select Type "scale", Name "Sleep quality", enter value 8
5. Click Save
6. Try Type "invalid_type" (if testing via API) → should get 400

**What's tested automatically:** Create with type validation, list, get, delete, invalid type rejection.

---

## Flow 7: Enter Lab Results

1. Navigate to Data Entry → Lab Result tab
2. Enter: Date today, Lab "Quest", Marker "TSH", Value 2.5, Unit "mIU/L"
3. Set Reference Low 0.4, Reference High 4.0
4. Click Save → verify `out_of_range` is false (computed)
5. Enter: Marker "TSH", Value 5.0, same refs → verify `out_of_range` is true

**What's tested automatically:** Create, list, out_of_range generated column.

---

## Flow 8: Export Data

1. Navigate to Settings
2. Click "Export JSON" → verify file downloads as `ownpulse-export.json`
3. Open the file → verify `schema_version: "0.1.0"`, `exported_at` timestamp, arrays for all data types
4. Click "Export CSV" → verify file downloads as `ownpulse-export.csv`
5. Open CSV → verify header row: `id,source,record_type,value,unit,start_time,end_time`

**What's tested automatically:** JSON export (schema_version present, all arrays), CSV export (header row).

---

## Flow 9: Source Preferences

1. Navigate to Settings
2. In Source Preferences section, set metric "heart_rate" preferred source to "garmin"
3. Refresh page → verify preference persists

**What's tested automatically:** List + upsert endpoints.

---

## Flow 10: Account Deletion

1. Navigate to Settings
2. Click "Delete Account"
3. Confirm in the dialog
4. Verify redirect to login page
5. Verify all user data is gone (try logging in → should fail)

**What's tested automatically:** DELETE /account cascades all user data, returns 204.

---

## Automated Test Commands

```bash
# Backend
cd backend
cargo test --lib                    # 5 unit tests (JWT, crypto, hashing)
cargo test --test integration       # 33 integration tests (all endpoints)
cargo clippy -- -D warnings         # 0 warnings

# Web
cd web
npm test                            # 14 vitest unit tests
npx tsc --noEmit                    # 0 type errors
```
