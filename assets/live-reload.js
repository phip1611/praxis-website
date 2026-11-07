// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Development mode only: reloads the page once the server restarted with a
// new build, e.g., by `bacon`.
"use strict";

(() => {
  const POLL_INTERVAL_MS = 500;
  let known = null;

  async function poll() {
    try {
      const response = await fetch("/__dev/boot-id", { cache: "no-store" });
      if (response.ok) {
        const id = await response.text();
        if (known === null) {
          known = id;
        } else if (id !== known) {
          window.location.reload();
          return;
        }
      }
    } catch {
      // The server is restarting.
    }
    window.setTimeout(poll, POLL_INTERVAL_MS);
  }

  poll();
})();
