import { fileURLToPath } from "node:url";

const APP_ROOT = fileURLToPath(new URL("../", import.meta.url)).replace(/\/$/, "");
const captureDevice = process.env.GOLDIE_CAPTURE_DEVICE;
const flowFor = (name: string) => captureDevice === "ipad-13" ? `${name}-ipad` : name;

const config = {
  appRoot: APP_ROOT,
  appPath: `${APP_ROOT}/src-tauri/gen/apple/build/arm64-sim/Volum.app`,
  bundleId: "com.dsd.volum.companion",
  devices: ["iphone-6.9", "ipad-13"],
  locales: ["en-US", "ca-ES", "es-ES"],
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
    template: ["hero", "offset", "copy-below", "minimal", "tilt-right"],
    layout: "classic",
  },
  store: {
    name: "Volum: 3D File Library",
    subtitle: {
      "en-US": "Your 3D workshop library",
      "ca-ES": "La biblioteca 3D del taller",
      "es-ES": "Tu biblioteca 3D del taller",
    },
    developer: "Dídac Sabatés Duran",
    category: "Productivity",
    ageRating: "4+",
    price: "Free",
    description: {
      "en-US": "Browse your private Volum library from the workshop. Find projects, inspect files and 3D details, and keep every build close without a Volum account or cloud relay.",
      "ca-ES": "Consulta la teva biblioteca privada de Volum des del taller. Troba projectes, revisa fitxers i detalls 3D i tingues cada muntatge a mà sense compte ni relay al núvol de Volum.",
      "es-ES": "Consulta tu biblioteca privada de Volum desde el taller. Encuentra proyectos, revisa archivos y detalles 3D y ten cada montaje a mano sin cuenta ni relay en la nube de Volum.",
    },
  },
  scenes: [
    {
      kind: "screenshot",
      id: "library",
      flow: flowFor("store-01-library"),
      headline: {
        "en-US": "Your workshop library",
        "ca-ES": "El teu taller, a mà",
        "es-ES": "Tu taller, a mano",
      },
      subhead: {
        "en-US": "Browse every project, file, and favorite from the bench.",
        "ca-ES": "Consulta projectes, fitxers i preferits des del banc de treball.",
        "es-ES": "Consulta proyectos, archivos y favoritos desde el banco de trabajo.",
      },
    },
    {
      kind: "screenshot",
      id: "detail",
      flow: flowFor("store-02-detail"),
      headline: {
        "en-US": "Every detail, at hand",
        "ca-ES": "Cada detall, a mà",
        "es-ES": "Cada detalle, a mano",
      },
      subhead: {
        "en-US": "Files, dimensions, notes, parts, and 3D previews in one place.",
        "ca-ES": "Fitxers, mides, notes, peces i vistes 3D en un sol lloc.",
        "es-ES": "Archivos, medidas, notas, piezas y vistas 3D en un solo lugar.",
      },
    },
    {
      kind: "screenshot",
      id: "filters",
      flow: flowFor("store-03-filters"),
      headline: {
        "en-US": "Find the right print",
        "ca-ES": "Troba la impressió ideal",
        "es-ES": "Encuentra la impresión ideal",
      },
      subhead: {
        "en-US": "Search, filter, and sort hundreds of projects in seconds.",
        "ca-ES": "Cerca, filtra i ordena centenars de projectes en segons.",
        "es-ES": "Busca, filtra y ordena cientos de proyectos en segundos.",
      },
    },
    {
      kind: "screenshot",
      id: "collections",
      flow: flowFor("store-04-collections"),
      headline: {
        "en-US": "Keep builds organized",
        "ca-ES": "Muntatges ben organitzats",
        "es-ES": "Montajes bien organizados",
      },
      subhead: {
        "en-US": "Group parts and ideas without moving a single file.",
        "ca-ES": "Agrupa peces i idees sense moure cap fitxer.",
        "es-ES": "Agrupa piezas e ideas sin mover ningún archivo.",
      },
    },
    {
      kind: "screenshot",
      id: "private",
      flow: flowFor("store-05-private"),
      headline: {
        "en-US": "Private by design",
        "ca-ES": "Privat per disseny",
        "es-ES": "Privado por diseño",
      },
      subhead: {
        "en-US": "Connect directly to Desktop—no account, cloud, or public relay.",
        "ca-ES": "Connecta directament amb l'ordinador, sense compte, núvol ni relay públic.",
        "es-ES": "Conecta directamente con el ordenador, sin cuenta, nube ni relay público.",
      },
    },
  ],
};

export default config;
