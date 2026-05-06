const TOKEN_KEY = "fairyflow_token";
let _token: string | null = null;

export function setToken(token: string): void {
  _token = token;
  sessionStorage.setItem(TOKEN_KEY, token);
}

export function loadToken(): string | null {
  if (!_token) {
    _token = sessionStorage.getItem(TOKEN_KEY);
  }
  return _token;
}

export function withToken(url: string): string {
  if (!_token) return url;
  const sep = url.includes("?") ? "&" : "?";
  return `${url}${sep}token=${encodeURIComponent(_token)}`;
}
