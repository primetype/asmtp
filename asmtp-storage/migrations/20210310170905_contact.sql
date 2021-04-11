CREATE TABLE IF NOT EXISTS contact
(
    id          INTEGER PRIMARY KEY NOT NULL,
    name        TEXT                NOT NULL,
    created_at  TEXT                NOT NULL DEFAULT (DATETIME('now')),
    updated_at  TEXT                NOT NULL DEFAULT (DATETIME('now'))
);

CREATE TABLE IF NOT EXISTS public_key
(
    key         BLOB PRIMARY KEY NOT NULL,
    alias       TEXT,
    created_at  TEXT             NOT NULL DEFAULT (DATETIME('now')),
    updated_at  TEXT             NOT NULL DEFAULT (DATETIME('now'))
);

CREATE TABLE IF NOT EXISTS passport
(
    id          BLOB PRIMARY KEY NOT NULL,
    alias       TEXT,
    created_at  TEXT             NOT NULL DEFAULT (DATETIME('now')),
    updated_at  TEXT             NOT NULL DEFAULT (DATETIME('now')),
    blocks      BLOB             NOT NULL
);

CREATE TABLE IF NOT EXISTS passport_key
(
    public_key  BLOB    NOT NULL,
    passport    BLOB    NOT NULL,

    PRIMARY KEY (public_key, passport),
    FOREIGN KEY (public_key)
        REFERENCES public_key (key)
        ON DELETE CASCADE,
    FOREIGN KEY (passport)
        REFERENCES passport   (id)
        ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS contact_passport
(
    passport    BLOB    NOT NULL UNIQUE,
    contact     INTEGER NOT NULL,

    verified    BOOLEAN NOT NULL DEFAULT false,
    verified_at TEXT,

    PRIMARY KEY (passport, contact),
    FOREIGN KEY (passport)
        REFERENCES passport (id)
        ON DELETE CASCADE,
    FOREIGN KEY (contact)
        REFERENCES contact (id)
        ON DELETE CASCADE
);