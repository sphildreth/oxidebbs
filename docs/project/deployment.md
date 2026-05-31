# Documentation Deployment

The public documentation site is built with VitePress from the `docs/`
directory and deployed to GitHub Pages.

## Local Build

```bash
npm ci
npm run docs:build
npm run docs:preview
```

The production output is `docs/.vitepress/dist`.

## GitHub Pages

The deployment workflow is `.github/workflows/pages.yml`. It builds the
VitePress site and uploads `docs/.vitepress/dist` to GitHub Pages.

Repository settings must use:

- Pages source: GitHub Actions
- Custom domain: `oxidebbs.com`

DNS for `oxidebbs.com` must point to GitHub Pages. For an apex domain, use the
records recommended by GitHub Pages for apex domains. For a `www` subdomain, use
a CNAME pointing at the repository's GitHub Pages host.
