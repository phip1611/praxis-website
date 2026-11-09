// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Starts the self-hosted Matomo without cookies. The same snippet as on the
// WordPress site, but as a file, so that the Content Security Policy can
// forbid inline scripts. The configuration comes from data attributes.
"use strict";

(() => {
  const script = document.currentScript;
  if (!script) {
    return;
  }
  const { trackerUrl, libraryUrl, siteId } = script.dataset;
  const queue = (window._paq = window._paq || []);
  queue.push(["disableCookies"]);
  queue.push(["enableHeartBeatTimer"]);
  queue.push(["trackPageView"]);
  queue.push(["enableLinkTracking"]);
  queue.push(["setTrackerUrl", trackerUrl]);
  queue.push(["setSiteId", siteId]);
  const library = document.createElement("script");
  library.async = true;
  library.src = libraryUrl;
  document.head.appendChild(library);
})();
