export function validatedExternalUrl(value: string) {
  let url: URL;
  try { url = new URL(value); }
  catch { throw new Error("This source link is invalid."); }
  if (!matchesPublicWebProtocol(url.protocol) || url.username || url.password) {
    throw new Error("Volum can only open ordinary HTTP or HTTPS source links.");
  }
  return url;
}

function matchesPublicWebProtocol(protocol: string) {
  return protocol === "https:" || protocol === "http:";
}
