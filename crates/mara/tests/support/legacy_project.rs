//! The 0.3.0 input of the performance harness (DES-PERFORMANCE-HARNESS in
//! `docs/performance.mara.md`): a small hand-written 0.3.0 project, by path, `.git` included.

pub const FILES: [(&str, &str); 14] = [
    (
        ".mara/project.toml",
        "format_version = 3\n\n[project]\nname = \"counts\"\nschema = \".mara/schema.yaml\"\n\n[content]\ninclude = [\"docs/**/*.mara.md\"]\n",
    ),
    (
        ".mara/schema.yaml",
        "format_version: 3\nflavours:\n  requirement:\n    description: An obligation.\n    use_when: [x]\n    avoid_when: []\n    distinguish_from: {}\n    id_prefix: REQ-\n    body: required\n    fields:\n      status: {type: enum, values: [draft]}\nrelations:\n  depends_on:\n    description: Needs.\n    source: [requirement]\n    target: [requirement]\n    inverse: required_by\n",
    ),
    (
        "docs/a.mara.md",
        "# A\n\n:::mara requirement REQ-A\n:mid: 01M7FX0E000000000000000001\n:title: A\n:status: draft\n:depends_on: REQ-B\n\nNames [[REQ-B]].\n:::\n",
    ),
    (
        "docs/sub/b.mara.md",
        "# B\n\n:::mara requirement REQ-B\n:mid: 01M7FX0E000000000000000002\n:title: B\n:status: draft\n\nBody.\n:::\n",
    ),
    (
        "docs/notes.mara.md",
        "# Notes\n\nNo 0.3.0 syntax: not written.\n",
    ),
    (".gitignore", "ignored/\n"),
    ("docs/.gitignore", "drafts/\n"),
    ("docs/ignored/c.mara.md", "# Ignored\n"),
    (
        "docs/ignored/.gitignore",
        "Below an ignored directory: never read.\n",
    ),
    (
        "docs/drafts/d.mara.md",
        "# Ignored by the nested .gitignore\n",
    ),
    ("docs/notes.txt", "Not a document.\n"),
    ("README.md", "# Readme\n"),
    (".git/HEAD", "ref: refs/heads/main\n"),
    (".git/objects/00/object", "object"),
];
