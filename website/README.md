# Volum website

Static product website for Volum. It has no runtime framework, analytics, trackers, or third-party assets.

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

## Deploying to Vercel

The Vercel project uses `website/` as its root directory. Production deployments can be published from the repository root with:

```bash
npx vercel@latest --prod
```

The canonical site is [volum.didac.dev](https://volum.didac.dev), with [volum-three.vercel.app](https://volum-three.vercel.app) as the Vercel fallback URL.

## Product screenshots

Screenshots under `assets/screenshots/` are captured from the live Tauri app and compressed to WebP. The hero uses the Batraci Mini 3MF project, as requested.
