// web-companion UI (ai-bitwarden-hw-key-eml.6): vanilla JS, no build step,
// no framework. Talks to the axum server's /api/* routes over same-origin
// fetch, authenticated with the per-process bearer token the server injects
// into index.html as `window.__BHK_API_TOKEN__` (see src/routes.rs /
// src/auth.rs). The token is read once into this module's closure and never
// written to localStorage, a cookie, or a URL.
//
// Views are plain <section> elements toggled via a `hidden` class -- no
// client-side router, no virtual DOM. State lives in a handful of `let`
// bindings below.
(function () {
  "use strict";

  const API_TOKEN = window.__BHK_API_TOKEN__;

  // ---------------------------------------------------------------------
  // Two-factor provider metadata. `TwoFactorProviders` (bitwarden-core) has
  // no `u2f` field (deprecated) and `remember` isn't a selectable method --
  // it just signals "this device can bypass 2FA" -- so it's intentionally
  // left out of this map. Numeric values must match
  // `bitwarden_core::auth::login::two_factor::TwoFactorProvider`'s `#[repr(u8)]`.
  // ---------------------------------------------------------------------
  const PROVIDER_INFO = {
    authenticator: { value: 0, label: "Authenticator app" },
    email: { value: 1, label: "Email" },
    duo: { value: 2, label: "Duo" },
    yubiKey: { value: 3, label: "YubiKey" },
    organizationDuo: { value: 6, label: "Duo (organization)" },
    webAuthn: { value: 7, label: "Security key (WebAuthn)" },
  };
  const PROVIDER_ORDER = [
    "authenticator",
    "email",
    "duo",
    "yubiKey",
    "organizationDuo",
    "webAuthn",
  ];

  // ---------------------------------------------------------------------
  // DOM refs
  // ---------------------------------------------------------------------
  const el = {
    statusBar: document.getElementById("status-bar"),
    statusText: document.getElementById("status-text"),
    lockBtn: document.getElementById("lock-btn"),
    logoutBtn: document.getElementById("logout-btn"),

    viewLogin: document.getElementById("view-login"),
    loginForm: document.getElementById("login-form"),
    loginEmail: document.getElementById("login-email"),
    loginPassword: document.getElementById("login-password"),
    loginError: document.getElementById("login-error"),

    viewTwoFactor: document.getElementById("view-twofactor"),
    twoFactorForm: document.getElementById("twofactor-form"),
    twoFactorProvider: document.getElementById("twofactor-provider"),
    twoFactorCode: document.getElementById("twofactor-code"),
    twoFactorCancel: document.getElementById("twofactor-cancel"),
    twoFactorError: document.getElementById("twofactor-error"),

    viewApp: document.getElementById("view-app"),
    vaultRefreshBtn: document.getElementById("vault-refresh-btn"),
    vaultMeta: document.getElementById("vault-meta"),
    vaultError: document.getElementById("vault-error"),
    vaultSearch: document.getElementById("vault-search"),
    vaultSelectAll: document.getElementById("vault-select-all"),
    vaultList: document.getElementById("vault-list"),

    deviceSelect: document.getElementById("device-select"),
    deviceError: document.getElementById("device-error"),
    syncBtn: document.getElementById("sync-btn"),
    syncHint: document.getElementById("sync-hint"),
    syncResult: document.getElementById("sync-result"),
  };

  // ---------------------------------------------------------------------
  // In-memory client state. Nothing here is persisted -- a page reload
  // starts this over from scratch and re-derives the view from
  // GET /api/auth/status (the server holds the real session).
  // ---------------------------------------------------------------------
  let vaultItems = [];
  const selectedIds = new Set();
  let devices = [];

  // ---------------------------------------------------------------------
  // fetch helper -- attaches the bearer token to every /api/* call.
  // ---------------------------------------------------------------------
  async function api(path, options) {
    const opts = options || {};
    const headers = Object.assign(
      { Authorization: "Bearer " + API_TOKEN },
      opts.body ? { "Content-Type": "application/json" } : {},
      opts.headers || {}
    );
    return fetch(path, Object.assign({}, opts, { headers }));
  }

  async function readError(res, fallback) {
    try {
      const body = await res.json();
      if (body && typeof body.error === "string" && body.error.length > 0) {
        return body.error;
      }
    } catch (_err) {
      // Non-JSON or empty body (e.g. the bearer-token middleware's bare
      // 401) -- fall through to the caller-supplied message.
    }
    return fallback;
  }

  function showError(node, message) {
    node.textContent = message;
    node.classList.remove("hidden");
  }

  function hideError(node) {
    node.textContent = "";
    node.classList.add("hidden");
  }

  // ---------------------------------------------------------------------
  // View switching
  // ---------------------------------------------------------------------
  function showView(name) {
    el.viewLogin.classList.toggle("hidden", name !== "login");
    el.viewTwoFactor.classList.toggle("hidden", name !== "twofactor");
    el.viewApp.classList.toggle("hidden", name !== "app");
    el.statusBar.classList.toggle("hidden", name !== "app");
  }

  function resetClientVaultState() {
    vaultItems = [];
    selectedIds.clear();
    devices = [];
    el.vaultList.innerHTML = "";
    el.deviceSelect.innerHTML = "";
    el.vaultSearch.value = "";
    el.vaultSelectAll.checked = false;
    el.syncResult.classList.add("hidden");
    hideError(el.vaultError);
    hideError(el.deviceError);
  }

  function goToLogin(message) {
    resetClientVaultState();
    el.loginForm.reset();
    if (message) {
      showError(el.loginError, message);
    } else {
      hideError(el.loginError);
    }
    showView("login");
  }

  // ---------------------------------------------------------------------
  // Two-factor provider select population
  // ---------------------------------------------------------------------
  function populateProviderSelect(providers) {
    el.twoFactorProvider.innerHTML = "";
    let keys = PROVIDER_ORDER.filter(
      (key) => providers && providers[key] != null
    );
    if (keys.length === 0) {
      // Either no metadata was available (e.g. the page was reloaded while
      // a two-factor login was already pending server-side, so we never
      // saw the original login response) or the server reported a
      // combination this UI doesn't recognize -- fall back to offering
      // every known method rather than leaving the form unusable.
      keys = PROVIDER_ORDER;
    }
    for (const key of keys) {
      const info = PROVIDER_INFO[key];
      const option = document.createElement("option");
      option.value = String(info.value);
      option.textContent = info.label;
      el.twoFactorProvider.appendChild(option);
    }
  }

  // ---------------------------------------------------------------------
  // Login
  // ---------------------------------------------------------------------
  el.loginForm.addEventListener("submit", async (event) => {
    event.preventDefault();
    hideError(el.loginError);

    const email = el.loginEmail.value.trim();
    const masterPassword = el.loginPassword.value;

    if (!email || !masterPassword) {
      showError(el.loginError, "Enter your email and master password.");
      return;
    }

    let res;
    try {
      res = await api("/api/auth/login", {
        method: "POST",
        body: JSON.stringify({ email, master_password: masterPassword }),
      });
    } catch (_err) {
      showError(
        el.loginError,
        "Couldn't reach the web-companion server. Is it running?"
      );
      return;
    } finally {
      el.loginPassword.value = "";
    }

    if (res.ok) {
      const body = await res.json();
      if (body.status === "unlocked") {
        el.loginForm.reset();
        await enterApp();
        return;
      }
      if (body.status === "two_factor_required") {
        populateProviderSelect(body.providers);
        el.twoFactorCode.value = "";
        hideError(el.twoFactorError);
        showView("twofactor");
        return;
      }
      showError(el.loginError, "Something went wrong. Please try again.");
      return;
    }

    if (res.status === 401) {
      showError(el.loginError, "Wrong email or master password.");
    } else if (res.status === 400) {
      showError(el.loginError, "Enter your email and master password.");
    } else if (res.status === 409) {
      // Another tab/request already has a session going -- resync to
      // whatever the server actually thinks is true rather than guessing.
      await bootstrap();
    } else {
      showError(
        el.loginError,
        await readError(res, "Something went wrong. Please try again.")
      );
    }
  });

  // ---------------------------------------------------------------------
  // Two-factor
  // ---------------------------------------------------------------------
  el.twoFactorForm.addEventListener("submit", async (event) => {
    event.preventDefault();
    hideError(el.twoFactorError);

    const provider = Number(el.twoFactorProvider.value);
    const token = el.twoFactorCode.value.trim();
    if (!token) {
      showError(el.twoFactorError, "Enter the verification code.");
      return;
    }

    let res;
    try {
      res = await api("/api/auth/2fa", {
        method: "POST",
        body: JSON.stringify({ provider, token }),
      });
    } catch (_err) {
      showError(
        el.twoFactorError,
        "Couldn't reach the web-companion server. Is it running?"
      );
      return;
    }

    if (res.ok) {
      el.twoFactorForm.reset();
      await enterApp();
      return;
    }

    if (res.status === 401) {
      showError(el.twoFactorError, "That code didn't work. Try again.");
      el.twoFactorCode.value = "";
    } else if (res.status === 400) {
      goToLogin("Your login session expired. Please log in again.");
    } else {
      showError(
        el.twoFactorError,
        await readError(res, "Something went wrong. Please try again.")
      );
    }
  });

  el.twoFactorCancel.addEventListener("click", async () => {
    try {
      await api("/api/auth/logout", { method: "POST" });
    } catch (_err) {
      // Best-effort -- if the server is unreachable there's nothing left
      // to clean up client-side beyond returning to the login view.
    }
    goToLogin();
  });

  // ---------------------------------------------------------------------
  // Lock / logout
  // ---------------------------------------------------------------------
  el.lockBtn.addEventListener("click", async () => {
    try {
      await api("/api/auth/lock", { method: "POST" });
    } catch (_err) {
      // Fall through -- still return to the login view locally.
    }
    goToLogin();
  });

  el.logoutBtn.addEventListener("click", async () => {
    try {
      await api("/api/auth/logout", { method: "POST" });
    } catch (_err) {
      // Fall through -- still return to the login view locally.
    }
    goToLogin();
  });

  // ---------------------------------------------------------------------
  // Vault list
  // ---------------------------------------------------------------------
  function matchesSearch(item, needle) {
    if (!needle) return true;
    const haystack = [item.name, item.username, item.uri || ""]
      .join(" ")
      .toLowerCase();
    return haystack.includes(needle);
  }

  function visibleItems() {
    const needle = el.vaultSearch.value.trim().toLowerCase();
    return vaultItems.filter((item) => matchesSearch(item, needle));
  }

  function renderVaultList() {
    const items = visibleItems();
    el.vaultList.innerHTML = "";

    if (vaultItems.length === 0) {
      const empty = document.createElement("p");
      empty.className = "empty-state";
      empty.textContent =
        "Your vault is empty here. Sync from Bitwarden to bring in your items.";
      el.vaultList.appendChild(empty);
      el.vaultSelectAll.checked = false;
      el.vaultSelectAll.disabled = true;
      return;
    }

    if (items.length === 0) {
      const empty = document.createElement("p");
      empty.className = "empty-state";
      empty.textContent = "No items match your search.";
      el.vaultList.appendChild(empty);
      el.vaultSelectAll.disabled = true;
      return;
    }

    el.vaultSelectAll.disabled = false;
    el.vaultSelectAll.checked = items.every((item) =>
      selectedIds.has(item.id)
    );

    for (const item of items) {
      const row = document.createElement("label");
      row.className = "vault-item";

      const checkbox = document.createElement("input");
      checkbox.type = "checkbox";
      checkbox.checked = selectedIds.has(item.id);
      checkbox.addEventListener("change", () => {
        if (checkbox.checked) {
          selectedIds.add(item.id);
        } else {
          selectedIds.delete(item.id);
        }
        updateSyncHint();
        el.vaultSelectAll.checked = visibleItems().every((i) =>
          selectedIds.has(i.id)
        );
      });

      const body = document.createElement("div");
      body.className = "vault-item-body";

      const name = document.createElement("div");
      name.className = "vault-item-name";
      name.textContent = item.name || "(untitled)";

      const detail = document.createElement("div");
      detail.className = "vault-item-detail";
      const parts = [];
      if (item.username) parts.push(item.username);
      if (item.uri) parts.push(item.uri);
      detail.textContent = parts.length > 0 ? parts.join(" · ") : "No username or URL saved";

      body.appendChild(name);
      body.appendChild(detail);
      row.appendChild(checkbox);
      row.appendChild(body);
      el.vaultList.appendChild(row);
    }
  }

  el.vaultSearch.addEventListener("input", renderVaultList);

  el.vaultSelectAll.addEventListener("change", () => {
    const items = visibleItems();
    if (el.vaultSelectAll.checked) {
      for (const item of items) selectedIds.add(item.id);
    } else {
      for (const item of items) selectedIds.delete(item.id);
    }
    renderVaultList();
    updateSyncHint();
  });

  async function loadVaultList() {
    let res;
    try {
      res = await api("/api/vault/list");
    } catch (_err) {
      showError(el.vaultError, "Couldn't reach the web-companion server.");
      return;
    }

    if (res.status === 409) {
      goToLogin("Your session isn't unlocked anymore. Please log in again.");
      return;
    }
    if (!res.ok) {
      showError(
        el.vaultError,
        await readError(res, "Couldn't load your vault.")
      );
      return;
    }

    hideError(el.vaultError);
    vaultItems = await res.json();
    // Drop selections for ids no longer present (e.g. after a re-sync).
    const currentIds = new Set(vaultItems.map((item) => item.id));
    for (const id of Array.from(selectedIds)) {
      if (!currentIds.has(id)) selectedIds.delete(id);
    }
    renderVaultList();
    updateSyncHint();
  }

  async function refreshVaultMeta() {
    let res;
    try {
      res = await api("/api/vault/status");
    } catch (_err) {
      return;
    }
    if (!res.ok) return;
    const status = await res.json();
    el.vaultMeta.textContent = status.synced
      ? "Last synced " + status.count + " item" + (status.count === 1 ? "" : "s") + " from Bitwarden."
      : "Not synced yet. Sync from Bitwarden to load your vault.";
  }

  async function syncFromBitwarden() {
    hideError(el.vaultError);
    el.vaultRefreshBtn.disabled = true;
    el.vaultRefreshBtn.textContent = "Syncing…";
    try {
      const res = await api("/api/vault/sync", { method: "POST" });
      if (res.status === 409) {
        goToLogin("Your session isn't unlocked anymore. Please log in again.");
        return;
      }
      if (!res.ok) {
        showError(
          el.vaultError,
          await readError(
            res,
            "Couldn't sync your vault from Bitwarden. Please try again."
          )
        );
        return;
      }
      await refreshVaultMeta();
      await loadVaultList();
    } catch (_err) {
      showError(el.vaultError, "Couldn't reach the web-companion server.");
    } finally {
      el.vaultRefreshBtn.disabled = false;
      el.vaultRefreshBtn.textContent = "Sync from Bitwarden";
    }
  }

  el.vaultRefreshBtn.addEventListener("click", syncFromBitwarden);

  // ---------------------------------------------------------------------
  // Devices
  // ---------------------------------------------------------------------
  async function loadDevices() {
    let res;
    try {
      res = await api("/api/devices");
    } catch (_err) {
      showError(el.deviceError, "Couldn't reach the web-companion server.");
      return;
    }

    if (res.status === 409) {
      goToLogin("Your session isn't unlocked anymore. Please log in again.");
      return;
    }
    if (!res.ok) {
      showError(el.deviceError, await readError(res, "Couldn't load devices."));
      return;
    }

    hideError(el.deviceError);
    devices = await res.json();
    el.deviceSelect.innerHTML = "";
    if (devices.length === 0) {
      const option = document.createElement("option");
      option.textContent = "No devices found";
      option.disabled = true;
      el.deviceSelect.appendChild(option);
      el.syncBtn.disabled = true;
      return;
    }
    el.syncBtn.disabled = false;
    for (const device of devices) {
      const option = document.createElement("option");
      option.value = device.id;
      option.textContent = device.name;
      el.deviceSelect.appendChild(option);
    }
  }

  // ---------------------------------------------------------------------
  // Sync to device
  // ---------------------------------------------------------------------
  function updateSyncHint() {
    if (selectedIds.size > 0) {
      el.syncHint.textContent =
        "Will push " + selectedIds.size + " selected item" +
        (selectedIds.size === 1 ? "" : "s") + ".";
    } else {
      el.syncHint.textContent =
        "No items selected — will push everything in your vault (" +
        vaultItems.length + " item" + (vaultItems.length === 1 ? "" : "s") + ").";
    }
  }

  el.syncBtn.addEventListener("click", async () => {
    el.syncResult.classList.add("hidden");
    el.syncResult.classList.remove("is-error");

    const targetId = el.deviceSelect.value;
    if (!targetId) {
      showError(el.deviceError, "Choose a target device first.");
      return;
    }
    hideError(el.deviceError);

    const itemIds = selectedIds.size > 0 ? Array.from(selectedIds) : null;

    el.syncBtn.disabled = true;
    el.syncBtn.textContent = "Syncing…";
    try {
      const res = await api("/api/sync", {
        method: "POST",
        body: JSON.stringify({ target_id: targetId, item_ids: itemIds }),
      });

      if (res.status === 409) {
        goToLogin("Your session isn't unlocked anymore. Please log in again.");
        return;
      }

      if (res.ok) {
        const body = await res.json();
        el.syncResult.textContent =
          "Pushed " + body.pushed + " item" + (body.pushed === 1 ? "" : "s") +
          " to " + body.device.name + ".";
        el.syncResult.classList.remove("hidden");
        return;
      }

      let message = "Sync failed. Please try again.";
      if (res.status === 404) {
        message = "That device wasn't found. Try refreshing the device list.";
      } else if (res.status === 502) {
        message = "Couldn't reach the device — is the emulator running?";
      } else {
        message = await readError(res, message);
      }
      el.syncResult.textContent = message;
      el.syncResult.classList.remove("hidden");
      el.syncResult.classList.add("is-error");
    } catch (_err) {
      el.syncResult.textContent = "Couldn't reach the web-companion server.";
      el.syncResult.classList.remove("hidden");
      el.syncResult.classList.add("is-error");
    } finally {
      el.syncBtn.disabled = false;
      el.syncBtn.textContent = "Sync to device";
    }
  });

  // ---------------------------------------------------------------------
  // Entering the authenticated app view
  // ---------------------------------------------------------------------
  async function enterApp() {
    showView("app");
    el.statusText.textContent = "Unlocked";
    resetClientVaultState();
    await refreshVaultMeta();
    await loadVaultList();
    await loadDevices();
    updateSyncHint();

    // First time in (nothing synced yet this process lifetime) -- sync
    // automatically so the vault isn't a confusing empty list on first
    // login. `vault-refresh-btn` remains available for manual re-sync
    // afterwards.
    if (vaultItems.length === 0) {
      let statusRes;
      try {
        statusRes = await api("/api/vault/status");
      } catch (_err) {
        return;
      }
      if (statusRes.ok) {
        const status = await statusRes.json();
        if (!status.synced) {
          await syncFromBitwarden();
        }
      }
    }
  }

  // ---------------------------------------------------------------------
  // Bootstrap -- figure out which view to show on page load by asking the
  // server what session state it's actually in (the server, not the
  // browser, is the source of truth -- see module docs).
  // ---------------------------------------------------------------------
  async function bootstrap() {
    let res;
    try {
      res = await api("/api/auth/status");
    } catch (_err) {
      goToLogin("Couldn't reach the web-companion server. Is it running?");
      return;
    }

    if (!res.ok) {
      goToLogin();
      return;
    }

    const body = await res.json();
    switch (body.status) {
      case "unlocked":
        await enterApp();
        break;
      case "two_factor_required":
        // The original login response (with its provider list) is long
        // gone after a fresh page load -- fall back to the full method
        // list. See populateProviderSelect.
        populateProviderSelect(null);
        showView("twofactor");
        break;
      case "locked":
        goToLogin("Your vault is locked. Please log in again.");
        break;
      case "logged_out":
      default:
        goToLogin();
        break;
    }
  }

  bootstrap();
})();
