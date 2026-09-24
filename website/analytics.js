const projectToken = import.meta.env.VITE_POSTHOG_PROJECT_TOKEN;
const apiHost = import.meta.env.VITE_POSTHOG_HOST;
let isReady = false;
let analyticsClient;
const pendingEvents = [];

const send = (event, properties) => analyticsClient?.capture(event, properties);

export const initAnalytics = async () => {
  if (!import.meta.env.PROD || !projectToken || !apiHost) return;

  const { default: posthog } = await import("posthog-js");
  analyticsClient = posthog;
  posthog.init(projectToken, {
    api_host: apiHost,
    ui_host: "https://eu.posthog.com",
    defaults: "2026-05-30",
    autocapture: false,
    capture_pageview: true,
    capture_pageleave: true,
    disable_session_recording: true,
    persistence: "sessionStorage",
    person_profiles: "identified_only",
    capture_exceptions: true,
    loaded: (client) => {
      analyticsClient = client;
      client.register({
        product: "volum",
        site: "volum.didac.dev",
        surface: "marketing_site"
      });
      isReady = true;
      pendingEvents.splice(0).forEach(({ event, properties }) => send(event, properties));
    }
  });
};

export const capture = (event, properties = {}) => {
  if (isReady) send(event, properties);
  else pendingEvents.push({ event, properties });
};
