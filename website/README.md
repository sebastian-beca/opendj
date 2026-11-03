# opendj.app

The openDJ website: a static landing page, a tutorials page and the docs, which
are an [mdBook](https://rust-lang.github.io/mdBook/) in `book/`.

## Preview

```sh
./build.sh                       # builds everything into dist/
python3 -m http.server -d dist   # then open http://localhost:8000
```

For the docs alone, `mdbook serve book --open` reloads as you edit.

## Deploy (Cloudflare Pages, free plan)

Connect the repository in Cloudflare Pages with:

| Setting | Value |
|---|---|
| Production branch | `main` |
| Framework preset | None |
| Root directory | `website` |
| Build command | `./build.sh` |
| Build output directory | `dist` |

`build.sh` downloads mdBook if the build image doesn't have it. Add
`opendj.app` under the project's Custom domains. `_headers` sets caching and
security headers, and `404.html` is the not-found page.

The Download buttons link to the latest GitHub release; `main.js` swaps in the
direct file links from GitHub's API when it can.

## Mixcloud sign-in

`functions/mixcloud/` holds two Cloudflare Pages Functions that let the app
sign DJs in to their own Mixcloud accounts without shipping the app's client
secret: `/mixcloud/login` sends the DJ to Mixcloud, `/mixcloud/callback`
swaps the code for a token and hands it to the app on `127.0.0.1`. They need
two variables in the Pages project (Settings › Variables and Secrets, for
Production):

| Variable | Value |
|---|---|
| `MIXCLOUD_CLIENT_ID` | the client id of the openDJ app on mixcloud.com/developers |
| `MIXCLOUD_CLIENT_SECRET` | its client secret (type: Secret) |

Register the app at https://www.mixcloud.com/developers/create/ with the
redirect URI `https://opendj.app/mixcloud/callback`. Until the variables are
set, the sign-in pages say it isn't set up yet.

## Controller pages

`tools/controllers.py` writes a docs page for every mapping in
`res/controllers` when the site builds, plus the docs' `SUMMARY.md` from
`book/src/SUMMARY.in.md`. Lines that drive a control openDJ doesn't have yet
are marked using `book/supported-controls.txt`. CI fails when that list is out
of date; regenerate it from the repository root with:

```sh
cargo run -q -p opendj -- --list-controls > website/book/supported-controls.txt
```
