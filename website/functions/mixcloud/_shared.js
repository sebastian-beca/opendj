// Mixcloud sign-in for the openDJ app. Mixcloud's token exchange needs the
// app's client secret and has no desktop flow, so the secret stays here, in
// the Cloudflare Pages project's environment variables:
//
//   MIXCLOUD_CLIENT_ID      the app's client id (public)
//   MIXCLOUD_CLIENT_SECRET  the app's client secret (keep it encrypted)
//
// The Mixcloud app's redirect URI is https://opendj.app/mixcloud/callback.
//
// The app listens on 127.0.0.1 and opens /mixcloud/login?port=<its port>
// &state=<a one-off check>. Those two ride in a short-lived cookie while
// the DJ signs in on mixcloud.com, so the redirect URI stays fixed. Mixcloud
// sends the DJ back to /mixcloud/callback, which swaps the code for a token
// and hands it, with the check, to the app on 127.0.0.1. Nothing is stored.

export const AUTHORIZE = "https://www.mixcloud.com/oauth/authorize";
export const TOKEN = "https://www.mixcloud.com/oauth/access_token";
export const COOKIE = "opendj_mixcloud";

// The app's loopback port, or null if it isn't one.
export function validPort(raw) {
  if (!/^\d{4,5}$/.test(raw || "")) return null;
  const port = Number(raw);
  return port >= 1024 && port <= 65535 ? port : null;
}

// The app's one-off check, or null if it isn't one.
export function validState(raw) {
  return /^[A-Za-z0-9_-]{16,64}$/.test(raw || "") ? raw : null;
}

export function callbackUrl(url) {
  return `${url.origin}/mixcloud/callback`;
}

// The port and check the login step left in the cookie.
export function signIn(request) {
  const cookies = request.headers.get("cookie") || "";
  const found = cookies
    .split(";")
    .map((c) => c.trim())
    .find((c) => c.startsWith(`${COOKIE}=`));
  if (!found) return null;
  const [port, state] = found.slice(COOKIE.length + 1).split(".");
  const p = validPort(port);
  const s = validState(state);
  return p && s ? { port: p, state: s } : null;
}

export function cookie(value, maxAge) {
  return `${COOKIE}=${value}; Path=/mixcloud; Max-Age=${maxAge}; Secure; HttpOnly; SameSite=Lax`;
}

export function redirect(to, setCookie) {
  return new Response(null, {
    status: 302,
    headers: { location: to, "set-cookie": setCookie, "cache-control": "no-store" },
  });
}

export function page(status, title, text) {
  const html = `<!doctype html><html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${title} · openDJ</title>
<style>body{font:16px/1.5 system-ui,sans-serif;background:#101112;color:#eee;
display:grid;place-items:center;min-height:100vh;margin:0}
main{max-width:28rem;padding:2rem}h1{color:#B69CFF;font-size:1.4rem}</style>
</head><body><main><h1>${title}</h1><p>${text}</p></main></body></html>`;
  return new Response(html, {
    status,
    headers: { "content-type": "text/html; charset=utf-8", "cache-control": "no-store" },
  });
}
