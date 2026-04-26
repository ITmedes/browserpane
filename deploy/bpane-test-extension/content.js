function activateFixture() {
  const body = document.body;
  if (!body) {
    return;
  }
  body.dataset.extensionReady = "1";
  document.documentElement.dataset.extensionReady = "1";
  document.title = "Workflow Extension Fixture Activated";
  let marker = document.getElementById("bpane-extension-marker");
  if (!marker) {
    marker = document.createElement("div");
    marker.id = "bpane-extension-marker";
    marker.textContent = "BrowserPane extension ready";
    marker.style.cssText =
      "position:fixed;top:12px;right:12px;z-index:2147483647;padding:8px 10px;background:#0b6bcb;color:#fff;font:600 12px/1.2 sans-serif;border-radius:999px;";
    body.appendChild(marker);
  }
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", activateFixture, { once: true });
} else {
  activateFixture();
}
