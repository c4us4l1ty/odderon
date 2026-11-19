// Pin the Ed25519 public key of *your* authority at deployment time (64 hex
// characters). Never obtain this trust anchor from an API response or from
// localStorage. An empty key intentionally disables ranked certification;
// offline practice and local replay remain available.
export const SERVER_PUBLIC_KEY = '';
