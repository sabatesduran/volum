<script setup lang="ts">
import libraryMacbookScreenshot from "../../assets/screenshots/desktop-library-macbook-pro-m5.webp";
import projectMacbookScreenshot from "../../assets/screenshots/desktop-project-macbook-pro-m5.webp";
import mobileLibraryScreenshot from "../../assets/screenshots/mobile-library-iphone-17-pro-orange.webp";
import mobileProjectScreenshot from "../../assets/screenshots/mobile-project-detail-iphone-17-pro-orange.webp";
</script>

<template>
  <main class="docs-landing">
    <section class="docs-hero" aria-labelledby="docs-hero-title">
      <div class="docs-hero__copy">
        <p class="docs-overline"><span aria-hidden="true"></span> Volum documentation</p>
        <h1 id="docs-hero-title">Your private 3D library, <strong>properly organized.</strong></h1>
        <p class="docs-hero__lede">
          Start on one computer, bring projects to the workbench, or run an always-on library on hardware you control.
        </p>
        <div class="docs-actions">
          <a class="docs-button docs-button--primary" href="/docs/getting-started/introduction">
            Start with Desktop <span aria-hidden="true">→</span>
          </a>
          <a class="docs-button docs-button--secondary" href="/docs/server/docker">Install with Docker</a>
        </div>
        <ul class="docs-principles" aria-label="Volum principles">
          <li><span aria-hidden="true"></span>No account</li>
          <li><span aria-hidden="true"></span>No hosted library</li>
          <li><span aria-hidden="true"></span>Your storage</li>
        </ul>
      </div>

      <figure class="docs-hero__visual">
        <div class="docs-product-frame">
          <img
            :src="libraryMacbookScreenshot"
            alt="Volum Desktop showing a searchable library of 3D-printing projects in a MacBook Pro"
            fetchpriority="high"
          >
        </div>
        <figcaption>
          <span>Desktop</span>
          Index the folders you already have. Volum adds previews and organization without moving linked files.
        </figcaption>
      </figure>
    </section>

    <section class="docs-paths" aria-labelledby="docs-paths-title">
      <div class="docs-section-heading">
        <p class="docs-overline"><span aria-hidden="true"></span> Choose a setup</p>
        <h2 id="docs-paths-title">One library. Three ways to use it.</h2>
        <p>Begin with Desktop. Add mobile or a dedicated server only when your workflow needs them.</p>
      </div>

      <div class="docs-path-ledger">
        <a class="docs-path" href="/docs/getting-started/install-desktop">
          <header>
            <span class="docs-path__icon" aria-hidden="true">
              <svg viewBox="0 0 24 24">
                <rect x="4" y="4" width="16" height="11" rx="2" />
                <path d="M2 19h20M9 19l1-4h4l1 4" />
              </svg>
            </span>
            <span class="docs-status docs-status--ready">Available now</span>
          </header>
          <h3>Desktop library</h3>
          <p>Install Volum, connect existing model folders, and build previews locally.</p>
          <span class="docs-path__link">Installation guide <b aria-hidden="true">→</b></span>
        </a>

        <a class="docs-path" href="/docs/companion/overview">
          <header>
            <span class="docs-path__icon" aria-hidden="true">
              <svg viewBox="0 0 24 24">
                <rect x="6" y="2" width="12" height="20" rx="2.5" />
                <path d="M10 5h4M11 18h2" />
              </svg>
            </span>
            <span class="docs-status">Beta</span>
          </header>
          <h3>Mobile companion</h3>
          <p>Browse projects, capture photos, and keep selected work available offline.</p>
          <span class="docs-path__link">Companion guide <b aria-hidden="true">→</b></span>
        </a>

        <a class="docs-path" href="/docs/server/">
          <header>
            <span class="docs-path__icon" aria-hidden="true">
              <svg viewBox="0 0 24 24">
                <rect x="3" y="3" width="18" height="7" rx="2" />
                <rect x="3" y="14" width="18" height="7" rx="2" />
                <path d="M7 6.5h.01M7 17.5h.01M11 6.5h7M11 17.5h7" />
              </svg>
            </span>
            <span class="docs-status">Early access</span>
          </header>
          <h3>Always-on server</h3>
          <p>Run <code>volumd</code> in Docker on a NAS or home server you manage.</p>
          <span class="docs-path__link">Self-hosting guide <b aria-hidden="true">→</b></span>
        </a>
      </div>
    </section>

    <section class="docs-workbench" aria-labelledby="docs-workbench-title">
      <div class="docs-workbench__visual">
        <img
          class="docs-macbook"
          :src="projectMacbookScreenshot"
          alt="Volum Desktop showing the interactive 3D project viewer and model details in a MacBook Pro"
          loading="lazy"
        >
        <img
          class="docs-phone docs-phone--library"
          :src="mobileLibraryScreenshot"
          alt="Volum mobile library screen in an orange iPhone 17 Pro"
          loading="lazy"
        >
        <img
          class="docs-phone docs-phone--project"
          :src="mobileProjectScreenshot"
          alt="Volum mobile project detail screen in an orange iPhone 17 Pro"
          loading="lazy"
        >
      </div>

      <div class="docs-workbench__copy">
        <p class="docs-overline docs-overline--dark"><span aria-hidden="true"></span> Desktop + Companion</p>
        <h2 id="docs-workbench-title">From your desk to the workbench.</h2>
        <p>
          Pair a trusted phone through Tailscale to take project details, photos, parts, and assembly steps where you print.
        </p>
        <ul>
          <li><b>Direct connection.</b> Your device talks to your Desktop or server, not a Volum cloud.</li>
          <li><b>Offline selections.</b> Keep chosen projects available when the host sleeps.</li>
          <li><b>Revocable access.</b> Every paired device gets its own token.</li>
        </ul>
        <a class="docs-text-link" href="/docs/desktop/companion-access">Set up Companion Access <span aria-hidden="true">→</span></a>
      </div>
    </section>

    <section class="docs-guides" aria-labelledby="docs-guides-title">
      <div class="docs-section-heading docs-section-heading--compact">
        <p class="docs-overline"><span aria-hidden="true"></span> Popular guides</p>
        <h2 id="docs-guides-title">Get to the next useful step.</h2>
      </div>

      <div class="docs-guide-list">
        <a href="/docs/getting-started/first-library">
          <span>Desktop</span>
          <strong>Create your first library</strong>
          <p>Choose between linked folders and a managed library.</p>
          <b aria-hidden="true">01 →</b>
        </a>
        <a href="/docs/companion/pair">
          <span>Companion</span>
          <strong>Pair a mobile device</strong>
          <p>Connect over your Tailnet with a short-lived pairing code.</p>
          <b aria-hidden="true">02 →</b>
        </a>
        <a href="/docs/server/docker">
          <span>Self-hosting</span>
          <strong>Install with Docker</strong>
          <p>Deploy <code>volumd</code> and verify the first client connection.</p>
          <b aria-hidden="true">03 →</b>
        </a>
        <a href="/docs/help/troubleshooting">
          <span>Help</span>
          <strong>Troubleshoot a connection</strong>
          <p>Check health, Tailscale, pairing, permissions, and storage.</p>
          <b aria-hidden="true">04 →</b>
        </a>
      </div>
    </section>

    <section class="docs-closing" aria-labelledby="docs-closing-title">
      <div>
        <p class="docs-overline docs-overline--dark"><span aria-hidden="true"></span> New to Volum?</p>
        <h2 id="docs-closing-title">Start local. Expand when it helps.</h2>
      </div>
      <a class="docs-button docs-button--light" href="/docs/getting-started/choose-a-setup">
        Compare every setup <span aria-hidden="true">→</span>
      </a>
    </section>
  </main>
</template>
