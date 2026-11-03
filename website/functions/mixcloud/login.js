import { AUTHORIZE, callbackUrl, cookie, page, redirect, validPort, validState } from "./_shared.js";

// GET /mixcloud/login?port=N&state=S: off to Mixcloud to sign in.
export function onRequestGet({ request, env }) {
  const url = new URL(request.url);
  const port = validPort(url.searchParams.get("port"));
  const state = validState(url.searchParams.get("state"));
  if (!port || !state) return page(400, "Can't sign in", "Start signing in from openDJ.");
  if (!env.MIXCLOUD_CLIENT_ID) {
    return page(503, "Not set up yet", "Mixcloud sign-in isn't set up on opendj.app yet.");
  }
  const to = new URL(AUTHORIZE);
  to.searchParams.set("client_id", env.MIXCLOUD_CLIENT_ID);
  to.searchParams.set("redirect_uri", callbackUrl(url));
  return redirect(to.toString(), cookie(`${port}.${state}`, 900));
}
