import {
  ArchiveRestore,
  ArrowUpRight,
  BadgeEuro,
  Box,
  Check,
  ChevronDown,
  createIcons,
  Download,
  FolderSync,
  Heart,
  ListFilter,
  Menu,
  Plus,
  X
} from "lucide";
import { capture, initAnalytics } from "./analytics.js";

createIcons({
  icons: {
    ArchiveRestore,
    ArrowUpRight,
    BadgeEuro,
    Box,
    Check,
    ChevronDown,
    Download,
    FolderSync,
    Heart,
    ListFilter,
    Menu,
    Plus,
    X
  }
});

void initAnalytics();

const version = document.body.dataset.version || "unknown";
let detectedPlatform = "unknown";

const platformDownloads = {
  macos: {
    architecture: "universal",
    href: `https://github.com/sabatesduran/volum/releases/latest/download/Volum_${version}_universal.dmg`,
    label: "Download for macOS"
  },
  linux: {
    architecture: "x86_64",
    href: "https://github.com/sabatesduran/volum/releases/latest/download/Volum_linux_x86_64.AppImage",
    label: "Download AppImage"
  },
  windows: {
    architecture: "x86_64",
    href: "https://github.com/sabatesduran/volum/releases/latest/download/Volum_windows_x86_64_setup.exe",
    label: "Download for Windows"
  },
  windowsArm: {
    architecture: "arm64",
    href: "https://github.com/sabatesduran/volum/releases/latest/download/Volum_windows_arm64_setup.exe",
    label: "Download for Windows ARM64",
    platform: "windows"
  }
};

const detectPlatform = async () => {
  const ua = navigator.userAgent || "";
  const platform = navigator.userAgentData?.platform || navigator.platform || "";
  if (/iphone|ipad|android/i.test(ua) || (/mac/i.test(platform) && navigator.maxTouchPoints > 1)) return "unknown";
  if (/mac/i.test(platform) || /macintosh/i.test(ua)) return "macos";
  if (/win/i.test(platform) || /windows/i.test(ua)) {
    try {
      const values = await navigator.userAgentData?.getHighEntropyValues?.(["architecture"]);
      if (values?.architecture === "arm") return "windowsArm";
    } catch {
      // A direct x64 download remains the safest fallback when architecture hints are unavailable.
    }
    return "windows";
  }
  if (/linux/i.test(platform) || /linux/i.test(ua)) return "linux";
  return "unknown";
};

const applyPlatformDownload = async () => {
  detectedPlatform = await detectPlatform();
  const download = platformDownloads[detectedPlatform];
  if (!download) return;

  const platform = download.platform || detectedPlatform;
  document.querySelector(`[data-platform-card="${platform}"]`)?.classList.add("is-detected");
  document.querySelectorAll("[data-platform-download]").forEach((link) => {
    link.href = download.href;
    link.dataset.downloadPlatform = platform;
    link.dataset.downloadArchitecture = download.architecture;
    const label = link.querySelector("[data-platform-download-label]");
    if (label) label.textContent = link.classList.contains("nav-download") ? download.label : `${download.label} — free`;
  });
};

void applyPlatformDownload();

const header = document.querySelector("[data-header]");
const nav = document.querySelector("[data-nav]");
const navToggle = document.querySelector("[data-nav-toggle]");
const navToggleLabel = navToggle?.querySelector(".sr-only");

const updateHeader = () => header?.classList.toggle("is-scrolled", window.scrollY > 18);
updateHeader();
window.addEventListener("scroll", updateHeader, { passive: true });

const setNavOpen = (open) => {
  nav?.classList.toggle("is-open", open);
  navToggle?.setAttribute("aria-expanded", String(open));
  if (navToggleLabel) navToggleLabel.textContent = open ? "Close navigation" : "Open navigation";
};

navToggle?.addEventListener("click", () => {
  const open = !nav?.classList.contains("is-open");
  setNavOpen(open);
});

nav?.querySelectorAll("a").forEach((link) => link.addEventListener("click", () => {
  capture("volum_nav_clicked", { destination: link.getAttribute("href") || "unknown" });
  setNavOpen(false);
}));

document.addEventListener("click", (event) => {
  if (!nav?.classList.contains("is-open")) return;
  if (nav.contains(event.target) || navToggle?.contains(event.target)) return;
  setNavOpen(false);
});

document.addEventListener("keydown", (event) => {
  if (event.key !== "Escape" || !nav?.classList.contains("is-open")) return;
  setNavOpen(false);
  navToggle?.focus();
});

const sectionLinks = Array.from(document.querySelectorAll("[data-section-link]"));
const sectionTargets = sectionLinks.map((link) => ({
  link,
  section: document.querySelector(link.getAttribute("href"))
})).filter(({ section }) => section);
const navigationEnd = document.querySelector("#support");
let navigationFrame;

const updateCurrentSection = () => {
  navigationFrame = undefined;
  const marker = window.scrollY + window.innerHeight * 0.32;
  const pastNavigation = navigationEnd && marker >= navigationEnd.offsetTop;
  let current;

  if (!pastNavigation) {
    sectionTargets.forEach((entry) => {
      if (entry.section.offsetTop <= marker) current = entry;
    });
  }

  sectionTargets.forEach(({ link }) => {
    if (link === current?.link) link.setAttribute("aria-current", "location");
    else link.removeAttribute("aria-current");
  });
};

const scheduleCurrentSectionUpdate = () => {
  if (!navigationFrame) navigationFrame = requestAnimationFrame(updateCurrentSection);
};

updateCurrentSection();
window.addEventListener("scroll", scheduleCurrentSectionUpdate, { passive: true });
window.addEventListener("resize", scheduleCurrentSectionUpdate);

document.querySelectorAll("[data-download]").forEach((link) => link.addEventListener("click", () => {
  capture("volum_download_clicked", {
    architecture: link.dataset.downloadArchitecture || "unknown",
    detected_platform: detectedPlatform,
    location: link.dataset.downloadLocation || "unknown",
    platform: link.dataset.downloadPlatform || "unknown",
    version
  });
}));

document.querySelectorAll("[data-github-location]").forEach((link) => link.addEventListener("click", () => {
  capture("volum_github_opened", { location: link.dataset.githubLocation || "unknown" });
}));

document.querySelectorAll("[data-support-location]").forEach((link) => link.addEventListener("click", () => {
  capture("volum_support_clicked", { location: link.dataset.supportLocation || "unknown" });
}));

const observer = "IntersectionObserver" in window
  ? new IntersectionObserver((entries) => {
      entries.forEach((entry) => {
        if (!entry.isIntersecting) return;
        entry.target.classList.add("is-visible");
        observer.unobserve(entry.target);
      });
    }, { threshold: 0.12, rootMargin: "0px 0px -40px" })
  : undefined;

document.querySelectorAll(".reveal").forEach((element) => {
  if (element.closest(".hero")) element.classList.add("is-visible");
  else if (observer) observer.observe(element);
  else element.classList.add("is-visible");
});

const dialog = document.querySelector("[data-shot-dialog]");
const dialogImage = document.querySelector("[data-shot-image]");
const dialogCaption = document.querySelector("[data-shot-caption]");

document.querySelectorAll("[data-shot]").forEach((button) => button.addEventListener("click", () => {
  if (!(dialog instanceof HTMLDialogElement) || !(dialogImage instanceof HTMLImageElement)) return;
  const sourceImage = button.querySelector("img");
  if (!(sourceImage instanceof HTMLImageElement)) return;
  capture("volum_screenshot_opened", { screenshot: button.dataset.shotName || "unknown" });
  dialogImage.src = sourceImage.currentSrc || sourceImage.src;
  dialogImage.alt = sourceImage.alt;
  if (dialogCaption) dialogCaption.textContent = sourceImage.alt;
  dialog.showModal();
}));

document.querySelector("[data-shot-close]")?.addEventListener("click", () => dialog?.close());
dialog?.addEventListener("click", (event) => {
  if (event.target === dialog) dialog.close();
});

document.querySelectorAll(".faq-list details").forEach((details) => details.addEventListener("toggle", () => {
  if (!details.open) return;
  capture("volum_faq_opened", { question: details.dataset.faqId || "unknown" });
  document.querySelectorAll(".faq-list details").forEach((other) => {
    if (other !== details) other.removeAttribute("open");
  });
}));

const year = document.querySelector("[data-year]");
if (year) year.textContent = String(new Date().getFullYear());
