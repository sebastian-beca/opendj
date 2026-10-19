# opendj.app

The openDJ website: a static landing page, a tutorials page and the docs, which
are an [mdBook](https://rust-lang.github.io/mdBook/) in `book/`.

## Preview

```sh
./build.sh                       # builds everything into dist/
python3 -m http.server -d dist   # then open http://localhost:8000
```

For the docs alone, `mdbook serve book --open` reloads as you edit.

## Deploy (Cloudflare Workers, free plan)

The site is a Worker that only serves static files (`wrangler.jsonc`), which
is free and unlimited on Cloudflare's free plan. Connect the repository under
Workers & Pages with:

| Setting | Value |
|---|---|
| Production branch | `main` |
| Root directory | `website` |
| Build command | `./build.sh` |
| Deploy command | `npx wrangler deploy` |

`build.sh` downloads mdBook if the build image doesn't have it. Add
`opendj.app` under the Worker's Domains. `_headers` sets caching and security
headers, and `404.html` is the not-found page.

The Download buttons link to the latest GitHub release; `main.js` swaps in the
direct file links from GitHub's API when it can.
