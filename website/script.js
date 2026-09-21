const header = document.querySelector("[data-header]");
const nav = document.querySelector("[data-nav]");
const navToggle = document.querySelector("[data-nav-toggle]");

const updateHeader = () => header?.classList.toggle("is-scrolled", window.scrollY > 18);
updateHeader();
window.addEventListener("scroll", updateHeader, { passive: true });

navToggle?.addEventListener("click", () => {
  const open = !nav?.classList.contains("is-open");
  nav?.classList.toggle("is-open", open);
  navToggle.setAttribute("aria-expanded", String(open));
});

nav?.querySelectorAll("a").forEach((link) => link.addEventListener("click", () => {
  nav.classList.remove("is-open");
  navToggle?.setAttribute("aria-expanded", "false");
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
  document.querySelectorAll(".faq-list details").forEach((other) => {
    if (other !== details) other.removeAttribute("open");
  });
}));

const year = document.querySelector("[data-year]");
if (year) year.textContent = String(new Date().getFullYear());
