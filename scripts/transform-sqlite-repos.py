#!/usr/bin/env python3
"""Transform backend repo modules into eunomio-sqlite trait impls."""
import re
import sys
from pathlib import Path

REPO_DIR = Path("/root/eunomio/crates/eunomio-sqlite/src/repo")
TRAIT_MAP = {
    "org": "OrgRepo",
    "user": "UserRepo",
    "session": "SessionRepo",
    "node": "NodeRepo",
    "partition": "PartitionRepo",
    "run": "RunRepo",
}

HEADER = """// SPDX-License-Identifier: Apache-2.0

use super::{{DbResultExt, require_affected_sqlite}};
use crate::db;
use async_trait::async_trait;
use eunomio_core::{{AppError, traits::*, types::*}};
use std::sync::Arc;
use tokio_rusqlite::Connection;

pub struct Sqlite{struct_name} {{
    conn: Arc<Connection>,
}}

impl Sqlite{struct_name} {{
    pub fn new(conn: Arc<Connection>) -> Self {{
        Self {{ conn }}
    }}
}}

"""


def struct_name(module: str) -> str:
    return "".join(p.capitalize() for p in module.split("_")) + "Repo"


def transform(content: str, module: str) -> str:
    trait_name = TRAIT_MAP[module]
    sn = struct_name(module)

    content = re.sub(
        r"use crate::\{[^}]+\};",
        "",
        content,
        count=1,
    )
    content = content.replace("use super::{require_affected_sqlite, DbResultExt};", "")
    content = content.replace(
        "use super::{require_affected_sqlite, DbResultExt, DbResultExt};", ""
    )

    # Remove New* structs from partition/run - they're in core now
    if module == "partition":
        content = re.sub(
            r"#\[derive\(Debug, Clone\)\]\npub struct SiblingInfo \{[^}]+\}\n\n",
            "",
            content,
        )
        content = re.sub(
            r"pub struct NewPartitionInsert \{[^}]+\}\n\n",
            "",
            content,
            flags=re.DOTALL,
        )
    if module == "run":
        content = re.sub(
            r"pub struct NewRunInsert \{[^}]+\}\n\n",
            "",
            content,
            flags=re.DOTALL,
        )
    if module == "node":
        content = re.sub(
            r"#\[derive\(Debug, Clone\)\]\npub struct NodeBasic \{[^}]+\}\n\n",
            "",
            content,
        )
    if module == "session":
        content = re.sub(
            r"pub struct CreatedSessionRow \{[^}]+\}\n\n",
            "",
            content,
            flags=re.DOTALL,
        )
        content = re.sub(
            r"pub struct SessionRepoFields \{[^}]+\}\n\n",
            "",
            content,
            flags=re.DOTALL,
        )
    if module == "user":
        content = re.sub(
            r"pub struct UserRow \{[^}]+\}\n\n",
            "",
            content,
            flags=re.DOTALL,
        )

    content = content.replace("state.db", "self.conn")
    content = content.replace("state\n        .db", "self.conn")
    content = content.replace("crate::db::", "db::")
    content = content.replace("crate::git::", "crate::git::")  # keep if added later

    # partition delete_many calls get(state,...) - fix to self.get
    if module == "partition":
        content = re.sub(
            r"\bget\(state, org_id, id\)",
            "self.get(org_id, id)",
            content,
        )
        content = re.sub(
            r"\bget\(state, &org_id, &partition_id\)",
            "self.get(&org_id, &partition_id)",
            content,
        )

    # run cancel_running calls partition::get
    if module == "run":
        content = content.replace(
            "super::partition::get(state, org_id, partition_id)",
            "SqlitePartitionRepo::new(self.conn.clone()).get(org_id, partition_id)",
        )

    # Convert pub async fn foo(state: &AppState, ...) to async fn foo(&self, ...)
    content = re.sub(
        r"pub async fn (\w+)\(\s*state: &AppState,\s*",
        r"async fn \1(&self, ",
        content,
    )
    content = re.sub(
        r"pub async fn (\w+)\(\s*state: &AppState\s*\)",
        r"async fn \1(&self)",
        content,
    )

    # session insert_seed_nodes takes org_id: String without state first
    content = re.sub(
        r"async fn insert_seed_nodes\(\s*&self,\s*",
        "async fn insert_seed_nodes(\n        &self,\n        ",
        content,
    )

    # Wrap functions in trait impl
    body = content.strip()
    if not body.startswith("#["):
        pass

    out = HEADER.format(struct_name=sn) + body + f"\n\n#[async_trait]\nimpl {trait_name} for Sqlite{sn} {{}}\n"

    # Move async fns inside impl block properly
    out = HEADER.format(struct_name=sn)
    # Extract function bodies and put inside trait impl
    funcs = re.findall(
        r"(#\[allow[^\]]*\]\n)?(async fn \w+\([^{]*\{(?:[^{}]|\{[^{}]*\})*\})",
        body,
        flags=re.DOTALL,
    )
    if not funcs:
        funcs_raw = body
    else:
        funcs_raw = "\n\n".join((a or "") + b for a, b in funcs)

    # Also grab pub fn mappers
    mappers = re.findall(r"pub fn \w+_mapper[^{]*\{[^}]+\}", body, flags=re.DOTALL)
    mapper_code = "\n\n".join(mappers)

    out += mapper_code + "\n\n" if mapper_code else ""
    out += f"#[async_trait]\nimpl {trait_name} for Sqlite{sn} {{\n"
    out += funcs_raw if funcs_raw else body
    out += "\n}\n"

    return out


def main():
    for path in sorted(REPO_DIR.glob("*.rs")):
        if path.name == "mod.rs":
            continue
        module = path.stem
        if module not in TRAIT_MAP:
            continue
        original = path.read_text()
        transformed = transform(original, module)
        path.write_text(transformed)
        print(f"transformed {path.name}")


if __name__ == "__main__":
    main()
