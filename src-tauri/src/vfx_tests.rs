use super::*;
use image::GenericImageView;
use tempfile::TempDir;
fn call(root: &Path, p: Option<&str>, e: Option<&str>, v: Value) -> Value {
    dispatch(root, p, e, serde_json::from_value(v).unwrap()).unwrap()
}
fn setup_at(root: &Path) -> (String, String) {
    fs::create_dir_all(root).unwrap();
    let p = call(
        root,
        None,
        None,
        json!({"operation":"create_project","name":"Synthetic VFX round-trip"}),
    )["createdProjectId"]
        .as_str()
        .unwrap()
        .to_string();
    let e=call(root,Some(&p),None,json!({"operation":"create_effect","effect_id":"effect.contact.light","name":"Synthetic Light Contact","package":"Draft example only","profile":Profile::study()}))["effect"]["id"].as_str().unwrap().to_string();
    (p, e)
}
fn setup() -> (TempDir, String, String) {
    let t = tempfile::tempdir().unwrap();
    let (p, e) = setup_at(t.path());
    (t, p, e)
}
fn frame(i: usize) -> RgbaImage {
    let mut im = RgbaImage::new(24, 24);
    let c = Rgba([255, 241, 210, 255]);
    im.put_pixel(11, 11, c);
    im.put_pixel(12, 11, c);
    im.put_pixel(12, 12, c);
    if i == 1 {
        im.put_pixel(2, 3, Rgba([201, 194, 184, 255]));
        for x in 13..21 {
            im.put_pixel(x, 12, c);
        }
        im.put_pixel(20, 13, Rgba([56, 52, 75, 255]));
    }
    if i == 2 {
        im.put_pixel(8, 18, Rgba([201, 194, 184, 255]));
    }
    im
}
fn imported(root: &Path, p: &str, e: &str) -> Value {
    let paths: Vec<_> = (0..3)
        .map(|i| {
            let path = root.join(format!("synthetic-input-{i}.png"));
            fs::write(&path, png(&frame(i)).unwrap()).unwrap();
            path.to_string_lossy().into_owned()
        })
        .collect();
    let proposal = call(
        root,
        Some(p),
        Some(e),
        json!({"operation":"import_frames","paths":paths,"board_count":null,"rectangles":null}),
    );
    let imp = proposal["effect"]["pendingImportId"].as_str().unwrap();
    call(
        root,
        Some(p),
        Some(e),
        json!({"operation":"approve_import","import_id":imp,"reviewer":"Synthetic test owner"}),
    )
}
fn processed(root: &Path, p: &str, e: &str) -> Value {
    call(
        root,
        Some(p),
        Some(e),
        json!({"operation":"process","profile":Profile::study(),"settings":vec![Processing::default();3],"holds":[2,3,3]}),
    )
}
fn approved(root: &Path, p: &str, e: &str) -> Value {
    let r = processed(root, p, e);
    call(
        root,
        Some(p),
        Some(e),
        json!({"operation":"approve","revision_id":r["effect"]["activeRevisionId"],"reviewer":"Synthetic test owner","checks":{"artwork":true,"regionsAndAnchor":true,"palette":true,"timing":true,"hitStopAndCompletion":true}}),
    )
}
#[test]
fn fixed_origin_keeps_asymmetric_sparks_and_single_pixel_motes() {
    let profile = Profile::study();
    for i in 0..3 {
        let source = frame(i);
        let (out, clipped) = process(&source, &Processing::default(), &profile).unwrap();
        assert_eq!(out, source);
        assert_eq!(clipped, 0);
        assert_eq!(out.get_pixel(12, 12), source.get_pixel(12, 12));
        assert!(inspect(&out, &profile, i == 2, clipped).errors.is_empty());
    }
    assert_eq!(frame(1).get_pixel(2, 3)[3], 255);
}
#[test]
fn alpha_threshold_chroma_and_hidden_rgb_are_explicit() {
    let mut source = frame(1);
    source.put_pixel(0, 0, Rgba([0, 255, 0, 255]));
    source.put_pixel(5, 5, Rgba([9, 8, 7, 0]));
    source.put_pixel(6, 5, Rgba([255, 241, 210, 90]));
    let profile = Profile::study();
    let (out, _) = process(&source, &Processing::default(), &profile).unwrap();
    assert_eq!(out.get_pixel(0, 0)[3], 255);
    assert_eq!(out.get_pixel(5, 5), &Rgba([0, 0, 0, 0]));
    assert_eq!(out.get_pixel(6, 5)[3], 90);
    assert!(!inspect(&out, &profile, false, 0).errors.is_empty());
    let mut p = Processing::default();
    p.chroma = Some("#00FF00".into());
    p.alpha_threshold = Some(128);
    let (out, _) = process(&source, &p, &profile).unwrap();
    assert_eq!(out.get_pixel(0, 0)[3], 0);
    assert_eq!(out.get_pixel(6, 5)[3], 0);
    assert_eq!(out.get_pixel(2, 3)[3], 255);
    assert!(inspect(&out, &profile, false, 0).errors.is_empty());
}
#[test]
fn legitimate_empty_terminal_is_never_filled() {
    let mut profile = Profile::study();
    let empty = RgbaImage::new(24, 24);
    assert!(!inspect(&empty, &profile, true, 0).errors.is_empty());
    profile.allow_empty_terminal = true;
    profile.completion = "empty_terminal".into();
    assert!(inspect(&empty, &profile, true, 0).errors.is_empty());
    assert!(!inspect(&empty, &profile, false, 0).errors.is_empty());
    assert_eq!(
        process(&empty, &Processing::default(), &profile).unwrap().0,
        empty
    );
}
#[test]
fn no_clipping_or_implicit_native_scaling() {
    let profile = Profile::study();
    let mut p = Processing::default();
    p.offset = [5, 0];
    let (_, clip) = process(&frame(1), &p, &profile).unwrap();
    assert!(clip > 0);
    let (out, _) = process(&frame(1), &p, &profile).unwrap();
    assert!(!inspect(&out, &profile, false, clip).errors.is_empty());
    let padded = RgbaImage::new(48, 48);
    assert_eq!(
        process(&padded, &Processing::default(), &profile)
            .unwrap()
            .0
            .dimensions(),
        (24, 24)
    );
    p.pixel_pitch = 7;
    assert!(process(&padded, &p, &profile).is_err());
    p.offset = [i32::MIN, 0];
    assert!(process(&frame(1), &p, &profile).is_err());
}
#[test]
fn palette_validation_does_not_silently_remap() {
    let mut im = frame(0);
    im.put_pixel(3, 3, Rgba([200, 190, 180, 255]));
    let profile = Profile::study();
    let (out, _) = process(&im, &Processing::default(), &profile).unwrap();
    assert_eq!(out.get_pixel(3, 3), im.get_pixel(3, 3));
    assert_eq!(
        inspect(&out, &profile, false, 0).unexpected_colors,
        vec!["#C8BEB4"]
    );
    let mut p = Processing::default();
    p.remap_palette = true;
    let (out, _) = process(&im, &p, &profile).unwrap();
    assert!(inspect(&out, &profile, false, 0)
        .unexpected_colors
        .is_empty());
    assert_eq!(profile.palette.authority_id, None);
}
#[test]
fn regions_require_explicit_approval_and_preserve_order_bytes_padding() {
    let (t, p, e) = setup();
    let mut board = RgbaImage::new(72, 24);
    for i in 0..3 {
        imageops::replace(&mut board, &frame(i), i as i64 * 24, 0);
    }
    let path = t.path().join("board.png");
    let bytes = png(&board).unwrap();
    fs::write(&path, &bytes).unwrap();
    let proposal = call(
        t.path(),
        Some(&p),
        Some(&e),
        json!({"operation":"import_frames","paths":[path],"board_count":3,"rectangles":null}),
    );
    let s: Effect = serde_json::from_value(proposal["effect"].clone()).unwrap();
    assert_eq!(
        checked_bytes(t.path(), &s.sources[0].path, &s.sources[0].sha256).unwrap(),
        bytes
    );
    for i in 0..3 {
        assert_eq!(
            region_image(t.path(), &s, &s.imports[0].regions[i]).unwrap(),
            frame(i)
        );
    }
    assert!(dispatch(
        t.path(),
        Some(&p),
        Some(&e),
        Request::Process {
            profile: Profile::study(),
            settings: vec![Processing::default(); 3],
            holds: vec![2, 3, 3]
        }
    )
    .unwrap_err()
    .contains("Approve"));
    assert!(s.revisions.is_empty());
}
#[test]
fn explicit_sheet_rectangles_keep_detached_fragments() {
    let (t, p, e) = setup();
    let mut board = RgbaImage::new(80, 30);
    imageops::replace(&mut board, &frame(1), 4, 3);
    let file = t.path().join("transparent-sheet.png");
    fs::write(&file, png(&board).unwrap()).unwrap();
    let r = call(
        t.path(),
        Some(&p),
        Some(&e),
        json!({"operation":"import_frames","paths":[file],"board_count":null,"rectangles":[[4,3,24,24]]}),
    );
    let s: Effect = serde_json::from_value(r["effect"].clone()).unwrap();
    assert_eq!(
        region_image(t.path(), &s, &s.imports[0].regions[0]).unwrap(),
        frame(1)
    );
}
#[test]
fn approval_and_positive_integer_holds_are_real_export_gates() {
    let (t, p, e) = setup();
    imported(t.path(), &p, &e);
    assert!(dispatch(
        t.path(),
        Some(&p),
        Some(&e),
        Request::Process {
            profile: Profile::study(),
            settings: vec![Processing::default(); 3],
            holds: vec![2, 0, 3]
        }
    )
    .is_err());
    assert!(serde_json::from_value::<Request>(json!({"operation":"process","profile":Profile::study(),"settings":vec![Processing::default();3],"holds":[2,2.5,3]})).is_err());
    let r = processed(t.path(), &p, &e);
    let rev = r["effect"]["activeRevisionId"].as_str().unwrap();
    assert!(dispatch(
        t.path(),
        Some(&p),
        Some(&e),
        Request::Export {
            revision_id: rev.into(),
            gutter: 0
        }
    )
    .unwrap_err()
    .contains("approval"));
    assert!(dispatch(
        t.path(),
        Some(&p),
        Some(&e),
        Request::Approve {
            revision_id: "different-target".into(),
            reviewer: "Owner".into(),
            checks: Checks {
                artwork: true,
                regions_and_anchor: true,
                palette: true,
                timing: true,
                hit_stop_and_completion: true
            }
        }
    )
    .is_err());
}
fn round_trip(root: &Path) -> Value {
    let (p, e) = setup_at(root);
    imported(root, &p, &e);
    approved(root, &p, &e);
    let (state, _) = load(root, &p, &e).unwrap();
    let rev = state.active_revision_id.unwrap();
    let first = call(
        root,
        Some(&p),
        Some(&e),
        json!({"operation":"export","revision_id":rev,"gutter":0}),
    );
    let second = call(
        root,
        Some(&p),
        Some(&e),
        json!({"operation":"export","revision_id":rev,"gutter":0}),
    );
    let exports = second["effect"]["exports"].as_array().unwrap();
    assert_eq!(exports[0]["pngHash"], exports[1]["pngHash"]);
    assert_eq!(exports[0]["manifestHash"], exports[1]["manifestHash"]);
    let dir = root.join(exports[0]["path"].as_str().unwrap());
    let sheet = decode(&fs::read(dir.join("spritesheet.png")).unwrap()).unwrap();
    assert_eq!(sheet.dimensions(), (72, 24));
    for i in 0..3 {
        assert_eq!(
            imageops::crop_imm(&sheet, i * 24, 0, 24, 24).to_image(),
            frame(i as usize)
        );
    }
    let manifest: Value =
        serde_json::from_slice(&fs::read(dir.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["contactAnchor"], json!([12, 12]));
    assert_eq!(manifest["totalDurationTicks"], 8);
    assert_eq!(manifest["loop"], false);
    assert_eq!(manifest["hitStopPolicy"], "pause");
    assert_eq!(manifest["palette"]["authorityId"], Value::Null);
    assert_eq!(manifest["timingAuthority"], "explicit_hold_ticks");
    assert_eq!(manifest["engineAdapter"], "NOT_IMPLEMENTED");
    let verify = call(
        root,
        Some(&p),
        Some(&e),
        json!({"operation":"verify_export","export_id":exports[0]["id"]}),
    );
    assert_eq!(verify["result"]["valid"], true);
    let reopened = call(root, Some(&p), Some(&e), json!({"operation":"load"}));
    assert_eq!(reopened["effect"]["activeRevisionId"], rev);
    assert_eq!(
        reopened["effect"]["revisions"][0]["frames"][1]["holdTicks"],
        3
    );
    json!({"kind":"SYNTHETIC_ONLY_NOT_AI_CLEANUP_PROOF","projectId":p,"effectStorageId":e,"sheet":dir.join("spritesheet.png"),"manifest":dir.join("manifest.json"),"sources":dir.join("source.json"),"sheetSize":[72,24],"holds":[2,3,3],"totalTicks":8,"sha256":first["result"]["pngSha256"],"reopened":true,"deterministic":true})
}
#[test]
fn actual_synthetic_import_process_approve_export_reopen_round_trip() {
    let t = tempfile::tempdir().unwrap();
    round_trip(t.path());
}
#[test]
fn changed_source_export_and_approved_hashes_are_rejected() {
    let (t, p, e) = setup();
    imported(t.path(), &p, &e);
    let r = approved(t.path(), &p, &e);
    let rev = r["effect"]["activeRevisionId"].as_str().unwrap();
    let exported = call(
        t.path(),
        Some(&p),
        Some(&e),
        json!({"operation":"export","revision_id":rev,"gutter":0}),
    );
    let x = &exported["effect"]["exports"][0];
    let sheet = t
        .path()
        .join(x["path"].as_str().unwrap())
        .join("spritesheet.png");
    fs::write(sheet, b"corrupt").unwrap();
    assert!(dispatch(
        t.path(),
        Some(&p),
        Some(&e),
        Request::VerifyExport {
            export_id: x["id"].as_str().unwrap().into()
        }
    )
    .unwrap_err()
    .contains("Hash mismatch"));
    let raw = t
        .path()
        .join(r["effect"]["sources"][0]["path"].as_str().unwrap());
    fs::write(raw, png(&frame(2)).unwrap()).unwrap();
    assert!(dispatch(
        t.path(),
        Some(&p),
        Some(&e),
        Request::Export {
            revision_id: rev.into(),
            gutter: 0
        }
    )
    .is_err());
}
#[test]
fn revisions_and_other_effect_character_environment_state_survive() {
    let (t, p, e) = setup();
    let untouched = t.path().join("projects/character/project.json");
    fs::create_dir_all(untouched.parent().unwrap()).unwrap();
    fs::write(&untouched, b"original character metadata").unwrap();
    let environment = t
        .path()
        .join("environment-projects/world/environments/index.json");
    fs::create_dir_all(environment.parent().unwrap()).unwrap();
    fs::write(&environment, b"original environment metadata").unwrap();
    imported(t.path(), &p, &e);
    let approved = approved(t.path(), &p, &e);
    let rev = approved["effect"]["activeRevisionId"].clone();
    let old = serde_json::to_vec(&load(t.path(), &p, &e).unwrap().0).unwrap();
    let second = call(
        t.path(),
        Some(&p),
        None,
        json!({"operation":"create_effect","effect_id":"effect.dust","name":"Independent dust","package":"","profile":null}),
    );
    let second_id = second["effect"]["id"].as_str().unwrap();
    imported(t.path(), &p, second_id);
    processed(t.path(), &p, second_id);
    assert_eq!(
        serde_json::to_vec(&load(t.path(), &p, &e).unwrap().0).unwrap(),
        old
    );
    processed(t.path(), &p, &e);
    let restored = call(
        t.path(),
        Some(&p),
        Some(&e),
        json!({"operation":"restore","revision_id":rev}),
    );
    assert!(restored["effect"]["revisions"][0]["approval"].is_object());
    assert_eq!(fs::read(untouched).unwrap(), b"original character metadata");
    assert_eq!(
        fs::read(environment).unwrap(),
        b"original environment metadata"
    );
}
#[test]
fn damaged_metadata_recovers_without_overwriting_or_downgrading() {
    let (t, p, e) = setup();
    imported(t.path(), &p, &e);
    processed(t.path(), &p, &e);
    let dir = effect_dir(t.path(), &p, &e).unwrap();
    let primary = dir.join("index.json");
    fs::write(&primary, b"interrupted metadata").unwrap();
    let (_, notice) = load(t.path(), &p, &e).unwrap();
    assert!(notice.is_some());
    assert_eq!(fs::read(&primary).unwrap(), b"interrupted metadata");
    let mut value: Value =
        serde_json::from_slice(&fs::read(dir.join("index.previous.json")).unwrap()).unwrap();
    value["schemaVersion"] = json!(99);
    fs::write(&primary, serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(load(t.path(), &p, &e).unwrap_err().contains("Newer VFX"));
}
#[test]
fn dimensions_are_game_independent_and_paths_confined() {
    let (t, p, e) = setup();
    let mut profile = Profile::study();
    profile.cell = [40, 18];
    profile.anchor = [7, 8];
    assert!(validate_profile(&profile).is_ok());
    profile.anchor = [64, 112];
    assert!(validate_profile(&profile).is_err());
    assert!(effect_dir(t.path(), "../other", &e).is_err());
    assert!(checked_bytes(t.path(), "../outside.png", "fake").is_err());
    assert!(dispatch(
        t.path(),
        Some(&p),
        Some(&e),
        Request::SetRegions {
            regions: vec![Region {
                source_id: "missing".into(),
                rect: [0, 0, 4, 4]
            }]
        }
    )
    .is_err());
}
#[test]
fn draft_offsets_holds_palette_and_cell_reopen_without_processing_or_affecting_exports() {
    let (t, p, e) = setup();
    imported(t.path(), &p, &e);
    let a = approved(t.path(), &p, &e);
    let revision = a["effect"]["activeRevisionId"].as_str().unwrap();
    let (state, _) = load(t.path(), &p, &e).unwrap();
    let mut settings = vec![Processing::default(); 3];
    settings[1].offset = [-1, 2];
    let draft = Draft {
        profile: Profile::study(),
        settings,
        holds: vec![2, 5, 3],
        base_revision_id: state.active_revision_id.clone(),
        import_id: state.active_import_id.clone(),
    };
    call(
        t.path(),
        Some(&p),
        Some(&e),
        json!({"operation":"save_draft","draft":draft}),
    );
    let reopened = load(t.path(), &p, &e).unwrap().0;
    assert_eq!(reopened.draft, Some(draft.clone()));
    assert!(reopened.revisions[0].approval.is_some());
    assert!(dispatch(
        t.path(),
        Some(&p),
        Some(&e),
        Request::Export {
            revision_id: revision.into(),
            gutter: 0
        }
    )
    .unwrap_err()
    .contains("draft"));
    let dir = effect_dir(t.path(), &p, &e).unwrap();
    fs::write(dir.join("index.json"), b"interrupted").unwrap();
    let (recovered, notice) = load(t.path(), &p, &e).unwrap();
    assert_eq!(recovered.draft, Some(draft));
    assert!(notice.is_some());
}
#[test]
fn stale_draft_cannot_replace_a_new_import_or_review_revision() {
    let (t, p, e) = setup();
    imported(t.path(), &p, &e);
    let state = load(t.path(), &p, &e).unwrap().0;
    let draft = Draft {
        profile: Profile::study(),
        settings: vec![Processing::default(); 3],
        holds: vec![2, 3, 3],
        base_revision_id: None,
        import_id: state.active_import_id,
    };
    processed(t.path(), &p, &e);
    assert!(
        dispatch(t.path(), Some(&p), Some(&e), Request::SaveDraft { draft })
            .unwrap_err()
            .contains("older")
    );
}
#[test]
fn unchanged_wire_draft_and_load_do_not_write_or_invalidate_approval() {
    let (t, p, e) = setup();
    imported(t.path(), &p, &e);
    let initial = approved(t.path(), &p, &e);
    let revision = initial["effect"]["activeRevisionId"].clone();
    let draft = json!({"profile":Profile::study(),"settings":vec![Processing::default();3],"holds":[2,3,3],"baseRevisionId":revision,"importId":initial["effect"]["activeImportId"]});
    let saved = call(
        t.path(),
        Some(&p),
        Some(&e),
        json!({"operation":"save_draft","draft":draft}),
    );
    let dir = effect_dir(t.path(), &p, &e).unwrap();
    let before = fs::read(dir.join("index.json")).unwrap();
    let history = fs::read_dir(dir.join("history")).unwrap().count();
    // Round-trip the actual serde_json::Value, with its key order, not a JS mock.
    for _ in 0..5 {
        let r = call(
            t.path(),
            Some(&p),
            Some(&e),
            json!({"operation":"save_draft","draft":saved["effect"]["draft"]}),
        );
        assert_eq!(r["effect"]["generation"], saved["effect"]["generation"]);
        assert_eq!(r["effect"]["activeRevisionId"], revision);
        assert!(!r["effect"]["revisions"][0]["approval"].is_null());
        call(t.path(), Some(&p), Some(&e), json!({"operation":"load"}));
        library(t.path()).unwrap();
    }
    assert_eq!(fs::read(dir.join("index.json")).unwrap(), before);
    assert_eq!(fs::read_dir(dir.join("history")).unwrap().count(), history);
    let effect = load(t.path(), &p, &e).unwrap().0;
    checked_approval(t.path(), &effect, &effect.revisions[0]).unwrap();
}

#[test]
#[ignore = "Read-only audit; requires SPRITE_STUDIO_VFX_AUDIT_WORKSPACE"]
fn audit_existing_light_contact_read_only() {
    let root = PathBuf::from(std::env::var("SPRITE_STUDIO_VFX_AUDIT_WORKSPACE").unwrap());
    let p = "5e11b831-4815-4b2a-acfe-29cdf804e462";
    let e = "4b2d5c7f-b504-4e11-a4a8-4facc1d5446d";
    let dir = effect_dir(&root, p, e).unwrap();
    let before = fs::read(dir.join("index.json")).unwrap();
    let r = call(&root, Some(p), Some(e), json!({"operation":"load"}));
    let state = load(&root, p, e).unwrap().0;
    let mut verified = 0;
    for source in &state.sources {
        checked_bytes(&root, &source.path, &source.sha256).unwrap();
        verified += 1;
    }
    for revision in &state.revisions {
        for frame in &revision.frames {
            checked_bytes(&root, &frame.path, &frame.sha256).unwrap();
            verified += 1;
        }
    }
    for export in &state.exports {
        verify_export(&root, &state, &export.id).unwrap();
    }
    assert_eq!(fs::read(dir.join("index.json")).unwrap(), before);
    println!("Read-only Light Contact audit: {} imports; {} revisions; {} hashes; {} exports; generation {}; metadata unchanged", state.imports.len(), state.revisions.len(), verified, state.exports.len(), state.generation);
    println!(
        "Saved import/revision IDs: {} / {}",
        r["effect"]["activeImportId"], r["effect"]["activeRevisionId"]
    );
}
#[test]
fn empty_terminal_and_large_transparent_padding_export_without_fake_pixels() {
    let (t, p, e) = setup();
    let paths: Vec<_> = (0..3)
        .map(|i| {
            let mut image = RgbaImage::new(48, 48);
            if i != 2 {
                imageops::replace(&mut image, &frame(i), 0, 0);
            }
            let path = t.path().join(format!("padding-{i}.png"));
            fs::write(&path, png(&image).unwrap()).unwrap();
            path
        })
        .collect();
    let imported = call(
        t.path(),
        Some(&p),
        Some(&e),
        json!({"operation":"import_frames","paths":paths,"board_count":null,"rectangles":null}),
    );
    call(
        t.path(),
        Some(&p),
        Some(&e),
        json!({"operation":"approve_import","import_id":imported["effect"]["pendingImportId"],"reviewer":"Owner"}),
    );
    let mut profile = Profile::study();
    profile.cell = [48, 48];
    profile.allow_empty_terminal = true;
    profile.completion = "empty_terminal".into();
    let r = call(
        t.path(),
        Some(&p),
        Some(&e),
        json!({"operation":"process","profile":profile,"settings":vec![Processing::default();3],"holds":[2,3,3]}),
    );
    let rev = r["effect"]["activeRevisionId"].as_str().unwrap();
    assert_eq!(
        r["effect"]["revisions"][0]["frames"][2]["report"]["opaquePixels"],
        0
    );
    call(
        t.path(),
        Some(&p),
        Some(&e),
        json!({"operation":"approve","revision_id":rev,"reviewer":"Owner","checks":{"artwork":true,"regionsAndAnchor":true,"palette":true,"timing":true,"hitStopAndCompletion":true}}),
    );
    let exported = call(
        t.path(),
        Some(&p),
        Some(&e),
        json!({"operation":"export","revision_id":rev,"gutter":0}),
    );
    let dir = PathBuf::from(exported["result"]["directory"].as_str().unwrap());
    let sheet = decode(&fs::read(dir.join("spritesheet.png")).unwrap()).unwrap();
    assert_eq!(sheet.dimensions(), (144, 48));
    assert!(imageops::crop_imm(&sheet, 96, 0, 48, 48)
        .pixels()
        .all(|(_, _, p)| p == Rgba([0, 0, 0, 0])));
}
#[test]
#[ignore = "Writes synthetic round-trip evidence only to explicitly supplied new SPRITE_STUDIO_VFX_DEMO_DIR"]
fn write_synthetic_round_trip_evidence() {
    let root = std::env::var("SPRITE_STUDIO_VFX_DEMO_DIR").expect("Set a new demo directory");
    let root = Path::new(&root);
    assert!(!root.exists(), "Never overwrite a workspace");
    let report = round_trip(root);
    fs::write(
        root.join("round-trip.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    println!("{}", root.join("round-trip.json").display());
}
