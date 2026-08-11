//! Tests for the `Esc Esc` clear gesture on a non-empty composer.
//!
//! The gesture shares its clear procedure with Ctrl+C (`ChatComposer::clear_draft_to_history`), so
//! these tests focus on the parts that are specific to Esc: which contexts arm the latch, which
//! contexts must never consume it, and that clearing leaves the composer in the state the
//! backtrack gesture expects.

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;
use std::path::PathBuf;

use super::ChatComposer;
use super::FooterMode;
use super::InputResult;
use super::tests::new_test_composer;

fn press(composer: &mut ChatComposer, code: KeyCode) -> InputResult {
    composer
        .handle_key_event(KeyEvent::new(code, KeyModifiers::NONE))
        .0
}

fn press_esc(composer: &mut ChatComposer) -> InputResult {
    press(composer, KeyCode::Esc)
}

fn with_draft(text: &str) -> ChatComposer {
    let (mut composer, _rx) = new_test_composer();
    composer.set_text_content(text.to_string(), Vec::new(), Vec::new());
    composer.move_cursor_to_end();
    composer
}

#[test]
fn second_esc_clears_draft_and_hands_text_to_host() {
    let mut composer = with_draft("draft text");

    press_esc(&mut composer);

    assert_eq!(
        press_esc(&mut composer),
        InputResult::DraftCleared {
            text: "draft text".to_string()
        }
    );
    assert!(composer.is_empty());
}

#[test]
fn cleared_draft_is_recoverable_from_local_history() {
    let mut composer = with_draft("draft text");

    press_esc(&mut composer);
    press_esc(&mut composer);

    press(&mut composer, KeyCode::Up);
    assert_eq!(composer.current_text(), "draft text");
}

#[test]
fn esc_esc_clears_attachments_and_shell_mode_with_the_text() {
    let (mut composer, _rx) = new_test_composer();
    composer.set_text_content(
        "!look at this".to_string(),
        Vec::new(),
        vec![PathBuf::from("/tmp/screenshot.png")],
    );

    press_esc(&mut composer);
    press_esc(&mut composer);

    // `is_empty` is what the backtrack gesture gates on, and it accounts for shell mode and
    // attachments as well as text; a partial clear would leave it false.
    assert!(composer.is_empty());
    assert_eq!(composer.current_text(), "");
}

#[test]
fn editing_between_escapes_disarms_the_clear() {
    let mut composer = with_draft("draft text");

    press_esc(&mut composer);
    press(&mut composer, KeyCode::Backspace);

    assert_eq!(press_esc(&mut composer), InputResult::None);
    assert_eq!(composer.current_text(), "draft tex");
}

#[test]
fn esc_after_dismissing_a_popup_does_not_clear_the_draft() {
    // The popup path arms the shared `EscHint` mode and popup dismissal does not reset it, so a
    // latch keyed on that mode would clear here on what the user experiences as a single Esc.
    let (mut composer, _rx) = new_test_composer();
    composer.set_text_content("@src".to_string(), Vec::new(), Vec::new());
    assert!(
        composer.popup_active(),
        "expected the file popup to be open"
    );

    press_esc(&mut composer);
    press_esc(&mut composer);
    assert!(!composer.popup_active(), "expected the popup to be closed");

    assert_eq!(press_esc(&mut composer), InputResult::None);
    assert_eq!(composer.current_text(), "@src");
}

#[test]
fn esc_does_not_clear_while_a_task_is_running() {
    let mut composer = with_draft("draft text");
    composer.set_task_running(true);

    press_esc(&mut composer);
    assert_eq!(press_esc(&mut composer), InputResult::None);

    assert_eq!(composer.current_text(), "draft text");
}

#[test]
fn latch_armed_while_idle_does_not_survive_a_task() {
    let mut composer = with_draft("draft text");
    press_esc(&mut composer);
    assert_eq!(composer.footer_mode(), FooterMode::EscClearHint);

    // A turn can start between the two presses. Esc belongs to the interrupt gesture from here.
    composer.set_task_running(true);
    assert_eq!(press_esc(&mut composer), InputResult::None);
    assert_eq!(composer.footer_mode(), FooterMode::ComposerHasDraft);

    // And the stale latch must not come back to life once the turn ends.
    composer.set_task_running(false);
    assert_eq!(press_esc(&mut composer), InputResult::None);
    assert_eq!(composer.current_text(), "draft text");
}

#[test]
fn esc_does_not_clear_in_vim_normal_mode() {
    let mut composer = with_draft("draft text");
    composer.set_vim_enabled(true);

    // Leaving insert mode is the first Esc a Vim user presses; further presses are reflex.
    press_esc(&mut composer);
    press_esc(&mut composer);
    assert_eq!(press_esc(&mut composer), InputResult::None);

    assert_eq!(composer.current_text(), "draft text");
}

#[test]
fn esc_still_exits_shell_mode_before_arming_the_clear() {
    let (mut composer, _rx) = new_test_composer();
    composer.set_text_content("!".to_string(), Vec::new(), Vec::new());

    assert_eq!(press_esc(&mut composer), InputResult::None);

    assert!(composer.is_empty(), "expected Esc to leave shell mode");
    assert_ne!(composer.footer_mode(), FooterMode::EscClearHint);
}

#[test]
fn empty_composer_still_arms_the_backtrack_hint() {
    let (mut composer, _rx) = new_test_composer();

    assert_eq!(press_esc(&mut composer), InputResult::None);

    assert_eq!(composer.footer_mode(), FooterMode::EscHint);
}

#[test]
fn clearing_hands_the_gesture_back_to_backtrack() {
    let mut composer = with_draft("draft text");

    press_esc(&mut composer);
    press_esc(&mut composer);

    // Clearing empties the composer, so the next Esc arms the backtrack hint the App layer owns
    // rather than another clear.
    assert_eq!(press_esc(&mut composer), InputResult::None);
    assert_eq!(composer.footer_mode(), FooterMode::EscHint);
}

#[test]
fn paste_between_escapes_requires_a_fresh_clear_gesture() {
    let mut composer = with_draft("draft text");
    press_esc(&mut composer);
    composer.handle_paste(" pasted".to_string());

    assert_eq!(press_esc(&mut composer), InputResult::None);
    assert_eq!(composer.current_text(), "draft text pasted");
    assert_eq!(composer.footer_mode(), FooterMode::EscClearHint);
}

#[test]
fn external_edit_between_escapes_requires_a_fresh_clear_gesture() {
    let mut composer = with_draft("draft text");
    press_esc(&mut composer);
    composer.apply_external_edit("edited in $EDITOR".to_string());

    assert_eq!(press_esc(&mut composer), InputResult::None);
    assert_eq!(composer.current_text(), "edited in $EDITOR");
    assert_eq!(composer.footer_mode(), FooterMode::EscClearHint);
}

#[test]
fn image_attachment_between_escapes_requires_a_fresh_clear_gesture() {
    let mut composer = with_draft("draft text");
    press_esc(&mut composer);
    composer.attach_image(PathBuf::from("/tmp/probe.png"));

    assert_eq!(press_esc(&mut composer), InputResult::None);
    assert_eq!(composer.current_text(), "draft text[Image #1]");
    assert_eq!(composer.local_images().len(), 1);
    assert_eq!(composer.footer_mode(), FooterMode::EscClearHint);
}

#[test]
fn remote_image_selection_between_escapes_requires_a_fresh_clear_gesture() {
    let (mut composer, _rx) = new_test_composer();
    composer.set_text_content("draft text".to_string(), Vec::new(), Vec::new());
    composer.set_remote_image_urls(vec!["https://example.com/one.png".to_string()]);
    press_esc(&mut composer);

    press(&mut composer, KeyCode::Up);
    assert_eq!(composer.footer_mode(), FooterMode::ComposerHasDraft);

    assert_eq!(press_esc(&mut composer), InputResult::None);
    assert_eq!(composer.current_text(), "draft text");
    assert_eq!(composer.footer_mode(), FooterMode::EscClearHint);
}

#[test]
fn remote_image_deletion_between_escapes_requires_a_fresh_clear_gesture() {
    let (mut composer, _rx) = new_test_composer();
    composer.set_text_content("draft text".to_string(), Vec::new(), Vec::new());
    composer.set_remote_image_urls(vec!["https://example.com/one.png".to_string()]);
    press_esc(&mut composer);

    press(&mut composer, KeyCode::Up);
    press(&mut composer, KeyCode::Backspace);
    assert!(composer.remote_image_urls().is_empty());
    assert_eq!(composer.footer_mode(), FooterMode::ComposerHasDraft);

    assert_eq!(press_esc(&mut composer), InputResult::None);
    assert_eq!(composer.current_text(), "draft text");
    assert_eq!(composer.footer_mode(), FooterMode::EscClearHint);
}
