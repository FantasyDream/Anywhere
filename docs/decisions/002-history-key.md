# 002 — Non-null usage-history key

Problem: SQLite allows NULL in a composite primary key; NULL does not conflict
with another NULL. The plan's nullable `source_app` would create separate rows for
commands executed without a foreground application instead of incrementing usage.

Affected module: anywhere-core's SQLite history adapter.

Minimal change: use `source_app TEXT NOT NULL DEFAULT ''`. Empty string means
unknown application, so the existing three-column primary key reliably upserts.

Compatibility: no released database exists. No protocol field changes. A migration
would be necessary only for a database created using the original nullable schema.
