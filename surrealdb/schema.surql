-- =============================================================================
-- ColimaUI — SurrealDB Auth & Entitlement Schema (SurrealDB v2 / v3)
-- =============================================================================
-- Apply with:
--   curl -s -u root:omo-secret -H "Accept: application/json" \
--     -H "surreal-ns: colima_ui" -H "surreal-db: auth" \
--     -X POST http://127.0.0.1:18000/sql --data-binary @surrealdb/schema.surql
-- =============================================================================

DEFINE NAMESPACE IF NOT EXISTS colima_ui;
USE NS colima_ui DB auth;

-- -----------------------------------------------------------------------------
-- 1. User Identity Table
-- -----------------------------------------------------------------------------
DEFINE TABLE IF NOT EXISTS user SCHEMAFULL
  PERMISSIONS
    FOR select, update, delete WHERE id = $auth.id
    FOR create NONE;

DEFINE FIELD IF NOT EXISTS email ON user TYPE string ASSERT string::is_email($value);
DEFINE FIELD IF NOT EXISTS name ON user TYPE string DEFAULT 'Account';
DEFINE FIELD IF NOT EXISTS passhash ON user TYPE string;
DEFINE FIELD IF NOT EXISTS avatar ON user TYPE string DEFAULT '';
DEFINE FIELD IF NOT EXISTS provider ON user TYPE string DEFAULT 'surrealdb';
DEFINE FIELD IF NOT EXISTS created_at ON user TYPE datetime DEFAULT time::now();

DEFINE INDEX IF NOT EXISTS user_email_idx ON user FIELDS email UNIQUE;

-- -----------------------------------------------------------------------------
-- 2. Record Access Method (`account`)
-- -----------------------------------------------------------------------------
DEFINE ACCESS IF NOT EXISTS account ON DATABASE TYPE RECORD
  SIGNUP (
    CREATE user CONTENT {
      email: string::lowercase($email),
      name: $name OR string::split($email, '@')[0] OR 'Account',
      passhash: crypto::argon2::generate($password),
      avatar: $avatar OR '',
      provider: 'surrealdb',
      created_at: time::now()
    }
  )
  SIGNIN (
    SELECT * FROM user
    WHERE email = string::lowercase($email)
      AND crypto::argon2::compare(passhash, $password)
  )
  DURATION FOR TOKEN 30d, FOR SESSION 24h;

-- -----------------------------------------------------------------------------
-- 3. Subscription Entitlement Table (read-only for authenticated user)
-- -----------------------------------------------------------------------------
DEFINE TABLE IF NOT EXISTS subscription SCHEMAFULL
  PERMISSIONS
    FOR select WHERE user_id = <string>$auth.id
    FOR create, update, delete NONE;

DEFINE FIELD IF NOT EXISTS user_id ON subscription TYPE string;
DEFINE FIELD IF NOT EXISTS entitled ON subscription TYPE bool DEFAULT false;
DEFINE FIELD IF NOT EXISTS tier ON subscription TYPE string DEFAULT 'pro';
DEFINE FIELD IF NOT EXISTS seats_total ON subscription TYPE option<int>;
DEFINE FIELD IF NOT EXISTS seats_claimed ON subscription TYPE option<int>;
DEFINE FIELD IF NOT EXISTS expires_at ON subscription TYPE option<string>;
DEFINE FIELD IF NOT EXISTS updated_at ON subscription TYPE datetime DEFAULT time::now();

DEFINE INDEX IF NOT EXISTS subscription_user_idx ON subscription FIELDS user_id UNIQUE;
