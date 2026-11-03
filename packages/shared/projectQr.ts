export interface ProjectQrPayload {
  serverId: string;
  projectId: string;
}

function validIdentity(value: string) {
  return value.length > 0 && value.length <= 200 && !/[\u0000-\u001f\u007f]/.test(value);
}

export function projectQrPayload(serverId: string, projectId: string) {
  if (!validIdentity(serverId) || !validIdentity(projectId)) {
    throw new Error("The project identity is invalid.");
  }
  return `volum://project/${encodeURIComponent(serverId)}/${encodeURIComponent(projectId)}`;
}

export function parseProjectQrPayload(value: string): ProjectQrPayload {
  let url: URL;
  try {
    url = new URL(value.trim());
  } catch {
    throw new Error("This is not a Volum project code.");
  }
  const segments = url.pathname.split("/").filter(Boolean).map(decodeURIComponent);
  if (url.protocol !== "volum:" || url.hostname !== "project" || segments.length !== 2) {
    throw new Error("This is not a Volum project code.");
  }
  const [serverId, projectId] = segments;
  if (!validIdentity(serverId) || !validIdentity(projectId)) {
    throw new Error("This project code is invalid.");
  }
  return { serverId, projectId };
}
