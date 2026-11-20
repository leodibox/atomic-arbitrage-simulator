-- One-shot execution ledger for Sandwich Bot. The owner requested NO NEW
-- Render services and explicitly excluded Nimbus. Only apply within an
-- authorized, pre-verified, durable PostgreSQL instance, isolated in the
-- sandwich_bot schema under dedicated roles. Do not run against Nimbus.
-- The free Trading Lab database expires in December 2026 and is NOT a
-- trustworthy long-term live gas ledger without a migration/retention plan.
--
-- The app must NOT own the table or have DELETE/TRUNCATE/UPDATE privileges.
-- Use a distinct migration/admin role for DDL and a restricted INSERT/SELECT
-- application role; grant no privileges that can reset the trial.
--
-- The fixed primary key is intentional: a worker restart or multiple workers
-- must not create a second attempt by changing an arbitrary trial UUID.
CREATE SCHEMA IF NOT EXISTS sandwich_bot;

CREATE TABLE IF NOT EXISTS sandwich_bot.first_live_trade_attempt (
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

-- Independent DB-side guard against dishonest or buggy caller-provided
-- maximum_gas_usd_e8; compute exact ceiling after a 10% FX buffer.
-- It is not an ETH/USD oracle: the quote still MUST be independently verified.
-- Use clock_timestamp() in a trigger, not a volatile CHECK constraint.
CREATE OR REPLACE FUNCTION sandwich_bot.first_live_trade_validate_budget()
RETURNS trigger LANGUAGE plpgsql AS $
DECLARE
    computed_usd_e8 NUMERIC;
BEGIN
    IF NEW.quote_timestamp > clock_timestamp() + INTERVAL '2 seconds'
       OR NEW.quote_timestamp < clock_timestamp() - INTERVAL '30 seconds'
    THEN
        RAISE EXCEPTION 'missing, stale or future ETH/USD quote';
    END IF;
    -- wei * ETH/USD(8 decimals) / 10^18, with an additional 10% margin.
    computed_usd_e8 := ceil(
        NEW.gas_limit::numeric * NEW.max_fee_per_gas_wei
        * NEW.eth_usd_e8 * 110 / (100 * 1000000000000000000::numeric)
    );
    IF computed_usd_e8 <= 0
       OR computed_usd_e8 > 500000000
       OR NEW.maximum_gas_usd_e8 <> computed_usd_e8
    THEN
        RAISE EXCEPTION 'worst-case gas cost exceeds or mismatches $5 USD budget';
    END IF;
    RETURN NEW;
END $;

DROP TRIGGER IF EXISTS first_live_trade_budget_on_insert ON sandwich_bot.first_live_trade_attempt;
CREATE TRIGGER first_live_trade_budget_on_insert
BEFORE INSERT ON sandwich_bot.first_live_trade_attempt
FOR EACH ROW EXECUTE FUNCTION sandwich_bot.first_live_trade_validate_budget();

COMMENT ON TABLE sandwich_bot.first_live_trade_attempt IS
    'Immutable consumed-at-reservation ledger; no worker retries, even if broadcast outcome unknown';

-- Only the FIRST insertion can succeed. Caller must require exactly one
-- returned row before signing/broadcasting. No "select then insert" race.
-- Bind inputs as parameters; the signer/executor/nonce/calldata must match
-- the exact previously simulated transaction.
--
-- INSERT INTO sandwich_bot.first_live_trade_attempt
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

CREATE TABLE IF NOT EXISTS sandwich_bot.first_live_trade_receipt (
    trial_id TEXT PRIMARY KEY
        REFERENCES sandwich_bot.first_live_trade_attempt(trial_id),
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

-- Defense in depth for a shared database. Only an authorized migration/admin
-- role should apply this script. This does not grant access to any worker.
-- A separate non-owner app role must receive ONLY explicit USAGE,
-- INSERT/SELECT on these tables after the database is approved and tested.
REVOKE ALL ON SCHEMA sandwich_bot FROM PUBLIC;
REVOKE ALL ON TABLE sandwich_bot.first_live_trade_attempt FROM PUBLIC;
REVOKE ALL ON TABLE sandwich_bot.first_live_trade_receipt FROM PUBLIC;
REVOKE ALL ON FUNCTION sandwich_bot.first_live_trade_validate_budget() FROM PUBLIC;
