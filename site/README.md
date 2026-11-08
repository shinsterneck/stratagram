# Stratagram's homepage

A static page: `index.html`, `style.css`, `logo.svg` and the screenshots in
`shots/`. No build step; open `index.html` or serve the folder:

```sh
python3 -m http.server -d site 8000
```

It is published with GitHub Pages at <https://shinsterneck.github.io/stratagram/>
by `.github/workflows/pages.yml` on every push to `main` that changes `site/`.
