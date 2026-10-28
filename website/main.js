// Point the download buttons at the latest release's files and highlight
// the visitor's platform. Without JS (or if GitHub's API is unreachable)
// every button still goes to the latest release page.
(function () {
  var REPO = "sebastian-beca/opendj";
  var ua = navigator.userAgent;
  var os = /Mac/.test(ua) && !/iPhone|iPad/.test(ua) ? "mac" : /Linux/.test(ua) && !/Android/.test(ua) ? "linux" : /Windows/.test(ua) ? "windows" : null;

  if (os) {
    var card = document.querySelector('.dl-card[data-os="' + os + '"]');
    if (card) card.classList.add("is-mine");
  }
  // On Windows the main button keeps its generic label until a release has
  // the Windows download (see showWindows).
  if (os && os !== "windows") {
    var primary = document.querySelector("[data-download-primary]");
    if (primary) primary.textContent = os === "mac" ? "Download for Mac" : "Download for Linux";
  }

  var patterns = {
    macos: /macos.*\.zip$/i,
    appimage: /\.AppImage$/,
    tarball: /linux.*\.tar\.gz$/i,
    winsetup: /windows.*setup\.exe$/i,
    winzip: /windows.*\.zip$/i
  };

  function showWindows() {
    var button = document.querySelector("[data-windows-button]");
    if (button) {
      button.textContent = "Download for Windows";
      button.classList.replace("btn-ghost", "btn-primary");
    }
    document.querySelectorAll("[data-windows-zip]").forEach(function (el) { el.hidden = false; });
    document.querySelectorAll("[data-windows-note]").forEach(function (el) {
      el.textContent = "Builds aren't signed yet, so Windows asks before the first start: More info → Run anyway.";
    });
    if (os === "windows") {
      var primary = document.querySelector("[data-download-primary]");
      if (primary) primary.textContent = "Download for Windows";
    }
  }

  fetch("https://api.github.com/repos/" + REPO + "/releases/latest")
    .then(function (r) { return r.ok ? r.json() : Promise.reject(r.status); })
    .then(function (rel) {
      var version = (rel.tag_name || "").replace(/^v/, "");
      if (version) {
        document.querySelectorAll("[data-version]").forEach(function (el) { el.textContent = "v" + version + " available"; });
        document.querySelectorAll("[data-version-text]").forEach(function (el) { el.textContent = "Version " + version + "."; });
      }
      (rel.assets || []).forEach(function (asset) {
        Object.keys(patterns).forEach(function (key) {
          if (patterns[key].test(asset.name)) {
            if (key === "winsetup") showWindows();
            document.querySelectorAll('[data-asset="' + key + '"]').forEach(function (a) { a.href = asset.browser_download_url; });
          }
        });
      });
    })
    .catch(function () {});
})();
