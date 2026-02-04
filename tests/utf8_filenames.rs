/// Tests for UTF-8 filename handling with Chinese characters and emojis.
///
/// This tests verifies that files with non-ASCII characters in their filenames
/// are correctly tracked and attributed when git-ai processes commits.
///
/// Issue: Files with Chinese (or other non-ASCII) characters in filenames were
/// incorrectly classified as human-written because git outputs such filenames
/// with octal escape sequences (e.g., `"\344\270\255\346\226\207.txt"` for "中文.txt").
mod repos;
use git_ai::authorship::stats::CommitStats;
use repos::test_file::ExpectedLineExt;
use repos::test_repo::TestRepo;

/// Extract the first complete JSON object from mixed stdout/stderr output.
fn extract_json_object(output: &str) -> String {
    let start = output.find('{').unwrap_or(0);
    let end = output.rfind('}').unwrap_or(output.len().saturating_sub(1));
    output[start..=end].to_string()
}

#[test]
fn test_chinese_filename_ai_attribution() {
    let repo = TestRepo::new();

    // Create an initial commit
    let mut readme = repo.filename("README.md");
    readme.set_contents(lines!["# Project"]);
    repo.stage_all_and_commit("Initial commit").unwrap();

    // AI creates a file with Chinese characters in the filename
    let mut chinese_file = repo.filename("中文文件.txt");
    chinese_file.set_contents(lines![
        "第一行".ai(),
        "第二行".ai(),
        "第三行".ai(),
    ]);

    // Commit the Chinese-named file
    let commit = repo.stage_all_and_commit("Add Chinese file").unwrap();

    // Verify the authorship log contains the Chinese filename
    assert_eq!(
        commit.authorship_log.attestations.len(),
        1,
        "Should have 1 attestation for the Chinese-named file"
    );
    assert_eq!(
        commit.authorship_log.attestations[0].file_path,
        "中文文件.txt",
        "File path should be the actual UTF-8 filename"
    );

    // Get stats and verify AI attribution is correct
    let raw = repo.git_ai(&["stats", "--json"]).unwrap();
    let json = extract_json_object(&raw);
    let stats: CommitStats = serde_json::from_str(&json).unwrap();

    // The key check: ai_additions should NOT be 0
    assert_eq!(
        stats.ai_additions, 3,
        "All 3 lines should be attributed to AI, not human"
    );
    assert_eq!(
        stats.human_additions, 0,
        "No lines should be attributed to human"
    );
    assert_eq!(
        stats.ai_accepted, 3,
        "All 3 AI lines should be counted as accepted"
    );
    assert_eq!(
        stats.git_diff_added_lines, 3,
        "Git should report 3 added lines"
    );
}

#[test]
fn test_emoji_filename_ai_attribution() {
    let repo = TestRepo::new();

    // Create an initial commit
    let mut readme = repo.filename("README.md");
    readme.set_contents(lines!["# Project"]);
    repo.stage_all_and_commit("Initial commit").unwrap();

    // AI creates a file with emoji in the filename
    let mut emoji_file = repo.filename("🚀rocket_launch.txt");
    emoji_file.set_contents(lines![
        "Launch sequence initiated".ai(),
        "Engines igniting".ai(),
        "Liftoff!".ai(),
        "Mission success".ai(),
    ]);

    // Commit the emoji-named file
    let commit = repo.stage_all_and_commit("Add emoji file").unwrap();

    // Verify the authorship log contains the emoji filename
    assert_eq!(
        commit.authorship_log.attestations.len(),
        1,
        "Should have 1 attestation for the emoji-named file"
    );
    assert_eq!(
        commit.authorship_log.attestations[0].file_path,
        "🚀rocket_launch.txt",
        "File path should be the actual UTF-8 filename with emoji"
    );

    // Get stats and verify AI attribution is correct
    let raw = repo.git_ai(&["stats", "--json"]).unwrap();
    let json = extract_json_object(&raw);
    let stats: CommitStats = serde_json::from_str(&json).unwrap();

    // The key check: ai_additions should NOT be 0
    assert_eq!(
        stats.ai_additions, 4,
        "All 4 lines should be attributed to AI, not human"
    );
    assert_eq!(
        stats.human_additions, 0,
        "No lines should be attributed to human"
    );
    assert_eq!(
        stats.ai_accepted, 4,
        "All 4 AI lines should be counted as accepted"
    );
    assert_eq!(
        stats.git_diff_added_lines, 4,
        "Git should report 4 added lines"
    );
}

#[test]
fn test_mixed_ascii_and_utf8_filenames() {
    let repo = TestRepo::new();

    // Create an initial commit
    let mut readme = repo.filename("README.md");
    readme.set_contents(lines!["# Project"]);
    repo.stage_all_and_commit("Initial commit").unwrap();

    // AI creates multiple files - one with ASCII name, one with Chinese, one with emoji
    let mut ascii_file = repo.filename("normal_file.txt");
    ascii_file.set_contents(lines![
        "Normal line 1".ai(),
        "Normal line 2".ai(),
    ]);

    let mut chinese_file = repo.filename("配置文件.txt");
    chinese_file.set_contents(lines![
        "设置一".ai(),
        "设置二".ai(),
        "设置三".ai(),
    ]);

    let mut emoji_file = repo.filename("🎉celebration.txt");
    emoji_file.set_contents(lines![
        "Party time!".ai(),
    ]);

    // Commit all files together
    let commit = repo.stage_all_and_commit("Add mixed files").unwrap();

    // Verify the authorship log contains all 3 files
    assert_eq!(
        commit.authorship_log.attestations.len(),
        3,
        "Should have 3 attestations for all files"
    );

    // Verify each file path is correctly stored
    let file_paths: Vec<&str> = commit
        .authorship_log
        .attestations
        .iter()
        .map(|a| a.file_path.as_str())
        .collect();
    assert!(
        file_paths.contains(&"normal_file.txt"),
        "Should contain ASCII filename"
    );
    assert!(
        file_paths.contains(&"配置文件.txt"),
        "Should contain Chinese filename"
    );
    assert!(
        file_paths.contains(&"🎉celebration.txt"),
        "Should contain emoji filename"
    );

    // Get stats and verify AI attribution is correct for all files
    let raw = repo.git_ai(&["stats", "--json"]).unwrap();
    let json = extract_json_object(&raw);
    let stats: CommitStats = serde_json::from_str(&json).unwrap();

    // Total: 2 + 3 + 1 = 6 AI lines
    assert_eq!(
        stats.ai_additions, 6,
        "All 6 lines should be attributed to AI"
    );
    assert_eq!(
        stats.human_additions, 0,
        "No lines should be attributed to human"
    );
    assert_eq!(
        stats.ai_accepted, 6,
        "All 6 AI lines should be counted as accepted"
    );
    assert_eq!(
        stats.git_diff_added_lines, 6,
        "Git should report 6 added lines"
    );
}

#[test]
fn test_utf8_content_in_file() {
    let repo = TestRepo::new();

    // Create an initial commit
    let mut readme = repo.filename("README.md");
    readme.set_contents(lines!["# Project"]);
    repo.stage_all_and_commit("Initial commit").unwrap();

    // AI creates a file with UTF-8 content (but ASCII filename)
    let mut content_file = repo.filename("content.txt");
    content_file.set_contents(lines![
        "Hello World".ai(),
        "你好世界".ai(),
        "🌍 地球".ai(),
        "مرحبا بالعالم".ai(),
        "Привет мир".ai(),
    ]);

    // Commit the file
    let commit = repo.stage_all_and_commit("Add UTF-8 content").unwrap();

    // Verify the authorship log
    assert_eq!(commit.authorship_log.attestations.len(), 1);

    // Get stats and verify AI attribution is correct
    let raw = repo.git_ai(&["stats", "--json"]).unwrap();
    let json = extract_json_object(&raw);
    let stats: CommitStats = serde_json::from_str(&json).unwrap();

    assert_eq!(
        stats.ai_additions, 5,
        "All 5 lines should be attributed to AI"
    );
    assert_eq!(
        stats.human_additions, 0,
        "No lines should be attributed to human"
    );
    assert_eq!(
        stats.ai_accepted, 5,
        "All 5 AI lines should be counted as accepted"
    );
}

#[test]
fn test_utf8_filename_blame() {
    let repo = TestRepo::new();

    // Create an initial commit
    let mut readme = repo.filename("README.md");
    readme.set_contents(lines!["# Project"]);
    repo.stage_all_and_commit("Initial commit").unwrap();

    // AI creates a file with Chinese characters in the filename
    let mut chinese_file = repo.filename("测试文件.rs");
    chinese_file.set_contents(lines![
        "fn main() {".ai(),
        "    println!(\"Hello\");".ai(),
        "}".ai(),
    ]);

    // Commit the Chinese-named file
    repo.stage_all_and_commit("Add test file").unwrap();

    // Verify blame works correctly with the UTF-8 filename
    chinese_file.assert_lines_and_blame(lines![
        "fn main() {".ai(),
        "    println!(\"Hello\");".ai(),
        "}".ai(),
    ]);
}

#[test]
fn test_nested_directory_with_utf8_filename() {
    let repo = TestRepo::new();

    // Create an initial commit
    let mut readme = repo.filename("README.md");
    readme.set_contents(lines!["# Project"]);
    repo.stage_all_and_commit("Initial commit").unwrap();

    // AI creates a file in a nested directory with UTF-8 name
    let mut nested_file = repo.filename("src/模块/组件.ts");
    nested_file.set_contents(lines![
        "export const 组件 = () => {};".ai(),
        "export default 组件;".ai(),
    ]);

    // Commit the file
    let commit = repo.stage_all_and_commit("Add nested UTF-8 file").unwrap();

    // Verify the authorship log contains the correct path
    assert_eq!(commit.authorship_log.attestations.len(), 1);
    assert_eq!(
        commit.authorship_log.attestations[0].file_path,
        "src/模块/组件.ts",
        "File path should preserve UTF-8 in both directory and file names"
    );

    // Get stats and verify AI attribution
    let raw = repo.git_ai(&["stats", "--json"]).unwrap();
    let json = extract_json_object(&raw);
    let stats: CommitStats = serde_json::from_str(&json).unwrap();

    assert_eq!(
        stats.ai_additions, 2,
        "Both lines should be attributed to AI"
    );
    assert_eq!(
        stats.human_additions, 0,
        "No lines should be attributed to human"
    );
}

#[test]
fn test_utf8_filename_with_human_and_ai_lines() {
    let repo = TestRepo::new();

    // Create an initial commit
    let mut readme = repo.filename("README.md");
    readme.set_contents(lines!["# Project"]);
    repo.stage_all_and_commit("Initial commit").unwrap();

    // Create a file with mixed human and AI contributions
    let mut mixed_file = repo.filename("数据.json");
    mixed_file.set_contents(lines![
        "{".human(),
        "  \"name\": \"测试\",".ai(),
        "  \"value\": 123,".ai(),
        "  \"enabled\": true".human(),
        "}".human(),
    ]);

    // Commit the file
    repo.stage_all_and_commit("Add data file").unwrap();

    // Get stats and verify attribution
    let raw = repo.git_ai(&["stats", "--json"]).unwrap();
    let json = extract_json_object(&raw);
    let stats: CommitStats = serde_json::from_str(&json).unwrap();

    assert_eq!(
        stats.ai_additions, 2,
        "2 lines should be attributed to AI"
    );
    assert_eq!(
        stats.ai_accepted, 2,
        "2 AI lines should be counted as accepted"
    );
    assert_eq!(
        stats.human_additions, 3,
        "3 lines should be attributed to human"
    );
    assert_eq!(
        stats.git_diff_added_lines, 5,
        "Git should report 5 total added lines"
    );
}

// =============================================================================
// Phase 1: CJK Extended Coverage (Japanese, Korean, Traditional Chinese)
// =============================================================================

#[test]
fn test_japanese_hiragana_katakana_filename() {
    let repo = TestRepo::new();

    // Create an initial commit
    let mut readme = repo.filename("README.md");
    readme.set_contents(lines!["# Project"]);
    repo.stage_all_and_commit("Initial commit").unwrap();

    // AI creates a file with Japanese Hiragana and Katakana in the filename
    let mut japanese_file = repo.filename("ひらがな_カタカナ.txt");
    japanese_file.set_contents(lines![
        "こんにちは".ai(),
        "コンニチハ".ai(),
        "Hello in Japanese".ai(),
    ]);

    // Commit the Japanese-named file
    let commit = repo.stage_all_and_commit("Add Japanese hiragana/katakana file").unwrap();

    // Verify the authorship log contains the Japanese filename
    assert_eq!(
        commit.authorship_log.attestations.len(),
        1,
        "Should have 1 attestation for the Japanese-named file"
    );
    assert_eq!(
        commit.authorship_log.attestations[0].file_path,
        "ひらがな_カタカナ.txt",
        "File path should be the actual UTF-8 filename with Hiragana and Katakana"
    );

    // Get stats and verify AI attribution is correct
    let raw = repo.git_ai(&["stats", "--json"]).unwrap();
    let json = extract_json_object(&raw);
    let stats: CommitStats = serde_json::from_str(&json).unwrap();

    assert_eq!(
        stats.ai_additions, 3,
        "All 3 lines should be attributed to AI"
    );
    assert_eq!(
        stats.human_additions, 0,
        "No lines should be attributed to human"
    );
}

#[test]
fn test_japanese_kanji_filename() {
    let repo = TestRepo::new();

    // Create an initial commit
    let mut readme = repo.filename("README.md");
    readme.set_contents(lines!["# Project"]);
    repo.stage_all_and_commit("Initial commit").unwrap();

    // AI creates a file with Japanese Kanji in the filename
    let mut kanji_file = repo.filename("漢字ファイル.rs");
    kanji_file.set_contents(lines![
        "fn main() {".ai(),
        "    println!(\"日本語\");".ai(),
        "}".ai(),
    ]);

    // Commit the Kanji-named file
    let commit = repo.stage_all_and_commit("Add Japanese kanji file").unwrap();

    assert_eq!(
        commit.authorship_log.attestations[0].file_path,
        "漢字ファイル.rs",
        "File path should preserve Japanese Kanji characters"
    );

    let raw = repo.git_ai(&["stats", "--json"]).unwrap();
    let json = extract_json_object(&raw);
    let stats: CommitStats = serde_json::from_str(&json).unwrap();

    assert_eq!(stats.ai_additions, 3, "All 3 lines should be attributed to AI");
    assert_eq!(stats.human_additions, 0, "No lines should be attributed to human");
}

// =============================================================================
// Phase 5: Cyrillic and Greek Scripts
// =============================================================================

#[test]
fn test_russian_cyrillic_filename() {
    let repo = TestRepo::new();

    // Create an initial commit
    let mut readme = repo.filename("README.md");
    readme.set_contents(lines!["# Project"]);
    repo.stage_all_and_commit("Initial commit").unwrap();

    // AI creates a file with Russian Cyrillic characters in the filename
    let mut russian_file = repo.filename("Русский.txt");
    russian_file.set_contents(lines![
        "Привет мир".ai(),
        "Спасибо".ai(),
        "Россия".ai(),
    ]);

    // Commit the Russian-named file
    let commit = repo.stage_all_and_commit("Add Russian file").unwrap();

    assert_eq!(
        commit.authorship_log.attestations[0].file_path,
        "Русский.txt",
        "File path should preserve Russian Cyrillic characters"
    );

    let raw = repo.git_ai(&["stats", "--json"]).unwrap();
    let json = extract_json_object(&raw);
    let stats: CommitStats = serde_json::from_str(&json).unwrap();

    assert_eq!(stats.ai_additions, 3, "All 3 lines should be attributed to AI");
    assert_eq!(stats.human_additions, 0, "No lines should be attributed to human");
}

#[test]
fn test_ukrainian_cyrillic_filename() {
    let repo = TestRepo::new();

    // Create an initial commit
    let mut readme = repo.filename("README.md");
    readme.set_contents(lines!["# Project"]);
    repo.stage_all_and_commit("Initial commit").unwrap();

    // AI creates a file with Ukrainian Cyrillic characters in the filename
    // Ukrainian has unique letters like ї, і, є, ґ
    let mut ukrainian_file = repo.filename("Українська.txt");
    ukrainian_file.set_contents(lines![
        "Привіт".ai(),
        "Дякую".ai(),
        "Україна".ai(),
    ]);

    // Commit the Ukrainian-named file
    let commit = repo.stage_all_and_commit("Add Ukrainian file").unwrap();

    assert_eq!(
        commit.authorship_log.attestations[0].file_path,
        "Українська.txt",
        "File path should preserve Ukrainian Cyrillic characters"
    );

    let raw = repo.git_ai(&["stats", "--json"]).unwrap();
    let json = extract_json_object(&raw);
    let stats: CommitStats = serde_json::from_str(&json).unwrap();

    assert_eq!(stats.ai_additions, 3, "All 3 lines should be attributed to AI");
    assert_eq!(stats.human_additions, 0, "No lines should be attributed to human");
}

#[test]
fn test_greek_filename() {
    let repo = TestRepo::new();

    // Create an initial commit
    let mut readme = repo.filename("README.md");
    readme.set_contents(lines!["# Project"]);
    repo.stage_all_and_commit("Initial commit").unwrap();

    // AI creates a file with Greek characters in the filename
    let mut greek_file = repo.filename("Ελληνικά.txt");
    greek_file.set_contents(lines![
        "Γειά σου".ai(),
        "Ευχαριστώ".ai(),
        "Ελλάδα".ai(),
    ]);

    // Commit the Greek-named file
    let commit = repo.stage_all_and_commit("Add Greek file").unwrap();

    assert_eq!(
        commit.authorship_log.attestations[0].file_path,
        "Ελληνικά.txt",
        "File path should preserve Greek characters"
    );

    let raw = repo.git_ai(&["stats", "--json"]).unwrap();
    let json = extract_json_object(&raw);
    let stats: CommitStats = serde_json::from_str(&json).unwrap();

    assert_eq!(stats.ai_additions, 3, "All 3 lines should be attributed to AI");
    assert_eq!(stats.human_additions, 0, "No lines should be attributed to human");
}

#[test]
fn test_greek_polytonic_filename() {
    let repo = TestRepo::new();

    // Create an initial commit
    let mut readme = repo.filename("README.md");
    readme.set_contents(lines!["# Project"]);
    repo.stage_all_and_commit("Initial commit").unwrap();

    // AI creates a file with Greek polytonic (with diacritics) characters in the filename
    let mut polytonic_file = repo.filename("Ἑλληνική.txt");
    polytonic_file.set_contents(lines![
        "Ἀθῆναι".ai(),
        "φιλοσοφία".ai(),
    ]);

    // Commit the Greek polytonic-named file
    let commit = repo.stage_all_and_commit("Add Greek polytonic file").unwrap();

    assert_eq!(
        commit.authorship_log.attestations[0].file_path,
        "Ἑλληνική.txt",
        "File path should preserve Greek polytonic characters with diacritics"
    );

    let raw = repo.git_ai(&["stats", "--json"]).unwrap();
    let json = extract_json_object(&raw);
    let stats: CommitStats = serde_json::from_str(&json).unwrap();

    assert_eq!(stats.ai_additions, 2, "Both lines should be attributed to AI");
    assert_eq!(stats.human_additions, 0, "No lines should be attributed to human");
}

// =============================================================================
// Phase 4: Southeast Asian Scripts (Thai, Vietnamese, Khmer, Lao)
// =============================================================================

#[test]
fn test_thai_filename() {
    let repo = TestRepo::new();

    // Create an initial commit
    let mut readme = repo.filename("README.md");
    readme.set_contents(lines!["# Project"]);
    repo.stage_all_and_commit("Initial commit").unwrap();

    // AI creates a file with Thai characters in the filename
    let mut thai_file = repo.filename("ภาษาไทย.txt");
    thai_file.set_contents(lines![
        "สวัสดี".ai(),
        "ขอบคุณ".ai(),
        "ประเทศไทย".ai(),
    ]);

    // Commit the Thai-named file
    let commit = repo.stage_all_and_commit("Add Thai file").unwrap();

    assert_eq!(
        commit.authorship_log.attestations[0].file_path,
        "ภาษาไทย.txt",
        "File path should preserve Thai characters"
    );

    let raw = repo.git_ai(&["stats", "--json"]).unwrap();
    let json = extract_json_object(&raw);
    let stats: CommitStats = serde_json::from_str(&json).unwrap();

    assert_eq!(stats.ai_additions, 3, "All 3 lines should be attributed to AI");
    assert_eq!(stats.human_additions, 0, "No lines should be attributed to human");
}

#[test]
fn test_vietnamese_filename() {
    let repo = TestRepo::new();

    // Create an initial commit
    let mut readme = repo.filename("README.md");
    readme.set_contents(lines!["# Project"]);
    repo.stage_all_and_commit("Initial commit").unwrap();

    // AI creates a file with Vietnamese characters (with tone marks) in the filename
    let mut vietnamese_file = repo.filename("tiếng_việt.txt");
    vietnamese_file.set_contents(lines![
        "Xin chào".ai(),
        "Cảm ơn".ai(),
        "Việt Nam".ai(),
    ]);

    // Commit the Vietnamese-named file
    let commit = repo.stage_all_and_commit("Add Vietnamese file").unwrap();

    assert_eq!(
        commit.authorship_log.attestations[0].file_path,
        "tiếng_việt.txt",
        "File path should preserve Vietnamese tone marks"
    );

    let raw = repo.git_ai(&["stats", "--json"]).unwrap();
    let json = extract_json_object(&raw);
    let stats: CommitStats = serde_json::from_str(&json).unwrap();

    assert_eq!(stats.ai_additions, 3, "All 3 lines should be attributed to AI");
    assert_eq!(stats.human_additions, 0, "No lines should be attributed to human");
}

#[test]
fn test_khmer_filename() {
    let repo = TestRepo::new();

    // Create an initial commit
    let mut readme = repo.filename("README.md");
    readme.set_contents(lines!["# Project"]);
    repo.stage_all_and_commit("Initial commit").unwrap();

    // AI creates a file with Khmer (Cambodian) characters in the filename
    let mut khmer_file = repo.filename("ភាសាខ្មែរ.txt");
    khmer_file.set_contents(lines![
        "សួស្តី".ai(),
        "អរគុណ".ai(),
    ]);

    // Commit the Khmer-named file
    let commit = repo.stage_all_and_commit("Add Khmer file").unwrap();

    assert_eq!(
        commit.authorship_log.attestations[0].file_path,
        "ភាសាខ្មែរ.txt",
        "File path should preserve Khmer characters"
    );

    let raw = repo.git_ai(&["stats", "--json"]).unwrap();
    let json = extract_json_object(&raw);
    let stats: CommitStats = serde_json::from_str(&json).unwrap();

    assert_eq!(stats.ai_additions, 2, "Both lines should be attributed to AI");
    assert_eq!(stats.human_additions, 0, "No lines should be attributed to human");
}

#[test]
fn test_lao_filename() {
    let repo = TestRepo::new();

    // Create an initial commit
    let mut readme = repo.filename("README.md");
    readme.set_contents(lines!["# Project"]);
    repo.stage_all_and_commit("Initial commit").unwrap();

    // AI creates a file with Lao characters in the filename
    let mut lao_file = repo.filename("ພາສາລາວ.txt");
    lao_file.set_contents(lines![
        "ສະບາຍດີ".ai(),
        "ຂອບໃຈ".ai(),
    ]);

    // Commit the Lao-named file
    let commit = repo.stage_all_and_commit("Add Lao file").unwrap();

    assert_eq!(
        commit.authorship_log.attestations[0].file_path,
        "ພາສາລາວ.txt",
        "File path should preserve Lao characters"
    );

    let raw = repo.git_ai(&["stats", "--json"]).unwrap();
    let json = extract_json_object(&raw);
    let stats: CommitStats = serde_json::from_str(&json).unwrap();

    assert_eq!(stats.ai_additions, 2, "Both lines should be attributed to AI");
    assert_eq!(stats.human_additions, 0, "No lines should be attributed to human");
}

// =============================================================================
// Phase 3: Indic Scripts (Hindi, Tamil, Bengali, Telugu, Gujarati)
// =============================================================================

#[test]
fn test_hindi_devanagari_filename() {
    let repo = TestRepo::new();

    // Create an initial commit
    let mut readme = repo.filename("README.md");
    readme.set_contents(lines!["# Project"]);
    repo.stage_all_and_commit("Initial commit").unwrap();

    // AI creates a file with Hindi/Devanagari characters in the filename
    let mut hindi_file = repo.filename("हिंदी.txt");
    hindi_file.set_contents(lines![
        "नमस्ते".ai(),
        "धन्यवाद".ai(),
        "भारत".ai(),
    ]);

    // Commit the Hindi-named file
    let commit = repo.stage_all_and_commit("Add Hindi file").unwrap();

    assert_eq!(
        commit.authorship_log.attestations[0].file_path,
        "हिंदी.txt",
        "File path should preserve Hindi/Devanagari characters"
    );

    let raw = repo.git_ai(&["stats", "--json"]).unwrap();
    let json = extract_json_object(&raw);
    let stats: CommitStats = serde_json::from_str(&json).unwrap();

    assert_eq!(stats.ai_additions, 3, "All 3 lines should be attributed to AI");
    assert_eq!(stats.human_additions, 0, "No lines should be attributed to human");
}

#[test]
fn test_tamil_filename() {
    let repo = TestRepo::new();

    // Create an initial commit
    let mut readme = repo.filename("README.md");
    readme.set_contents(lines!["# Project"]);
    repo.stage_all_and_commit("Initial commit").unwrap();

    // AI creates a file with Tamil characters in the filename
    let mut tamil_file = repo.filename("தமிழ்.txt");
    tamil_file.set_contents(lines![
        "வணக்கம்".ai(),
        "நன்றி".ai(),
    ]);

    // Commit the Tamil-named file
    let commit = repo.stage_all_and_commit("Add Tamil file").unwrap();

    assert_eq!(
        commit.authorship_log.attestations[0].file_path,
        "தமிழ்.txt",
        "File path should preserve Tamil characters"
    );

    let raw = repo.git_ai(&["stats", "--json"]).unwrap();
    let json = extract_json_object(&raw);
    let stats: CommitStats = serde_json::from_str(&json).unwrap();

    assert_eq!(stats.ai_additions, 2, "Both lines should be attributed to AI");
    assert_eq!(stats.human_additions, 0, "No lines should be attributed to human");
}

#[test]
fn test_bengali_filename() {
    let repo = TestRepo::new();

    // Create an initial commit
    let mut readme = repo.filename("README.md");
    readme.set_contents(lines!["# Project"]);
    repo.stage_all_and_commit("Initial commit").unwrap();

    // AI creates a file with Bengali characters in the filename
    let mut bengali_file = repo.filename("বাংলা.txt");
    bengali_file.set_contents(lines![
        "নমস্কার".ai(),
        "ধন্যবাদ".ai(),
    ]);

    // Commit the Bengali-named file
    let commit = repo.stage_all_and_commit("Add Bengali file").unwrap();

    assert_eq!(
        commit.authorship_log.attestations[0].file_path,
        "বাংলা.txt",
        "File path should preserve Bengali characters"
    );

    let raw = repo.git_ai(&["stats", "--json"]).unwrap();
    let json = extract_json_object(&raw);
    let stats: CommitStats = serde_json::from_str(&json).unwrap();

    assert_eq!(stats.ai_additions, 2, "Both lines should be attributed to AI");
    assert_eq!(stats.human_additions, 0, "No lines should be attributed to human");
}

#[test]
fn test_telugu_filename() {
    let repo = TestRepo::new();

    // Create an initial commit
    let mut readme = repo.filename("README.md");
    readme.set_contents(lines!["# Project"]);
    repo.stage_all_and_commit("Initial commit").unwrap();

    // AI creates a file with Telugu characters in the filename
    let mut telugu_file = repo.filename("తెలుగు.txt");
    telugu_file.set_contents(lines![
        "నమస్కారం".ai(),
        "ధన్యవాదాలు".ai(),
    ]);

    // Commit the Telugu-named file
    let commit = repo.stage_all_and_commit("Add Telugu file").unwrap();

    assert_eq!(
        commit.authorship_log.attestations[0].file_path,
        "తెలుగు.txt",
        "File path should preserve Telugu characters"
    );

    let raw = repo.git_ai(&["stats", "--json"]).unwrap();
    let json = extract_json_object(&raw);
    let stats: CommitStats = serde_json::from_str(&json).unwrap();

    assert_eq!(stats.ai_additions, 2, "Both lines should be attributed to AI");
    assert_eq!(stats.human_additions, 0, "No lines should be attributed to human");
}

#[test]
fn test_gujarati_filename() {
    let repo = TestRepo::new();

    // Create an initial commit
    let mut readme = repo.filename("README.md");
    readme.set_contents(lines!["# Project"]);
    repo.stage_all_and_commit("Initial commit").unwrap();

    // AI creates a file with Gujarati characters in the filename
    let mut gujarati_file = repo.filename("ગુજરાતી.txt");
    gujarati_file.set_contents(lines![
        "નમસ્તે".ai(),
        "આભાર".ai(),
    ]);

    // Commit the Gujarati-named file
    let commit = repo.stage_all_and_commit("Add Gujarati file").unwrap();

    assert_eq!(
        commit.authorship_log.attestations[0].file_path,
        "ગુજરાતી.txt",
        "File path should preserve Gujarati characters"
    );

    let raw = repo.git_ai(&["stats", "--json"]).unwrap();
    let json = extract_json_object(&raw);
    let stats: CommitStats = serde_json::from_str(&json).unwrap();

    assert_eq!(stats.ai_additions, 2, "Both lines should be attributed to AI");
    assert_eq!(stats.human_additions, 0, "No lines should be attributed to human");
}

#[test]
fn test_devanagari_combining_chars() {
    let repo = TestRepo::new();

    // Create an initial commit
    let mut readme = repo.filename("README.md");
    readme.set_contents(lines!["# Project"]);
    repo.stage_all_and_commit("Initial commit").unwrap();

    // AI creates a file with Devanagari combining vowel marks
    // The word "किताब" (kitaab = book) uses combining vowels
    let mut combining_file = repo.filename("किताब.txt");
    combining_file.set_contents(lines![
        "पुस्तक".ai(),
        "अध्याय".ai(),
    ]);

    // Commit the file with combining characters
    let commit = repo.stage_all_and_commit("Add file with combining chars").unwrap();

    assert_eq!(
        commit.authorship_log.attestations[0].file_path,
        "किताब.txt",
        "File path should preserve Devanagari combining characters"
    );

    let raw = repo.git_ai(&["stats", "--json"]).unwrap();
    let json = extract_json_object(&raw);
    let stats: CommitStats = serde_json::from_str(&json).unwrap();

    assert_eq!(stats.ai_additions, 2, "Both lines should be attributed to AI");
    assert_eq!(stats.human_additions, 0, "No lines should be attributed to human");
}

#[test]
fn test_korean_hangul_filename() {
    let repo = TestRepo::new();

    // Create an initial commit
    let mut readme = repo.filename("README.md");
    readme.set_contents(lines!["# Project"]);
    repo.stage_all_and_commit("Initial commit").unwrap();

    // AI creates a file with Korean Hangul in the filename
    let mut korean_file = repo.filename("한글파일.txt");
    korean_file.set_contents(lines![
        "안녕하세요".ai(),
        "감사합니다".ai(),
    ]);

    // Commit the Korean-named file
    let commit = repo.stage_all_and_commit("Add Korean hangul file").unwrap();

    assert_eq!(
        commit.authorship_log.attestations[0].file_path,
        "한글파일.txt",
        "File path should preserve Korean Hangul characters"
    );

    let raw = repo.git_ai(&["stats", "--json"]).unwrap();
    let json = extract_json_object(&raw);
    let stats: CommitStats = serde_json::from_str(&json).unwrap();

    assert_eq!(stats.ai_additions, 2, "Both lines should be attributed to AI");
    assert_eq!(stats.human_additions, 0, "No lines should be attributed to human");
}

#[test]
fn test_chinese_traditional_filename() {
    let repo = TestRepo::new();

    // Create an initial commit
    let mut readme = repo.filename("README.md");
    readme.set_contents(lines!["# Project"]);
    repo.stage_all_and_commit("Initial commit").unwrap();

    // AI creates a file with Traditional Chinese in the filename
    let mut traditional_file = repo.filename("繁體中文.txt");
    traditional_file.set_contents(lines![
        "傳統字體".ai(),
        "正體中文".ai(),
        "臺灣".ai(),
    ]);

    // Commit the Traditional Chinese-named file
    let commit = repo.stage_all_and_commit("Add Traditional Chinese file").unwrap();

    assert_eq!(
        commit.authorship_log.attestations[0].file_path,
        "繁體中文.txt",
        "File path should preserve Traditional Chinese characters"
    );

    let raw = repo.git_ai(&["stats", "--json"]).unwrap();
    let json = extract_json_object(&raw);
    let stats: CommitStats = serde_json::from_str(&json).unwrap();

    assert_eq!(stats.ai_additions, 3, "All 3 lines should be attributed to AI");
    assert_eq!(stats.human_additions, 0, "No lines should be attributed to human");
}

#[test]
fn test_mixed_cjk_filename() {
    let repo = TestRepo::new();

    // Create an initial commit
    let mut readme = repo.filename("README.md");
    readme.set_contents(lines!["# Project"]);
    repo.stage_all_and_commit("Initial commit").unwrap();

    // AI creates a file with mixed CJK (Chinese, Japanese, Korean) in the filename
    let mut mixed_cjk_file = repo.filename("日本語_中文_한글.txt");
    mixed_cjk_file.set_contents(lines![
        "Japanese: 日本".ai(),
        "Chinese: 中国".ai(),
        "Korean: 한국".ai(),
        "Mixed CJK content".ai(),
    ]);

    // Commit the mixed CJK-named file
    let commit = repo.stage_all_and_commit("Add mixed CJK file").unwrap();

    assert_eq!(
        commit.authorship_log.attestations[0].file_path,
        "日本語_中文_한글.txt",
        "File path should preserve mixed CJK characters"
    );

    let raw = repo.git_ai(&["stats", "--json"]).unwrap();
    let json = extract_json_object(&raw);
    let stats: CommitStats = serde_json::from_str(&json).unwrap();

    assert_eq!(stats.ai_additions, 4, "All 4 lines should be attributed to AI");
    assert_eq!(stats.human_additions, 0, "No lines should be attributed to human");
}

// =============================================================================
// Phase 2: RTL Scripts (Arabic, Hebrew, Persian, Urdu)
// =============================================================================

#[test]
fn test_arabic_filename() {
    let repo = TestRepo::new();

    // Create an initial commit
    let mut readme = repo.filename("README.md");
    readme.set_contents(lines!["# Project"]);
    repo.stage_all_and_commit("Initial commit").unwrap();

    // AI creates a file with Arabic characters in the filename
    let mut arabic_file = repo.filename("مرحبا.txt");
    arabic_file.set_contents(lines![
        "السلام عليكم".ai(),
        "مرحبا بالعالم".ai(),
        "شكراً".ai(),
    ]);

    // Commit the Arabic-named file
    let commit = repo.stage_all_and_commit("Add Arabic file").unwrap();

    assert_eq!(
        commit.authorship_log.attestations.len(),
        1,
        "Should have 1 attestation for the Arabic-named file"
    );
    assert_eq!(
        commit.authorship_log.attestations[0].file_path,
        "مرحبا.txt",
        "File path should preserve Arabic characters"
    );

    let raw = repo.git_ai(&["stats", "--json"]).unwrap();
    let json = extract_json_object(&raw);
    let stats: CommitStats = serde_json::from_str(&json).unwrap();

    assert_eq!(stats.ai_additions, 3, "All 3 lines should be attributed to AI");
    assert_eq!(stats.human_additions, 0, "No lines should be attributed to human");
}

#[test]
fn test_hebrew_filename() {
    let repo = TestRepo::new();

    // Create an initial commit
    let mut readme = repo.filename("README.md");
    readme.set_contents(lines!["# Project"]);
    repo.stage_all_and_commit("Initial commit").unwrap();

    // AI creates a file with Hebrew characters in the filename
    let mut hebrew_file = repo.filename("שלום.txt");
    hebrew_file.set_contents(lines![
        "שלום עולם".ai(),
        "תודה רבה".ai(),
    ]);

    // Commit the Hebrew-named file
    let commit = repo.stage_all_and_commit("Add Hebrew file").unwrap();

    assert_eq!(
        commit.authorship_log.attestations[0].file_path,
        "שלום.txt",
        "File path should preserve Hebrew characters"
    );

    let raw = repo.git_ai(&["stats", "--json"]).unwrap();
    let json = extract_json_object(&raw);
    let stats: CommitStats = serde_json::from_str(&json).unwrap();

    assert_eq!(stats.ai_additions, 2, "Both lines should be attributed to AI");
    assert_eq!(stats.human_additions, 0, "No lines should be attributed to human");
}

#[test]
fn test_persian_filename() {
    let repo = TestRepo::new();

    // Create an initial commit
    let mut readme = repo.filename("README.md");
    readme.set_contents(lines!["# Project"]);
    repo.stage_all_and_commit("Initial commit").unwrap();

    // AI creates a file with Persian/Farsi characters in the filename
    let mut persian_file = repo.filename("فارسی.txt");
    persian_file.set_contents(lines![
        "سلام".ai(),
        "خوش آمدید".ai(),
        "ممنون".ai(),
    ]);

    // Commit the Persian-named file
    let commit = repo.stage_all_and_commit("Add Persian file").unwrap();

    assert_eq!(
        commit.authorship_log.attestations[0].file_path,
        "فارسی.txt",
        "File path should preserve Persian characters"
    );

    let raw = repo.git_ai(&["stats", "--json"]).unwrap();
    let json = extract_json_object(&raw);
    let stats: CommitStats = serde_json::from_str(&json).unwrap();

    assert_eq!(stats.ai_additions, 3, "All 3 lines should be attributed to AI");
    assert_eq!(stats.human_additions, 0, "No lines should be attributed to human");
}

#[test]
fn test_urdu_filename() {
    let repo = TestRepo::new();

    // Create an initial commit
    let mut readme = repo.filename("README.md");
    readme.set_contents(lines!["# Project"]);
    repo.stage_all_and_commit("Initial commit").unwrap();

    // AI creates a file with Urdu characters in the filename
    let mut urdu_file = repo.filename("اردو.txt");
    urdu_file.set_contents(lines![
        "السلام علیکم".ai(),
        "شکریہ".ai(),
    ]);

    // Commit the Urdu-named file
    let commit = repo.stage_all_and_commit("Add Urdu file").unwrap();

    assert_eq!(
        commit.authorship_log.attestations[0].file_path,
        "اردو.txt",
        "File path should preserve Urdu characters"
    );

    let raw = repo.git_ai(&["stats", "--json"]).unwrap();
    let json = extract_json_object(&raw);
    let stats: CommitStats = serde_json::from_str(&json).unwrap();

    assert_eq!(stats.ai_additions, 2, "Both lines should be attributed to AI");
    assert_eq!(stats.human_additions, 0, "No lines should be attributed to human");
}

#[test]
fn test_rtl_with_ltr_mixed_filename() {
    let repo = TestRepo::new();

    // Create an initial commit
    let mut readme = repo.filename("README.md");
    readme.set_contents(lines!["# Project"]);
    repo.stage_all_and_commit("Initial commit").unwrap();

    // AI creates a file with mixed RTL (Arabic) and LTR (English) in the filename
    let mut mixed_file = repo.filename("test_مرحبا_file.txt");
    mixed_file.set_contents(lines![
        "Mixed RTL and LTR content".ai(),
        "محتوى مختلط".ai(),
    ]);

    // Commit the mixed RTL/LTR-named file
    let commit = repo.stage_all_and_commit("Add mixed RTL/LTR file").unwrap();

    assert_eq!(
        commit.authorship_log.attestations[0].file_path,
        "test_مرحبا_file.txt",
        "File path should preserve mixed RTL/LTR characters"
    );

    let raw = repo.git_ai(&["stats", "--json"]).unwrap();
    let json = extract_json_object(&raw);
    let stats: CommitStats = serde_json::from_str(&json).unwrap();

    assert_eq!(stats.ai_additions, 2, "Both lines should be attributed to AI");
    assert_eq!(stats.human_additions, 0, "No lines should be attributed to human");
}

#[test]
fn test_rtl_directory_path() {
    let repo = TestRepo::new();

    // Create an initial commit
    let mut readme = repo.filename("README.md");
    readme.set_contents(lines!["# Project"]);
    repo.stage_all_and_commit("Initial commit").unwrap();

    // AI creates a file in a directory with Arabic name
    let mut nested_file = repo.filename("src/العربية/ملف.rs");
    nested_file.set_contents(lines![
        "fn main() {".ai(),
        "    println!(\"مرحبا\");".ai(),
        "}".ai(),
    ]);

    // Commit the file in RTL-named directory
    let commit = repo.stage_all_and_commit("Add file in Arabic directory").unwrap();

    assert_eq!(
        commit.authorship_log.attestations[0].file_path,
        "src/العربية/ملف.rs",
        "File path should preserve Arabic characters in both directory and file names"
    );

    let raw = repo.git_ai(&["stats", "--json"]).unwrap();
    let json = extract_json_object(&raw);
    let stats: CommitStats = serde_json::from_str(&json).unwrap();

    assert_eq!(stats.ai_additions, 3, "All 3 lines should be attributed to AI");
    assert_eq!(stats.human_additions, 0, "No lines should be attributed to human");
}
