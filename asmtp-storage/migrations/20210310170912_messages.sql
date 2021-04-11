CREATE TABLE IF NOT EXISTS thread
(
    topic       BLOB PRIMARY KEY NOT NULL,
    created_at  TEXT             NOT NULL DEFAULT (DATETIME('now'))
);

CREATE TABLE IF NOT EXISTS message
(
    id          INTEGER PRIMARY KEY NOT NULL,
    thread      BLOB                NOT NULL,
    content     BLOB                NOT NULL,
    created_at  TEXT                NOT NULL DEFAULT (DATETIME('now')),
    read_at     TEXT,

    FOREIGN KEY (thread)
        REFERENCES thread (topic)
        ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS thread_key
(
    key         BLOB NOT NULL,
    thread      BLOB NOT NULL,

    PRIMARY KEY (key, thread),
    FOREIGN KEY (key)
        REFERENCES public_key (key)
        ON DELETE CASCADE,
    FOREIGN KEY (thread)
        REFERENCES thread (topic)
        ON DELETE CASCADE
);
