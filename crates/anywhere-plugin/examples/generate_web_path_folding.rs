//! Project the Rust archive validator's Unicode case folding into the zero-dependency CLI.
use std::{collections::BTreeMap, error::Error, path::Path};
use unicode_casefold::UnicodeCaseFold;

fn main() -> Result<(), Box<dyn Error>> {
    let check = match std::env::args().skip(1).collect::<Vec<_>>().as_slice() {
        [] => false,
        [flag] if flag == "--check" => true,
        _ => return Err("expected --check or no arguments".into()),
    };
    let mut folding = BTreeMap::new();
    for character in (0..=0x10ffff).filter_map(char::from_u32) {
        let folded = character.case_fold().collect::<String>();
        if !folded.starts_with(character) || folded.chars().count() != 1 {
            folding.insert(character, folded);
        }
    }
    let expected = serde_json::to_string(&folding)? + "\n";
    let file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packages/plugin-sdk/tools/unicode-case-fold.json");
    if check {
        if std::fs::read_to_string(&file)?.replace("\r\n", "\n") != expected {
            return Err("Web CLI case folding table has drifted".into());
        }
    } else {
        std::fs::write(file, expected)?;
    }
    Ok(())
}
