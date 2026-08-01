// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { HttpResponse, http } from "msw";

/**
 * Default `/api/v1/auth/refresh` handler for tests that assert a 401 on some
 * other endpoint triggers a logout. The client now attempts a refresh before
 * giving up, so those tests need a refresh outcome — 401 here reproduces "the
 * session is actually invalid" (refresh cookie invalid too), matching what
 * these tests were asserting before the refresh-and-retry flow existed.
 * Tests exercising a successful refresh override this with `server.use(...)`.
 */
export const refresh401Handler = http.post(
  "/api/v1/auth/refresh",
  () => new HttpResponse("Unauthorized", { status: 401 }),
);
