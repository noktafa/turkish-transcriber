//! Turkish-specific post-processing for Whisper transcription output.
//!
//! Runs on collected segments before writing output. Fixes common
//! Whisper errors for Turkish: missing question marks, garbled words,
//! wrong special characters, and mangled proper nouns.

/// Apply all Turkish post-processing passes to a segment's text.
pub fn process(text: &str) -> String {
    let text = fix_substitutions(text);
    let text = fix_proper_nouns(&text);
    let text = fix_turkish_chars(&text);
    fix_question_marks(&text)
}

// ── Question Particle Detection ─────────────────────────────────────

/// Turkish question particles (all vowel-harmony variants).
static QUESTION_PARTICLES: &[&str] = &[
    // Extended forms first (longest match)
    "mısınız", "misiniz", "musunuz", "müsünüz",
    "mıyız", "miyiz", "muyuz", "müyüz",
    "mısın", "misin", "musun", "müsün",
    "mıdır", "midir", "mudur", "müdür",
    // Base forms last
    "mı", "mi", "mu", "mü",
];

/// If the segment ends with a Turkish question particle, ensure it ends with `?`.
fn fix_question_marks(text: &str) -> String {
    let trimmed = text.trim_end();

    // Already has a question mark
    if trimmed.ends_with('?') {
        return text.to_string();
    }

    // Strip trailing punctuation (.!,;:) to check the bare word
    let stripped = trimmed.trim_end_matches(|c: char| matches!(c, '.' | '!' | ',' | ';' | ':'));

    let lower = stripped.to_lowercase();
    for particle in QUESTION_PARTICLES {
        // The particle must be a standalone word at the end, preceded by whitespace
        if lower.ends_with(particle) {
            let before = &lower[..lower.len() - particle.len()];
            if before.is_empty() || before.ends_with(char::is_whitespace) {
                // Replace from the end of the actual particle onward with `?`
                let particle_start = stripped.len() - particle.len();
                let base = &stripped[..particle_start + particle.len()];
                return format!("{base}?");
            }
        }
    }

    text.to_string()
}

// ── Common Whisper-Turkish Substitutions ─────────────────────────────

/// Known Whisper hallucination/garble patterns for Turkish.
/// Each pair is (wrong, correct). Only high-confidence replacements.
static REPLACEMENTS: &[(&str, &str)] = &[
    ("göğlen", "görülen"),
    ("göğünmeyen", "görünmeyen"),
    ("göğlü", "görülü"),
    ("bilepini", "deneyimini"),
];

fn fix_substitutions(text: &str) -> String {
    let mut result = text.to_string();
    for &(wrong, correct) in REPLACEMENTS {
        // Case-sensitive replacement — Whisper output is typically lowercase
        result = result.replace(wrong, correct);
    }
    result
}

// ── Turkish Character Normalization ──────────────────────────────────

/// Fix common Whisper outputs that use wrong Turkish special characters.
/// Conservative: only patterns where Whisper consistently gets it wrong.
static CHAR_FIXES: &[(&str, &str)] = &[
    ("hültür", "kültür"),
    ("kültüğü", "kültürü"),
];

fn fix_turkish_chars(text: &str) -> String {
    let mut result = text.to_string();
    for &(wrong, correct) in CHAR_FIXES {
        result = result.replace(wrong, correct);
    }
    result
}

// ── Proper Noun Dictionary ──────────────────────────────────────────

/// Known proper nouns that Whisper garbles in Turkish audio.
static PROPER_NOUNS: &[(&str, &str)] = &[
    ("Peter Dubek", "Peter Drucker"),
    ("Aydigur Şahina", "Edgar Schein"),
    ("Antağı de Sen", "Antoine de Saint"),
];

fn fix_proper_nouns(text: &str) -> String {
    let mut result = text.to_string();
    for &(wrong, correct) in PROPER_NOUNS {
        result = result.replace(wrong, correct);
    }
    result
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn question_particle_appends_question_mark() {
        assert_eq!(fix_question_marks("Bu doğru mu"), "Bu doğru mu?");
        assert_eq!(fix_question_marks("Gelecek misiniz"), "Gelecek misiniz?");
        assert_eq!(fix_question_marks("Hazır mısın"), "Hazır mısın?");
    }

    #[test]
    fn question_mark_not_duplicated() {
        assert_eq!(fix_question_marks("Bu doğru mu?"), "Bu doğru mu?");
    }

    #[test]
    fn question_particle_replaces_period() {
        assert_eq!(fix_question_marks("Bu doğru mu."), "Bu doğru mu?");
    }

    #[test]
    fn no_false_positive_question_mark() {
        // "mu" inside a word should not trigger
        assert_eq!(fix_question_marks("Muammer geldi"), "Muammer geldi");
        assert_eq!(fix_question_marks("Mumya bulundu"), "Mumya bulundu");
    }

    #[test]
    fn substitution_fixes_known_garbles() {
        assert_eq!(fix_substitutions("göğlen hatalar"), "görülen hatalar");
        assert_eq!(fix_substitutions("göğünmeyen sorun"), "görünmeyen sorun");
    }

    #[test]
    fn proper_nouns_corrected() {
        assert_eq!(
            fix_proper_nouns("Peter Dubek demiştir ki"),
            "Peter Drucker demiştir ki"
        );
    }

    #[test]
    fn turkish_chars_fixed() {
        assert_eq!(fix_turkish_chars("hültür değişimi"), "kültür değişimi");
    }

    #[test]
    fn full_pipeline() {
        let input = "Peter Dubek hültür değişimi hakkında mı.";
        let output = process(input);
        assert_eq!(output, "Peter Drucker kültür değişimi hakkında mı?");
    }
}
