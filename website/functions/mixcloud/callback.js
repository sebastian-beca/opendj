import { TOKEN, callbackUrl, cookie, page, redirect, signIn } from "./_shared.js";

// GET /mixcloud/callback?code=...: swaps the code for a token and passes it
// to the app on 127.0.0.1.
export async function onRequestGet({ request, env }) {
  const url = new URL(request.url);
  const started = signIn(request);
  if (!started) {
    return page(400, "Can't sign in", "This sign-in has expired. Start again from openDJ.");
  }
  const back = new URL(`http://127.0.0.1:${started.port}/mixcloud`);
  back.searchParams.set("state", started.state);
  const forget = cookie("", 0);
  const code = url.searchParams.get("code");
  if (!code) {
    back.searchParams.set("error", url.searchParams.get("error") || "cancelled");
    return redirect(back.toString(), forget);
  }
  if (!env.MIXCLOUD_CLIENT_ID || !env.MIXCLOUD_CLIENT_SECRET) {
    return page(503, "Not set up yet", "Mixcloud sign-in isn't set up on opendj.app yet.");
  }
  const swap = new URL(TOKEN);
  swap.searchParams.set("client_id", env.MIXCLOUD_CLIENT_ID);
  swap.searchParams.set("client_secret", env.MIXCLOUD_CLIENT_SECRET);
  swap.searchParams.set("redirect_uri", callbackUrl(url));
  swap.searchParams.set("code", code);
  const res = await fetch(swap.toString());
  const body = await res.text();
  // Mixcloud answers JSON ({"access_token": ...}) or a form body.
  let token = null;
  try {
    token = JSON.parse(body).access_token || null;
  } catch {
    token = new URLSearchParams(body).get("access_token");
  }
  if (!res.ok || !token) {
    back.searchParams.set("error", "Mixcloud didn't give a sign-in token");
  } else {
    back.searchParams.set("token", token);
  }
  return redirect(back.toString(), forget);
}
