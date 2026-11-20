-- One-shot execution ledger for Sandwich Bot. Apply ONLY to a dedicated
-- Sandwich Bot PostgreSQL database after owner approval. Never run against
-- Nimbus, Trading Lab or unrelated projects.
--
-- The app must NOT own the table or have DELETE/TRUNCATE/UPDATE privileges.
-- Use a distinct migration/admin role for DDL and a restricted INSERT/SELECT
-- application role; grant no privileges that can reset the trial.
--
-- The fixed primary key is intentional: a worker restart or multiple workers
-- must not create a second attempt by changing an arbitrary trial UUID.
CREATE TABLE IF NOT EXISTS first_live_trade_attempt (
    trial_id TEXT PRIMARY KEY DEFAULT 'first_live_trial'
        CHECK (trial_id = 'first_live_trial'),
    attempt_id UUID NOT NULL DEFAULT gen_random_uuid() UNIQUE,
    chain_id BIGINT NOT NULL CHECK (chain_id = 1),
    signer_address TEXT NOT NULL
        CHECK (signer_address ~ '^0x[0-9a-f]{40}$'),
    executor_address TEXT NOT NULL
        CHECK (executor_address ~ '^0x[0-9a-f]{40}$'),
    tx_nonce NUMERIC(78,0) NOT NULL CHECK (tx_nonce >= 0),
    calldata_sha256 TEXT NOT NULL
        CHECK (calldata_sha256 ~ '^[0-9a-f]{64}$'),
    gas_limit BIGINT NOT NULL CHECK (gas_limit > 0),
    max_fee_per_gas_wei NUMERIC(78,0) NOT NULL
        CHECK (max_fee_per_gas_wei > 0),
    eth_usd_e8 NUMERIC(78,0) NOT NULL CHECK (eth_usd_e8 > 0),
    quote_timestamp TIMESTAMPTZ NOT NULL,
    -- USD-e8 costs must be precomputed using reviewed checked U256 math,
    -- with a >=10% buffer and the signed EIP-1559 maximum fee cap.
    maximum_gas_usd_e8 NUMERIC(78,0) NOT NULL
        CHECK (maximum_gas_usd_e8 > 0 AND maximum_gas_usd_e8 <= 500000000),
    reserved_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

COMMENT ON TABLE first_live_trade_attempt IS
    'Immutable consumed-at-reservation ledger; no worker retries, even if broadcast outcome unknown';

-- Only the FIRST insertion can succeed. Caller must require exactly one
-- returned row before signing/broadcasting. No "select then insert" race.
-- Bind inputs as parameters; the signer/executor/nonce/calldata must match
-- the exact previously simulated transaction.
--
-- INSERT INTO first_live_trade_attempt
--   (trial_id, chain_id, signer_address, executor_address, tx_nonce,
--    calldata_sha256, gas_limit, max_fee_per_gas_wei, eth_usd_e8,
--    quote_timestamp, maximum_gas_usd_e8)
-- VALUES
--   ('first_live_trial', $1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
-- ON CONFLICT (trial_id) DO NOTHING
-- RETURNING attempt_id;
--
-- IMPORTANT:
--  * COMMIT the reservation BEFORE any possible broadcast.
--  * If zero rows are returned, STOP, including after restart or timeout.
--  * If the commit outcome is UNKNOWN, STOP; do not broadcast.
--  * If RPC broadcast outcome is UNKNOWN, STOP; do not retry/replace nonce.
--  * Do NOT TRUNCATE or DELETE a row to 'recover' a budget.
--  * Configure a dedicated DB role with INSERT + SELECT only and no ownership.
--  * SQL persistence protects a single attempt only if the app uses exactly
--    this fixed trial key and never bypasses the ledger.

CREATE TABLE IF NOT EXISTS first_live_trade_receipt (
    trial_id TEXT PRIMARY KEY
        REFERENCES first_live_trade_attempt(trial_id),
    tx_hash TEXT NOT NULL UNIQUE CHECK (tx_hash ~ '^0x[0-9a-f]{64}$'),
    gas_used BIGINT NOT NULL CHECK (gas_used >= 0),
    effective_gas_price_wei NUMERIC(78,0) NOT NULL
        CHECK (effective_gas_price_wei >= 0),
    receipt_status SMALLINT NOT NULL CHECK (receipt_status IN (0, 1)),
    observed_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Receipt observation is append-only. The transaction may never be
-- included; a missing receipt does NOT restore the consumed attempt.
-- Do not infer a successful profit from receipt_status alone: verify token
-- balances/event deltas for the exact approved executor and asset.
