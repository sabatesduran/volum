# Volum website

Static product website and VitePress documentation for Volum. The landing page has no runtime framework or remotely hosted visual assets. The production build uses PostHog product analytics through the same first-party proxy and project as didac.dev; this is separate from the Volum apps and never receives library or model data.

Copy `.env.example` to `.env.local` and provide the shared PostHog project token and ingestion host to enable analytics in production builds. Analytics stay disabled during local development.

## Development

From the repository root:

```bash
npm run website:dev
```

This starts the landing page and documentation together. Open `http://localhost:5173/` for the website or `http://localhost:5173/docs/` for the documentation.

Build the deployable site:

```bash
npm run website:build
```

The combined output is written to `website/dist/`. The landing page is at `/` and the documentation is at `/docs/`.

Run the documentation site by itself with:

```bash
npm run website:docs:dev
```

The docs-only command opens `http://localhost:5173/docs/`. Visiting that server's root redirects to the documentation route.

Public documentation source files live in `website/docs/`. Internal implementation and coverage plans remain in the repository-level `docs/` directory and are not part of the public build.

## Sharing image

The editable 1200 × 630 source is `assets/og-image-source.svg`. Its JPEG export is kept at `public/og-image.jpg` so link preview crawlers always receive a stable, absolute URL.

## Deploying to Vercel

The Vercel project uses `website/` as its root directory. Production deployments can be published from the repository root with:

```bash
npx vercel@latest --prod
```

The canonical site is [volum.didac.dev](https://volum.didac.dev), with [volum-three.vercel.app](https://volum-three.vercel.app) as the Vercel fallback URL.

## Product screenshots

Screenshots under `assets/screenshots/` are captured from the live Tauri app and compressed to WebP. The hero uses the Batraci Mini 3MF project, as requested.
