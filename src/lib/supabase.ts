/**
 * Compatibility bridge re-exporting the SurrealDB Auth & Session client.
 *
 * ColimaUI replaced Supabase Auth with SurrealDB Record Authentication
 * (`src/lib/surrealdb.ts` + `surrealdb/schema.surql`). Existing modules that
 * import from `./supabase` receive the SurrealDB client and storage adapter.
 */
export {
  AUTH_REDIRECT_URL,
  OAUTH_ENABLED,
  PKCE_VERIFIER_KEY,
  SESSION_STORAGE_KEY,
  SURREALDB_ACCESS,
  SURREALDB_DB,
  SURREALDB_NS,
  SURREALDB_URL,
  SURREALDB_URL as SUPABASE_URL,
  accountAvailability,
  clearMemoryStorage,
  getSurrealAuth,
  getSurrealAuth as getSupabase,
  keychainStorage,
  surrealQuery,
  type AuthChangeEvent,
  type AuthSession,
  type AuthStateChangeCallback,
  type AuthUser,
  type StorageAdapter,
  type SurrealAuthClient,
} from "./surrealdb";
