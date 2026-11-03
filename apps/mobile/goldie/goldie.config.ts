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
  { id: "library", flow: "store-01-library", template: "hero" },
  { id: "detail", flow: "store-02-detail", template: "offset" },
  { id: "filters", flow: "store-03-filters", template: "copy-below" },
  { id: "collections", flow: "store-04-collections", template: "minimal" },
  { id: "private", flow: "store-05-private", template: "tilt-right" },
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
    background: "linear-gradient(155deg, #F4F1EA 0%, #FFF8F2 58%, #FFD8CC 100%)",
    headlineColor: "#191916",
    subheadColor: "#68665F",
    fontFamily: '-apple-system, "SF Pro Display", system-ui, sans-serif',
    copyHeightRatio: 0.23,
    deviceWidthRatio: 0.84,
    template: sceneDefinitions.map((scene) => scene.template),
    layout: "classic",
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
  })),
};

export default config;
