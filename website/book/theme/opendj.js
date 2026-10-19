// Replace the book title in the top bar with the openDJ logo, linking home.
(function () {
  var title = document.querySelector(".menu-title");
  if (!title) return;
  var a = document.createElement("a");
  a.className = "opendj-home";
  a.href = "/";
  a.innerHTML = '<img src="/assets/logo.svg" alt=""><span>openDJ</span><small>Docs</small>';
  title.parentNode.insertBefore(a, title);
})();
