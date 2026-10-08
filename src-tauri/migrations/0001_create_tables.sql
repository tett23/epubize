-- 作品、話、挿絵（ADR 0017）。日時は UTC の ISO 8601 の文字列で持つ

CREATE TABLE novels (
    id INTEGER PRIMARY KEY,
    site TEXT NOT NULL CHECK (site IN ('narou', 'novel18', 'hameln', 'kakuyomu')),
    site_id TEXT NOT NULL CHECK (site_id <> ''),
    url TEXT NOT NULL,
    title TEXT NOT NULL,
    author_name TEXT NOT NULL,
    author_url TEXT,
    description TEXT NOT NULL,
    episode_count INTEGER NOT NULL CHECK (episode_count >= 0),
    -- 完結済みか。クローラーが渡さなければ null
    is_concluded INTEGER CHECK (is_concluded IN (0, 1)),
    metadata_fetched_at TEXT NOT NULL,
    -- 整形の設定（JSON）。null なら既定の設定を使う
    normalize_options TEXT CHECK (normalize_options IS NULL OR json_valid(normalize_options)),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (site, site_id)
) STRICT;

CREATE TABLE episodes (
    id INTEGER PRIMARY KEY,
    novel_id INTEGER NOT NULL REFERENCES novels (id) ON DELETE CASCADE,
    -- 目次の上での 1 始まりの位置
    no INTEGER NOT NULL CHECK (no >= 1),
    url TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL,
    -- 章の名前。続く話で同じ名前なら同じ章とみなす
    chapter TEXT,
    published_at TEXT,
    revised_at TEXT,
    char_count INTEGER CHECK (char_count >= 0),
    -- null なら本文は未取得
    body_fetched_at TEXT,
    -- 本文を取得したときの revised_at。目次の revised_at がこれより新しければ取り直す
    body_revised_at TEXT,
    -- Kindle に送った日時
    sent_at TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    -- 大きな列は最後に置き、一覧で読まずに済むようにする
    preface TEXT,
    body TEXT,
    afterword TEXT,
    UNIQUE (novel_id, no)
) STRICT;

CREATE TABLE episode_images (
    episode_id INTEGER NOT NULL REFERENCES episodes (id) ON DELETE CASCADE,
    -- 話の中で最初に現れた順（1 始まり）
    position INTEGER NOT NULL CHECK (position >= 1),
    url TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (episode_id, position)
) STRICT;

CREATE INDEX episode_images_url ON episode_images (url);

CREATE TABLE images (
    url TEXT PRIMARY KEY,
    content_type TEXT NOT NULL,
    data BLOB NOT NULL,
    byte_length INTEGER NOT NULL CHECK (byte_length = length(data)),
    fetched_at TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;
