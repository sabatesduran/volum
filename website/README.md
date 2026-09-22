# Volum website

Static product website for Volum. It has no runtime framework or remotely hosted visual assets. The production build uses PostHog product analytics through the same first-party proxy and project as didac.dev; this is separate from the Volum desktop app and never receives library or model data.

Copy `.env.example` to `.env.local` and provide the shared PostHog project token and ingestion host to enable analytics in production builds. Analytics stay disabled during local development.

## Development

From the repository root:

```bash
npm run website:dev
```

Build the deployable site:

```bash
npm run website:build
```

The output is written to `website-dist/`.

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
