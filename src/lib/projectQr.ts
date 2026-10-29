export function projectQrPayload(serverId: string, projectId: string) {
  if (!validIdentity(serverId) || !validIdentity(projectId)) throw new Error("The project identity is invalid.");
  return `volum://project/${encodeURIComponent(serverId)}/${encodeURIComponent(projectId)}`;
}

function validIdentity(value: string) {
  return value.length > 0 && value.length <= 200 && !/[\u0000-\u001f\u007f]/.test(value);
}
