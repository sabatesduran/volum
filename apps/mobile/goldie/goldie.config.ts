import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const APP_ROOT = fileURLToPath(new URL("../", import.meta.url)).replace(/\/$/, "");
const captureDevice = process.env.GOLDIE_CAPTURE_DEVICE;
const flowFor = (name: string) => captureDevice === "ipad-13" ? `${name}-ipad` : name;

interface LanguageDefinition {
  code: string;
  capture: string;
}

interface GoldieCopy {
  subtitle: string;
  description: string;
  scenes: Record<string, { headline: string; subhead: string }>;
}

const registry = JSON.parse(readFileSync(`${APP_ROOT}/locales.json`, "utf8")) as { languages: LanguageDefinition[] };
const copy = Object.fromEntries(registry.languages.map((language) => [
  language.code,
  JSON.parse(readFileSync(`${APP_ROOT}/goldie/locales/${language.code}.json`, "utf8")) as GoldieCopy,
])) as Record<string, GoldieCopy>;
const localized = (select: (value: GoldieCopy) => string) => Object.fromEntries(
  registry.languages.map((language) => [language.capture, select(copy[language.code])]),
);

const sceneDefinitions = [
  {
    id: "library", flow: "store-01-library", template: "hero",
    background: "linear-gradient(145deg, #06111D 0%, #0B2338 56%, #174E70 100%)",
    orbit: { src: "decor/orbit-blue.png", x: 0.55, y: 0.19, width: 0.64, rotate: -12 },
  },
  {
    id: "detail", flow: "store-02-detail", template: "offset",
    background: "linear-gradient(150deg, #07111B 0%, #172039 58%, #40304B 100%)",
    orbit: { src: "decor/orbit-orange.png", x: -0.19, y: 0.47, width: 0.62, rotate: 18 },
  },
  {
    id: "filters", flow: "store-03-filters", template: "copy-below",
    background: "linear-gradient(160deg, #07141E 0%, #103747 52%, #17635C 100%)",
    orbit: { src: "decor/orbit-blue.png", x: 0.53, y: 0.62, width: 0.58, rotate: 24 },
  },
  {
    id: "collections", flow: "store-04-collections", template: "offset",
    background: "linear-gradient(145deg, #0B1220 0%, #24203B 54%, #572B3C 100%)",
    orbit: { src: "decor/orbit-orange.png", x: 0.59, y: 0.08, width: 0.55, rotate: -24 },
  },
  {
    id: "photo-room", flow: "store-05-photo-room", template: "tilt-right",
    background: "linear-gradient(145deg, #08111E 0%, #321D3C 54%, #713423 100%)",
    orbit: { src: "decor/orbit-orange.png", x: -0.15, y: 0.14, width: 0.62, rotate: 8 },
  },
] as const;

const config = {
  appRoot: APP_ROOT,
  appPath: `${APP_ROOT}/src-tauri/gen/apple/build/arm64-sim/Volum.app`,
  bundleId: "com.dsd.volum.companion",
  devices: ["iphone-6.9", "ipad-13"],
  locales: registry.languages.map((language) => language.capture),
  localizedCaptures: true,
  appearance: "light",
  frame: {
    variant: {
      "iphone-6.9": "17-pro-blue",
      "ipad-13": "ipad-pro-13-space-gray",
    },
  },
  theme: {
    background: "linear-gradient(145deg, #06111D 0%, #0B2338 58%, #174E70 100%)",
    headlineColor: "#F7FBFF",
    subheadColor: "#B8C9D8",
    fontFamily: '"DM Sans", -apple-system, system-ui, sans-serif',
    copyHeightRatio: 0.22,
    deviceWidthRatio: 0.88,
    template: sceneDefinitions.map((scene) => scene.template),
    layout: "classic",
    decorations: [
      { kind: "image", src: "decor/technical-grid.png", x: 0, y: 0, width: 1 },
      { kind: "image", src: "decor/technical-grid.png", x: 0, y: 0.54, width: 1, rotate: 180 },
    ],
  },
  store: {
    name: "Volum: 3D File Library",
    subtitle: localized((value) => value.subtitle),
    developer: "Dídac Sabatés Duran",
    category: "Productivity",
    ageRating: "4+",
    price: "Free",
    description: localized((value) => value.description),
  },
  scenes: sceneDefinitions.map((scene) => ({
    kind: "screenshot",
    id: scene.id,
    flow: flowFor(scene.flow),
    headline: localized((value) => value.scenes[scene.id].headline),
    subhead: localized((value) => value.scenes[scene.id].subhead),
    background: scene.background,
    secondScene: "secondScene" in scene ? scene.secondScene : undefined,
    decorations: [
      { kind: "image", ...scene.orbit },
    ],
  })),
};

export default config;
