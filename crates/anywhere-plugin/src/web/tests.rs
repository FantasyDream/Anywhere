use super::*;
use anywhere_protocol::web_plugin::WebPermission;
use serde_json::{json, Value};
use std::io::{Cursor, Write};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

fn manifest() -> Value {
    json!({
        "schemaVersion": 1, "id": "dev.example.text-pad", "name": "Text Pad",
        "version": "1.0.0", "apiVersion": "1", "platforms": ["macos", "windows"],
        "entry": "dist/index.html", "permissions": [],
        "commands": [{"id": "open", "title": "Text Pad", "entrypoint": "workspace"}]
    })
}

#[test]
fn input_matchers_are_optional_bounded_and_require_the_reviewed_text_projection() {
    let old = parse_manifest(&serde_json::to_vec(&manifest()).unwrap()).unwrap();
    assert!(old.commands[0].input_matchers.is_empty());
    assert!(!old.commands[0].require_input_match);
    let mut value = manifest();
    value["commands"][0]["inputMatchers"] = json!(["json"]);
    assert!(parse_manifest(&serde_json::to_vec(&value).unwrap()).is_err());
    value["commands"][0]["accepts"] = json!(["core.text"]);
    assert!(parse_manifest(&serde_json::to_vec(&value).unwrap()).is_err());
    value["permissions"] = json!([{"name":"context.read","types":["core.text"]}]);
    assert!(parse_manifest(&serde_json::to_vec(&value).unwrap()).is_ok());
    value["commands"][0]["inputMatchers"] = json!(["json", "json_escaped"]);
    assert!(parse_manifest(&serde_json::to_vec(&value).unwrap()).is_ok());
    for invalid in [
        json!(["json", "json"]),
        json!(["arbitrary_javascript"]),
        json!([{"script":"run()"}]),
    ] {
        value["commands"][0]["inputMatchers"] = invalid;
        assert!(parse_manifest(&serde_json::to_vec(&value).unwrap()).is_err());
    }
}

#[test]
fn input_required_commands_must_declare_a_matcher() {
    let mut value = manifest();
    value["commands"][0]["requireInputMatch"] = json!(true);
    assert!(parse_manifest(&serde_json::to_vec(&value).unwrap()).is_err());
    value["commands"][0]["inputMatchers"] = json!(["json"]);
    value["commands"][0]["accepts"] = json!(["core.text"]);
    value["permissions"] = json!([{"name":"context.read","types":["core.text"]}]);
    let parsed = parse_manifest(&serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(parsed.commands[0].require_input_match);
}

fn base() -> Vec<(String, Vec<u8>)> {
    vec![
        (
            "manifest.json".into(),
            serde_json::to_vec(&manifest()).unwrap(),
        ),
        (
            "dist/index.html".into(),
            b"<!doctype html><script src=main.js></script>".to_vec(),
        ),
        ("dist/main.js".into(), b"console.log('probe');".to_vec()),
    ]
}

fn pack(files: &[(String, Vec<u8>)], compression: CompressionMethod) -> Vec<u8> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(compression);
    for (name, bytes) in files {
        if name.ends_with('/') {
            zip.add_directory(name, options).unwrap();
        } else {
            zip.start_file(name, options).unwrap();
            zip.write_all(bytes).unwrap();
        }
    }
    zip.finish().unwrap().into_inner()
}

fn package(files: &[(String, Vec<u8>)]) -> VerifiedWebPackage {
    VerifiedWebPackage::read(Cursor::new(pack(files, CompressionMethod::Stored)))
        .unwrap_or_else(|e| panic!("valid fixture rejected: {e:#}"))
}

fn rejected(files: &[(String, Vec<u8>)]) {
    assert!(VerifiedWebPackage::read(Cursor::new(pack(files, CompressionMethod::Stored))).is_err());
}

fn central_offsets(bytes: &[u8]) -> Vec<usize> {
    let end = bytes.len() - 22;
    let mut at = u32::from_le_bytes(bytes[end + 16..end + 20].try_into().unwrap()) as usize;
    let mut offsets = vec![];
    while at < end {
        offsets.push(at);
        let lengths: usize = [28, 30, 32]
            .into_iter()
            .map(|n| u16::from_le_bytes(bytes[at + n..at + n + 2].try_into().unwrap()) as usize)
            .sum();
        at += 46 + lengths;
    }
    offsets
}

#[test]
fn web_manifest_roundtrips_typed_permissions_without_legacy_privileges() {
    let mut value = manifest();
    value["permissions"] = json!([
        {"name":"context.read", "types":["core.text","core.file_selection"]},
        {"name":"context.currentApp"}, {"name":"clipboard.readText"},
        {"name":"clipboard.writeText"}, {"name":"storage.local"}
    ]);
    value["commands"][0]["accepts"] = json!(["git.repository"]);
    let parsed = parse_manifest(&serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(matches!(
        parsed.permissions[2],
        WebPermission::ClipboardReadText {}
    ));
    assert_eq!(
        parsed,
        parse_manifest(&serde_json::to_vec(&parsed).unwrap()).unwrap()
    );
}

#[test]
fn web_manifest_rejects_unknown_duplicate_and_malformed_fields_at_every_level() {
    for pointer in ["/extra", "/commands/0/extra", "/permissions/0/extra"] {
        let mut value = manifest();
        value["permissions"] = json!([{"name":"clipboard.writeText"}]);
        if pointer == "/extra" {
            value["extra"] = json!(true);
        }
        if pointer == "/commands/0/extra" {
            value["commands"][0]["extra"] = json!(true);
        }
        if pointer == "/permissions/0/extra" {
            value["permissions"][0]["extra"] = json!(true);
        }
        assert!(parse_manifest(&serde_json::to_vec(&value).unwrap()).is_err());
    }
    let raw = serde_json::to_string(&manifest()).unwrap();
    assert!(parse_manifest(
        raw.replacen(
            "\"schemaVersion\":1",
            "\"schemaVersion\":1,\"schemaVersion\":1",
            1
        )
        .as_bytes()
    )
    .is_err());
    for malformed in [b"[]".as_slice(), b"null", b"{", &[0xff]] {
        assert!(parse_manifest(malformed).is_err());
    }
    assert!(parse_manifest(&vec![b' '; MAX_MANIFEST_BYTES + 1]).is_err());
}

#[test]
fn web_manifest_versions_ids_and_platforms_fail_closed() {
    for (field, invalid) in [
        ("schemaVersion", json!(2)),
        ("apiVersion", json!("2")),
        ("version", json!("1.0")),
        ("version", json!("01.0.0")),
        ("id", json!("core.example")),
        ("id", json!("anywhere.tools")),
        ("id", json!("missingnamespace")),
        ("id", json!("dev.UPPER")),
        ("id", json!("dev.example:other")),
        ("name", json!("\nBad")),
        ("platforms", json!([])),
        ("platforms", json!(["linux"])),
        ("platforms", json!(["macos", "macos"])),
        ("entry", json!("https://example.com/index.html")),
        ("entry", json!("../index.html")),
        ("entry", json!("dist/main.js")),
    ] {
        let mut value = manifest();
        value[field] = invalid;
        assert!(
            parse_manifest(&serde_json::to_vec(&value).unwrap()).is_err(),
            "field: {field}"
        );
    }
    let mut value = manifest();
    value["version"] = json!("1.2.3-beta.1+build.2");
    assert!(parse_manifest(&serde_json::to_vec(&value).unwrap()).is_ok());
}

#[test]
fn web_manifest_rejects_duplicate_ids_capabilities_and_overbroad_projections() {
    for permissions in [
        json!([{"name":"process.start"}]),
        json!([{"name":"execute_builtin"}]),
        json!([{"name":"context.read", "types":[]}]),
        json!([{"name":"context.read", "types":["core.text", "core.text"]}]),
        json!([{"name":"context.read", "types":["git.repository"]}]),
        json!([{"name":"storage.local"}, {"name":"storage.local"}]),
        json!([{"name":"clipboard.readText", "types":["core.text"]}]),
    ] {
        let mut value = manifest();
        value["permissions"] = permissions;
        assert!(parse_manifest(&serde_json::to_vec(&value).unwrap()).is_err());
    }
    for commands in [
        json!([]),
        json!([manifest()["commands"][0], manifest()["commands"][0]]),
        json!([{"id":"open.child", "title":"Open", "entrypoint":"workspace"}]),
    ] {
        let mut value = manifest();
        value["commands"] = commands;
        assert!(parse_manifest(&serde_json::to_vec(&value).unwrap()).is_err());
    }
    let mut typed = parse_manifest(&serde_json::to_vec(&manifest()).unwrap()).unwrap();
    typed.commands[0].keywords = vec!["repeat".into(); 17];
    assert!(
        validate_manifest(&typed).is_err(),
        "direct DTO construction is revalidated"
    );
}

#[test]
fn complete_package_exposes_verified_bounded_assets_and_stable_ids() {
    let files = base();
    let package = package(&files);
    assert_eq!(package.manifest().id, "dev.example.text-pad");
    assert_eq!(package.assets().count(), 3);
    assert_eq!(
        package.action_id("open").as_deref(),
        Some("web:dev.example.text-pad:open")
    );
    assert_eq!(package.action_id("missing"), None);
    assert_eq!(
        package.read_asset("dist/main.js", 4096).unwrap(),
        files[2].1
    );
    for path in [
        "dist/../main.js",
        "Dist/main.js",
        "dist/%6dain.js",
        "missing.js",
    ] {
        assert!(package.read_asset(path, 4096).is_err());
    }
    assert!(package.read_asset("dist/main.js", 2).is_err());
    assert_eq!(package.digest().len(), 64);
    assert!(package.assets().all(|asset| asset.sha256().len() == 64));
}

#[test]
fn digest_covers_js_css_and_manifest_but_not_zip_order_or_compression() {
    let files = base();
    let original = package(&files);
    let mut reordered = files.clone();
    reordered.reverse();
    assert_eq!(original.digest(), package(&reordered).digest());
    let deflated =
        VerifiedWebPackage::read(Cursor::new(pack(&files, CompressionMethod::Deflated))).unwrap();
    assert_eq!(original.digest(), deflated.digest());
    let mut with_empty_dir = files.clone();
    with_empty_dir.push(("empty/".into(), vec![]));
    assert_eq!(original.digest(), package(&with_empty_dir).digest());
    for member in [0, 1, 2] {
        let mut changed = files.clone();
        changed[member].1.push(b' ');
        let updated = package(&changed);
        assert_ne!(original.digest(), updated.digest());
        assert_eq!(original.action_id("open"), updated.action_id("open"));
    }
    let mut with_css = files.clone();
    with_css.push(("dist/style.css".into(), b"body{}".to_vec()));
    assert_ne!(original.digest(), package(&with_css).digest());
}

#[test]
fn command_identity_survives_versions_but_not_command_renames() {
    let original = package(&base());
    let mut files = base();
    let mut value = manifest();
    value["version"] = json!("2.0.0");
    files[0].1 = serde_json::to_vec(&value).unwrap();
    let updated = package(&files);
    assert_eq!(original.action_id("open"), updated.action_id("open"));
    assert_ne!(original.digest(), updated.digest());
    value["commands"][0]["id"] = json!("edit");
    files[0].1 = serde_json::to_vec(&value).unwrap();
    let renamed = package(&files);
    assert_eq!(renamed.action_id("open"), None);
    assert_ne!(original.action_id("open"), renamed.action_id("edit"));
}

#[test]
fn rejects_missing_manifest_entry_icon_and_oversized_icon() {
    let mut files = base();
    files.remove(0);
    rejected(&files);
    let mut files = base();
    files.remove(1);
    rejected(&files);
    let mut files = base();
    let mut value = manifest();
    value["icon"] = json!("icon.png");
    files[0].1 = serde_json::to_vec(&value).unwrap();
    rejected(&files);
    files.push(("icon.png".into(), vec![1; 256 * 1024 + 1]));
    rejected(&files);
}

#[test]
fn declared_icon_is_decoded_with_dimension_and_integrity_limits() {
    fn png(width: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, width, 2);
            encoder.set_color(png::ColorType::Rgba);
            let mut writer = encoder.write_header().unwrap();
            writer
                .write_image_data(&vec![128; width as usize * 2 * 4])
                .unwrap();
        }
        bytes
    }
    let mut files = base();
    let mut value = manifest();
    value["icon"] = json!("icon.png");
    files[0].1 = serde_json::to_vec(&value).unwrap();
    files.push(("icon.png".into(), png(2)));
    assert!(package(&files).read_asset("icon.png", 256 * 1024).is_ok());
    for invalid in [png(513), b"not a png".to_vec(), png(2)[..24].to_vec()] {
        files.last_mut().unwrap().1 = invalid;
        rejected(&files);
    }
}

#[test]
fn portable_paths_reject_escape_encodings_windows_devices_and_controls() {
    for path in [
        "../x.js",
        "/x.js",
        "C:/x.js",
        "\\server\\x.js",
        "dist/../x.js",
        "dist//x.js",
        "./x.js",
        "dist/%2e.js",
        "dist/x.js?query",
        "dist/x#frag.js",
        "dist/x.js ",
        "dist/x.js.",
        "dist/CON.js",
        "dist/nul.txt",
        "dist/COM1.txt",
        "dist/LPT³.txt",
        "dist/a:b.js",
        "dist/CONOUT$.js",
        "dist/x\0.js",
        "dist/x\n.js",
    ] {
        let mut files = base();
        files.push((path.into(), b"x".to_vec()));
        rejected(&files);
    }
    assert!(path::validate(&format!("{}.js", "x".repeat(256))).is_err());
    assert!(path::validate(&format!("{}x.js", "d/".repeat(17))).is_err());
}

#[test]
fn rejects_unicode_case_and_file_directory_collisions_in_both_orders() {
    for pair in [
        ("dist/A.js", "dist/a.js"),
        ("Dist/a.js", "dist/b.js"),
        ("dist/café.js", "dist/cafe\u{301}.js"),
        ("dist/STRASSE.js", "dist/straße.js"),
        ("assets/x.js", "assets"),
        ("assets", "assets/x.js"),
    ] {
        let mut files = base();
        // Use a permitted file extension for the file-as-directory collision.
        let first = if pair.0 == "assets" {
            "assets.js"
        } else {
            pair.0
        };
        let second = if pair.1 == "assets" {
            "assets.js"
        } else {
            pair.1
        };
        let first = first.replace("assets/", "assets.js/");
        let second = second.replace("assets/", "assets.js/");
        files.extend([(first, b"x".to_vec()), (second, b"y".to_vec())]);
        rejected(&files);
    }
}

#[test]
fn rejects_actual_duplicate_entries_even_when_zip_map_would_hide_them() {
    let mut files = base();
    files.extend([("dist/a.js".into(), vec![1]), ("dist/b.js".into(), vec![2])]);
    let mut bytes = pack(&files, CompressionMethod::Stored);
    for at in (0..bytes.len().saturating_sub(9)).rev() {
        if bytes[at..].starts_with(b"dist/b.js") {
            bytes[at..at + 9].copy_from_slice(b"dist/a.js");
        }
    }
    assert!(VerifiedWebPackage::read(Cursor::new(bytes)).is_err());
}

#[test]
fn refuses_symlinks_special_files_and_executable_assets() {
    let mut writer = ZipWriter::new(Cursor::new(vec![]));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    for (name, bytes) in base() {
        writer.start_file(name, options).unwrap();
        writer.write_all(&bytes).unwrap();
    }
    writer
        .add_symlink("dist/link.js", "../../outside.js", options)
        .unwrap();
    assert!(VerifiedWebPackage::read(Cursor::new(writer.finish().unwrap().into_inner())).is_err());
    for name in [
        "install.sh",
        "native.dylib",
        "native.dll",
        "payload.exe",
        "source.dart",
    ] {
        let mut files = base();
        files.push((name.into(), vec![1]));
        rejected(&files);
    }
    let mut bytes = pack(&base(), CompressionMethod::Stored);
    let at = central_offsets(&bytes)[2];
    bytes[at + 5] = 3; // Unix system
    bytes[at + 38..at + 42].copy_from_slice(&(0o060644u32 << 16).to_le_bytes());
    assert!(VerifiedWebPackage::read(Cursor::new(bytes)).is_err());
}

#[test]
fn fails_crc_local_header_mismatch_and_truncated_archives() {
    let original = pack(&base(), CompressionMethod::Stored);
    let mut crc = original.clone();
    let at = crc.windows(11).position(|s| s == b"console.log").unwrap();
    crc[at] ^= 1;
    assert!(VerifiedWebPackage::read(Cursor::new(crc)).is_err());
    let mut header = original.clone();
    header[30] = b'X';
    assert!(VerifiedWebPackage::read(Cursor::new(header)).is_err());
    for length in [0, 3, 21, original.len() - 1] {
        assert!(VerifiedWebPackage::read(Cursor::new(&original[..length])).is_err());
    }
}

#[test]
fn bounds_directory_count_before_metadata_map_allocation() {
    let mut bytes = pack(&base(), CompressionMethod::Stored);
    let end = bytes.len() - 22;
    for offset in [8, 10] {
        bytes[end + offset..end + offset + 2]
            .copy_from_slice(&((MAX_ENTRIES + 1) as u16).to_le_bytes());
    }
    let error = VerifiedWebPackage::read(Cursor::new(bytes)).err().unwrap();
    assert!(error.to_string().contains("entry count"));
    let mut bytes = pack(&base(), CompressionMethod::Stored);
    let end = bytes.len() - 22;
    for offset in [8, 10] {
        bytes[end + offset..end + offset + 2].copy_from_slice(&1u16.to_le_bytes());
    }
    assert!(VerifiedWebPackage::read(Cursor::new(bytes)).is_err());
}

#[test]
fn blocks_zip_bombs_before_expanding_and_limits_header_claims() {
    let mut files = base();
    files.push(("dist/bomb.txt".into(), vec![b'A'; 256 * 1024]));
    let error = VerifiedWebPackage::read(Cursor::new(pack(&files, CompressionMethod::Deflated)))
        .err()
        .unwrap();
    assert!(error.to_string().contains("compression ratio"));
    let mut bytes = pack(&base(), CompressionMethod::Stored);
    let at = central_offsets(&bytes)[2];
    let local = u32::from_le_bytes(bytes[at + 42..at + 46].try_into().unwrap()) as usize;
    let size = (MAX_ASSET_BYTES + 1) as u32;
    bytes[at + 24..at + 28].copy_from_slice(&size.to_le_bytes());
    bytes[local + 22..local + 26].copy_from_slice(&size.to_le_bytes());
    let error = VerifiedWebPackage::read(Cursor::new(bytes)).err().unwrap();
    assert!(error.to_string().contains("size limit"), "{error:#}");
}

#[test]
fn expanded_package_budget_is_checked_before_asset_inflation() {
    let mut files = base();
    for n in 0..6 {
        files.push((format!("dist/large{n}.txt"), vec![b'X'; 128 * 1024]));
    }
    let mut bytes = pack(&files, CompressionMethod::Stored);
    for at in central_offsets(&bytes).into_iter().skip(3) {
        let local = u32::from_le_bytes(bytes[at + 42..at + 46].try_into().unwrap()) as usize;
        let size = MAX_ASSET_BYTES as u32;
        bytes[at + 24..at + 28].copy_from_slice(&size.to_le_bytes());
        bytes[local + 22..local + 26].copy_from_slice(&size.to_le_bytes());
    }
    let error = VerifiedWebPackage::read(Cursor::new(bytes)).err().unwrap();
    assert!(
        error.to_string().contains("expanded size limit"),
        "{error:#}"
    );
}

#[test]
fn encrypted_resources_are_rejected_without_requesting_passwords() {
    let mut bytes = pack(&base(), CompressionMethod::Stored);
    let at = central_offsets(&bytes)[2];
    let local = u32::from_le_bytes(bytes[at + 42..at + 46].try_into().unwrap()) as usize;
    for start in [at + 8, local + 6] {
        let flags = u16::from_le_bytes(bytes[start..start + 2].try_into().unwrap()) | 1;
        bytes[start..start + 2].copy_from_slice(&flags.to_le_bytes());
    }
    assert!(VerifiedWebPackage::read(Cursor::new(bytes)).is_err());
}

#[test]
fn archive_input_reads_at_most_compressed_limit_plus_one() {
    struct Endless {
        read: usize,
    }
    impl std::io::Read for Endless {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            buf.fill(0);
            self.read += buf.len();
            Ok(buf.len())
        }
    }
    let mut source = Endless { read: 0 };
    assert!(VerifiedWebPackage::read(&mut source).is_err());
    assert_eq!(source.read, MAX_ARCHIVE_BYTES + 1);
}

#[test]
fn rejects_comments_multidisk_and_zip64_in_the_restricted_envelope() {
    let original = pack(&base(), CompressionMethod::Stored);
    let mut comment = original.clone();
    let end = comment.len() - 22;
    comment[end + 20..end + 22].copy_from_slice(&1u16.to_le_bytes());
    comment.push(b'x');
    assert!(VerifiedWebPackage::read(Cursor::new(comment)).is_err());
    let mut multidisk = original.clone();
    let end = multidisk.len() - 22;
    multidisk[end + 4..end + 6].copy_from_slice(&1u16.to_le_bytes());
    assert!(VerifiedWebPackage::read(Cursor::new(multidisk)).is_err());
    let mut zip64 = original;
    zip64[18..22].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(VerifiedWebPackage::read(Cursor::new(zip64)).is_err());
}

#[test]
fn rejects_link_extra_fields_without_interpreting_them() {
    let mut bytes = pack(&base(), CompressionMethod::Stored);
    let at = central_offsets(&bytes)[0];
    let name_len = u16::from_le_bytes(bytes[at + 28..at + 30].try_into().unwrap()) as usize;
    bytes[at + 30..at + 32].copy_from_slice(&4u16.to_le_bytes());
    bytes.splice(
        at + 46 + name_len..at + 46 + name_len,
        [0x0d, 0x00, 0x00, 0x00],
    );
    let end = bytes.len() - 22;
    let size = u32::from_le_bytes(bytes[end + 12..end + 16].try_into().unwrap()) + 4;
    bytes[end + 12..end + 16].copy_from_slice(&size.to_le_bytes());
    assert!(VerifiedWebPackage::read(Cursor::new(bytes)).is_err());
}

#[test]
fn background_manifests_validate_export_names_owned_types_and_input_grants() {
    let mut value = manifest();
    value.as_object_mut().unwrap().remove("entry");
    value["background"] = json!({"entry":"dist/background.mjs","resolvers":[{"id":"parse","accepts":["core.text"],"produces":["plugin.dev.example.text-pad.value"],"entrypoint":"parseContext"}]});
    value["commands"][0]["runtime"] = json!("background");
    value["commands"][0]["entrypoint"] = json!("copyValue");
    value["permissions"] = json!([{"name":"context.read","types":["core.text"]}]);
    assert!(parse_manifest(&serde_json::to_vec(&value).unwrap()).is_ok());
    for export in ["1copy", "object.copy", "copy-value", ""] {
        let mut bad = value.clone();
        bad["commands"][0]["entrypoint"] = json!(export);
        assert!(parse_manifest(&serde_json::to_vec(&bad).unwrap()).is_err());
    }
    let mut bad = value.clone();
    bad["background"]["resolvers"][0]["produces"] = json!(["core.file"]);
    assert!(parse_manifest(&serde_json::to_vec(&bad).unwrap()).is_err());
    let mut bad = value.clone();
    bad["permissions"] = json!([]);
    assert!(parse_manifest(&serde_json::to_vec(&bad).unwrap()).is_err());
    value["commands"] = json!([]);
    assert!(parse_manifest(&serde_json::to_vec(&value).unwrap()).is_ok());
}
