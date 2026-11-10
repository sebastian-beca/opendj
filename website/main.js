// Point the download buttons at the latest release's files and highlight
// the visitor's platform. Without JS (or if GitHub's API is unreachable)
// every button still goes to the latest release page.
(function () {
  var REPO = "openwave-studio/opendj";
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
    deb: /\.deb$/i,
    rpm: /\.rpm$/i,
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

// Feature rows fade in, and screenshots zoom towards their focus point
// (--fx/--fy) as they scroll up the screen.
(function () {
  if (!("IntersectionObserver" in window)) return;
  document.documentElement.classList.add("js");
  var io = new IntersectionObserver(function (entries) {
    entries.forEach(function (e) {
      if (e.isIntersecting) { e.target.classList.add("in"); io.unobserve(e.target); }
    });
  }, { rootMargin: "0px 0px -10% 0px" });
  document.querySelectorAll(".reveal").forEach(function (el) { io.observe(el); });

  if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
  var frames = Array.prototype.slice.call(document.querySelectorAll("[data-zoom]"));
  var queued = false;
  function update() {
    queued = false;
    var vh = window.innerHeight;
    frames.forEach(function (f) {
      var r = f.getBoundingClientRect();
      if (r.bottom < 0 || r.top > vh) return;
      // 0 while the frame's centre is low on the screen, 1 once it reaches
      // the upper third, so the zoom has finished by the time it's read.
      var centre = r.top + r.height / 2;
      var p = Math.min(1, Math.max(0, (vh * 0.85 - centre) / (vh * 0.5)));
      p = p * p * (3 - 2 * p);
      f.style.setProperty("--z", (1 + (parseFloat(f.dataset.zoom) - 1) * p).toFixed(4));
    });
  }
  function queue() { if (!queued) { queued = true; requestAnimationFrame(update); } }
  window.addEventListener("scroll", queue, { passive: true });
  window.addEventListener("resize", queue);
  update();
})();
