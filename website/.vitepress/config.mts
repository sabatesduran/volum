import { defineConfig } from "vitepress";

const repository = "https://github.com/sabatesduran/volum";

export default defineConfig({
  title: "Volum documentation",
  description: "Install, connect, and self-host your private Volum 3D-printing library.",
  lang: "en-US",
  base: "/docs/",
  srcDir: "./docs",
  cleanUrls: true,
  markdown: {
    theme: {
      light: "github-light-high-contrast",
      dark: "github-dark-high-contrast",
    },
  },
  lastUpdated: true,
  outDir: "./dist/docs",
  sitemap: {
    hostname: "https://volum.didac.dev/docs/",
  },
  vite: {
    plugins: [
      {
        name: "volum-docs-dev-redirect",
        configureServer(server) {
          server.middlewares.use((request, response, next) => {
            if (request.url === "/") {
              response.statusCode = 302;
              response.setHeader("Location", "/docs/");
              response.end();
              return;
            }
            next();
          });
        },
      },
    ],
  },
  head: [
    ["link", { rel: "icon", href: "/docs/favicon.png" }],
    ["meta", { name: "color-scheme", content: "light dark" }],
    ["meta", { name: "theme-color", content: "#f1eee6", media: "(prefers-color-scheme: light)" }],
    ["meta", { name: "theme-color", content: "#0d0e0c", media: "(prefers-color-scheme: dark)" }],
    ["meta", { property: "og:site_name", content: "Volum documentation" }],
    ["meta", { property: "og:image", content: "https://volum.didac.dev/og-image.jpg" }],
  ],
  themeConfig: {
    logo: "/app-icon.png",
    siteTitle: "Volum docs",
    nav: [
      { text: "Start here", link: "/getting-started/introduction" },
      { text: "Companion", link: "/companion/overview" },
      { text: "Self-host", link: "/server/" },
      { text: "Reference", link: "/reference/architecture" },
      { text: "Volum", link: "https://volum.didac.dev/" },
    ],
    sidebar: {
      "/getting-started/": [
        {
          text: "Start here",
          items: [
            { text: "Introduction", link: "/getting-started/introduction" },
            { text: "Choose a setup", link: "/getting-started/choose-a-setup" },
            { text: "Install Desktop", link: "/getting-started/install-desktop" },
            { text: "Create your first library", link: "/getting-started/first-library" },
          ],
        },
      ],
      "/desktop/": [
        {
          text: "Volum Desktop",
          items: [
            { text: "Linked and managed libraries", link: "/desktop/libraries" },
            { text: "Companion Access", link: "/desktop/companion-access" },
            { text: "Connect to another server", link: "/desktop/remote-desktop" },
            { text: "Move a library to NAS", link: "/desktop/move-library-to-nas" },
            { text: "Managed mirror", link: "/desktop/managed-mirror" },
            { text: "Menu bar and system tray", link: "/desktop/menu-bar" },
          ],
        },
      ],
      "/companion/": [
        {
          text: "Volum on mobile",
          items: [
            { text: "Overview", link: "/companion/overview" },
            { text: "Install the beta", link: "/companion/install" },
            { text: "Choose a network", link: "/companion/networking" },
            { text: "Pair a device", link: "/companion/pair" },
            { text: "Browse and organize", link: "/companion/use" },
            { text: "Create photos and videos", link: "/companion/photo-room" },
            { text: "Offline projects", link: "/companion/offline" },
            { text: "Photos and file uploads", link: "/companion/uploads" },
          ],
        },
      ],
      "/server/": [
        {
          text: "Self-host Volum",
          items: [
            { text: "Server overview", link: "/server/" },
            { text: "Install with Docker", link: "/server/docker" },
            { text: "UGREEN NAS", link: "/server/ugreen" },
            { text: "Networking", link: "/server/networking" },
            { text: "Storage and backups", link: "/server/storage-and-backups" },
            { text: "Update and recover", link: "/server/update-and-recover" },
          ],
        },
      ],
      "/help/": [
        {
          text: "Help",
          items: [
            { text: "Troubleshooting", link: "/help/troubleshooting" },
            { text: "Report a security issue", link: "/help/security" },
          ],
        },
      ],
      "/reference/": [
        {
          text: "Reference",
          items: [
            { text: "Architecture", link: "/reference/architecture" },
            { text: "Security model", link: "/reference/security" },
            { text: "Compatibility", link: "/reference/compatibility" },
            { text: "Server environment", link: "/reference/server-environment" },
          ],
        },
      ],
      "/development/": [
        {
          text: "Development",
          items: [
            { text: "Development overview", link: "/development/" },
            { text: "Mobile development", link: "/development/mobile" },
            { text: "Server development", link: "/development/server" },
          ],
        },
      ],
    },
    search: {
      provider: "local",
    },
    outline: {
      level: [2, 3],
      label: "On this page",
    },
    editLink: {
      pattern: `${repository}/edit/main/website/docs/:path`,
      text: "Edit this page on GitHub",
    },
    socialLinks: [
      { icon: "github", link: repository },
    ],
    footer: {
      message: "Free, private, local-first, and built for makers.",
      copyright: "Volum is licensed under GPL-3.0-or-later.",
    },
  },
});
