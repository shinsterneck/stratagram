# Stratagram's homepage

A static page: `index.html`, `style.css`, `logo.svg` and the screenshots in
`shots/`. No build step; open `index.html` or serve the folder:

```sh
python3 -m http.server -d site 8000
```

It is published with GitHub Pages at <https://stratagram.app/> (the custom domain)
by `.github/workflows/pages.yml` on every push to `main` that changes `site/` or the
browser editor. The workflow also builds the browser editor (`web/build.sh`) and puts
`web/www` at `/editor/`, so `editor/#file=../examples/berlin-hq.stratagram` opens an
example from `examples/`. The helpers here (`make-shots.sh`, `netbox-seed.py`, this
README) are left out of the published site.
