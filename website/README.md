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
