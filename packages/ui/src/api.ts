/**
 * A service path. A window of the macOS app belongs to one processor
 * (`?device=<id>`) and talks only to that processor's own session; the
 * browser's page uses the service's single unprefixed session.
 */
export function apiPath(path: string, search: string = typeof location === "undefined" ? "" : location.search) {
  const device = new URLSearchParams(search).get("device");
  return device ? `/api/d/${encodeURIComponent(device)}/${path}` : `/api/${path}`;
}

export async function api(path: string, body?: unknown) {
  const r = await fetch(
    apiPath(path),
    body === undefined
      ? {}
      : {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify(body),
        },
  );
  const data = await r.json();
  if (!r.ok) throw Error(data.error || "Connection failed");
  return data;
}
