-- Placeholder migration establishing core tables.
-- Replace with actual schema once storage decisions are finalized.

BEGIN;

CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY,
    handle TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS devices (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id),
    fingerprint TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS conversations (
    id UUID PRIMARY KEY,
    title TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS conversation_members (
    conversation_id UUID NOT NULL REFERENCES conversations(id),
    device_id UUID NOT NULL REFERENCES devices(id),
    joined_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (conversation_id, device_id)
);

COMMIT;
