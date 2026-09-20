export interface DidacProject {
  name: string;
  description: string;
  imageUrl: string;
  url: string;
  appStoreUrl?: string;
}

const PROJECTS_ENDPOINT = "https://didac.dev/api/projects";

function secureUrl(value: unknown): string | undefined {
  if (typeof value !== "string" || value.length > 2048) return undefined;
  try {
    const url = new URL(value);
    return url.protocol === "https:" ? url.toString() : undefined;
  } catch {
    return undefined;
  }
}

function parseProject(value: unknown): DidacProject | undefined {
  if (!value || typeof value !== "object") return undefined;
  const item = value as Record<string, unknown>;
  const url = secureUrl(item.url);
  const imageUrl = secureUrl(item.imageUrl);
  if (typeof item.name !== "string" || typeof item.description !== "string" || !url || !imageUrl) return undefined;
  return {
    name: item.name.slice(0, 100),
    description: item.description.slice(0, 300),
    imageUrl,
    url,
    appStoreUrl: secureUrl(item.appStoreUrl)
  };
}

export async function fetchRandomDidacProjects(signal?: AbortSignal): Promise<DidacProject[]> {
  const response = await fetch(PROJECTS_ENDPOINT, { signal, headers: { Accept: "application/json" } });
  if (!response.ok) throw new Error(`Project list unavailable (${response.status})`);
  const payload: unknown = await response.json();
  if (!Array.isArray(payload)) throw new Error("Project list has an unexpected format");
  const projects = payload.map(parseProject).filter((project): project is DidacProject => Boolean(project));
  for (let index = projects.length - 1; index > 0; index -= 1) {
    const random = crypto.getRandomValues(new Uint32Array(1))[0] / 2 ** 32;
    const swap = Math.floor(random * (index + 1));
    [projects[index], projects[swap]] = [projects[swap], projects[index]];
  }
  return projects.slice(0, 5);
}
