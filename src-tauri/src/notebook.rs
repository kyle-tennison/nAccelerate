//! Reads how many steps an nTop notebook should log, from the block list stored in the `.ntop` file.
//!
//! The format is undocumented. The layout below was worked out from two v5 notebooks, so every
//! assumption is checked and any mismatch is an error rather than a guess:
//!
//! - `0x00` magic `MAGIC%$1`, `0x08` section count `n` (u64 LE), `0x10` eight zero bytes.
//! - `0x18` a table of `n` entries: a 16-byte NUL-padded name and a u64 LE end offset.
//! - 80 zero bytes, then the section data at `base = 0x18 + 24n + 80`, which starts with `MAGIC@@9`.
//! - End offsets are cumulative from `base`, and the last one ends exactly at the end of the file.
//!
//! The `main` section holds JSON whose `code` array lists the blocks, followed by binary data.
//! A step is a `code` record with a non-empty `func` that is not a `core.group`. Empty-`func`
//! records are unnamed input values and never appear in the log.
//!
//! The count is the total if every block succeeds. A failed block makes its dependents skip, so a
//! run with errors logs fewer steps. Optimization iterations are not steps.

use serde::Serialize;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

const FILE_MAGIC: &[u8; 8] = b"MAGIC%$1";
const DATA_MAGIC: &[u8; 8] = b"MAGIC@@9";
const MAX_MAIN_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Block {
    pub name: String,
    /// Names of the blocks this one takes inputs from (unnamed input values are left out).
    pub deps: Vec<String>,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StepCount {
    pub steps: u32,
    /// More than one group means nested groups or custom blocks, which have not been checked against a run.
    pub groups: u32,
    /// The steps in file order, used to guess which block is running between log lines.
    pub blocks: Vec<Block>,
}

fn u64_at(buf: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(buf[at..at + 8].try_into().unwrap())
}

pub fn count_steps(path: &Path) -> Result<StepCount, String> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let len = file.metadata().map_err(|e| e.to_string())?.len();

    let mut head = [0u8; 24];
    file.read_exact(&mut head)
        .map_err(|_| "File is too short to be a notebook".to_string())?;
    if &head[..8] != FILE_MAGIC {
        return Err("Not a recognised .ntop file (unexpected header)".into());
    }
    let n = u64_at(&head, 8);
    if n == 0 || n > 256 {
        return Err(format!("Unexpected section count {n}"));
    }
    let mut table = vec![0u8; 24 * n as usize];
    file.read_exact(&mut table)
        .map_err(|_| "Section table is cut short".to_string())?;
    let base = 0x18 + 24 * n + 80;

    let mut sections = Vec::new();
    let mut prev = 0u64;
    for entry in table.chunks_exact(24) {
        let name = String::from_utf8_lossy(&entry[..16]).trim_end_matches('\0').to_string();
        let end = u64_at(entry, 16);
        if end < prev {
            return Err("Section offsets are out of order".into());
        }
        sections.push((name, base + prev, base + end));
        prev = end;
    }
    if base + prev != len {
        return Err("Section table does not match the file size".into());
    }

    let mut magic = [0u8; 8];
    file.seek(SeekFrom::Start(base)).map_err(|e| e.to_string())?;
    file.read_exact(&mut magic).map_err(|e| e.to_string())?;
    if &magic != DATA_MAGIC {
        return Err("Section data does not start where expected".into());
    }

    let (_, start, end) = sections
        .iter()
        .find(|(name, _, _)| name == "main")
        .ok_or("No main section")?;
    if end - start > MAX_MAIN_BYTES {
        return Err("The main section is unexpectedly large".into());
    }
    let mut main = vec![0u8; (end - start) as usize];
    file.seek(SeekFrom::Start(*start)).map_err(|e| e.to_string())?;
    file.read_exact(&mut main).map_err(|e| e.to_string())?;

    let at = main
        .windows(7)
        .position(|w| w == b"{\"code\"")
        .ok_or("No block list in the main section")?;
    // Binary data follows the JSON, so only the first complete value is parsed.
    let value: serde_json::Value = serde_json::Deserializer::from_slice(&main[at..])
        .into_iter()
        .next()
        .ok_or("Block list is empty")?
        .map_err(|e| format!("Could not read the block list: {e}"))?;
    let code = value
        .get("code")
        .and_then(|c| c.as_array())
        .ok_or("Block list has no code array")?;

    let func = |r: &serde_json::Value| r.get("func").and_then(|f| f.as_str()).unwrap_or("").to_string();
    let is_step = |f: &str| !f.is_empty() && !f.starts_with("core.group");
    let names: std::collections::HashMap<u64, String> = code
        .iter()
        .filter(|r| is_step(&func(r)))
        .filter_map(|r| Some((r.get("id")?.as_u64()?, r.get("name")?.as_str()?.to_string())))
        .collect();

    let mut count = StepCount {
        steps: 0,
        groups: 0,
        blocks: Vec::new(),
    };
    for record in code {
        let f = func(record);
        if f.starts_with("core.group") {
            count.groups += 1;
        } else if is_step(&f) {
            count.steps += 1;
            let name = record
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or_default()
                .to_string();
            let mut deps: Vec<String> = record
                .get("inputs")
                .and_then(|i| i.as_array())
                .into_iter()
                .flatten()
                .filter_map(|i| i.get("instanceId")?.as_u64())
                .filter_map(|id| names.get(&id).cloned())
                .collect();
            deps.dedup();
            count.blocks.push(Block { name, deps });
        }
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn notebook(main_json: &str) -> Vec<u8> {
        let first = [DATA_MAGIC.as_slice(), b"header data"].concat();
        let main = [
            b"\x00\x01".as_slice(),
            main_json.as_bytes(),
            b"\xff\xfe binary \x80 tail",
        ]
        .concat();
        let sections: [(&str, &[u8]); 2] = [("stuff", &first), ("main", &main)];
        let mut out = FILE_MAGIC.to_vec();
        out.extend(2u64.to_le_bytes());
        out.extend([0u8; 8]);
        let mut end = 0u64;
        for (name, data) in sections {
            let mut field = [0u8; 16];
            field[..name.len()].copy_from_slice(name.as_bytes());
            out.extend(field);
            end += data.len() as u64;
            out.extend(end.to_le_bytes());
        }
        out.extend([0u8; 80]);
        for (_, data) in sections {
            out.extend_from_slice(data);
        }
        out
    }

    #[test]
    fn counts_named_blocks_and_skips_groups_and_values() {
        let json = r#"{"code":[{"id":1,"name":"Root","func":"core.group"},{"id":2,"name":"Scalar_anon_1","func":""},
            {"id":3,"name":"Mesh_0","func":"mesh.from_body","inputs":[{"instanceId":4},{"instanceId":2},{"instanceId":0}]},
            {"id":4,"name":"Radius","func":"core.var.scalar"},
            {"id":5,"name":"Plane List_0","func":"core.list.plane"},{"id":6,"name":"Brücke","func":"core.concatenate_lists"}],"other":1}"#;
        let path = std::env::temp_dir().join(format!("naccel-steps-{}.ntop", std::process::id()));
        std::fs::write(&path, notebook(json)).unwrap();
        let c = count_steps(&path).unwrap();
        assert_eq!((c.steps, c.groups), (4, 1));
        assert_eq!(
            c.blocks[0],
            Block {
                name: "Mesh_0".into(),
                deps: vec!["Radius".into()]
            }
        );
        assert_eq!(c.blocks[3].name, "Brücke");
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn rejects_a_file_whose_table_does_not_match_its_size() {
        let mut bytes = notebook(r#"{"code":[]}"#);
        bytes.push(0);
        let path = std::env::temp_dir().join(format!("naccel-steps-bad-{}.ntop", std::process::id()));
        std::fs::write(&path, bytes).unwrap();
        assert!(count_steps(&path).is_err());
        std::fs::remove_file(&path).unwrap();
    }
}
