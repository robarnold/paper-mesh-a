use super::*;

fn tap(guide: &mut Guide, label: &str) {
    let control = guide
        .state()
        .controls
        .into_iter()
        .find(|control| control.label == label)
        .unwrap();
    guide
        .command(Command::TouchDown {
            id: 0,
            x: control.x,
            y: control.y,
        })
        .unwrap();
    guide
        .command(Command::TouchUp {
            id: 0,
            x: control.x,
            y: control.y,
        })
        .unwrap();
}

#[test]
fn pages_retain_state_and_run_lifecycle_callbacks_in_order() {
    let mut guide = Guide::default();
    assert_eq!(guide.state().home_entries, 1);
    assert!(guide.state().controls.iter().all(|c| c.label != "Back"));
    tap(&mut guide, "Open Detail");
    assert_eq!(guide.state().transition, "Home.leave → Detail.enter");
    tap(&mut guide, "Count a tap");
    tap(&mut guide, "Choice: unselected");
    tap(&mut guide, "Back");
    assert_eq!(guide.state().transition, "Detail.leave → Home.enter");
    tap(&mut guide, "Open Detail");
    let state = guide.state();
    assert_eq!((state.count, state.selected), (1, true));
    assert_eq!(
        (
            state.home_entries,
            state.home_exits,
            state.detail_entries,
            state.detail_exits
        ),
        (2, 2, 2, 1)
    );
    guide.command(Command::Reset).unwrap();
    assert_eq!((guide.state().count, guide.state().selected), (0, false));
}

#[test]
fn the_same_detail_page_returns_to_the_page_that_opened_it() {
    let mut guide = Guide::default();
    assert!(guide.state().back_stack.is_empty());
    tap(&mut guide, "Open Detail");
    assert_eq!(guide.state().return_to, Some("Home"));
    assert_eq!(guide.state().back_stack, ["Home"]);
    tap(&mut guide, "Count a tap");
    tap(&mut guide, "Back");
    assert!(guide.state().back_stack.is_empty());
    tap(&mut guide, "Open Tools");
    tap(&mut guide, "Open Detail");
    assert_eq!(guide.state().return_to, Some("Tools"));
    assert_eq!(guide.state().back_stack, ["Home", "Tools"]);
    assert_eq!(
        serde_json::to_value(guide.state()).unwrap()["back_stack"],
        serde_json::json!(["Home", "Tools"])
    );
    assert_eq!(guide.state().count, 1);
    tap(&mut guide, "Back");
    assert_eq!(guide.state().page, "Tools");
    assert_eq!(guide.state().back_stack, ["Home"]);
    assert_eq!(guide.state().transition, "Detail.leave → Tools.enter");
    tap(&mut guide, "Open Detail");
    guide.command(Command::Home).unwrap();
    assert_eq!(guide.state().return_to, None);
    assert!(guide.state().back_stack.is_empty());
    tap(&mut guide, "Open Detail");
    assert_eq!(guide.state().return_to, Some("Home"));
    guide.command(Command::Reset).unwrap();
    assert!(guide.state().back_stack.is_empty());
}

#[test]
fn full_screen_and_pending_modes_keep_system_navigation_rules() {
    let mut guide = Guide::default();
    tap(&mut guide, "Open Detail");
    tap(&mut guide, "Enter full screen");
    assert!(guide.state().full_screen);
    assert!(guide.state().controls.iter().all(|c| c.label != "Back"));
    guide.command(Command::Home).unwrap();
    assert_eq!(guide.state().page, "Home");
    tap(&mut guide, "Open Detail");
    assert!(!guide.state().full_screen);
    tap(&mut guide, "Start example operation");
    assert!(guide.state().controls.iter().all(|c| !c.enabled));
    guide.command(Command::Home).unwrap();
    tap(&mut guide, "Back");
    assert_eq!(guide.state().page, "Detail");
    let back = LAYOUT.back.center();
    guide
        .command(Command::TouchDown {
            id: 0,
            x: back.x,
            y: back.y,
        })
        .unwrap();
    guide.command(Command::Finish).unwrap();
    guide
        .command(Command::TouchUp {
            id: 0,
            x: back.x,
            y: back.y,
        })
        .unwrap();
    assert_eq!(guide.state().page, "Detail");
    tap(&mut guide, "Back");
    assert_eq!(guide.state().page, "Home");
}

#[test]
fn system_navigation_suppresses_held_contacts_and_rejects_invalid_input() {
    let mut guide = Guide::default();
    tap(&mut guide, "Open Detail");
    guide
        .command(Command::TouchDown {
            id: 0,
            x: 250,
            y: 264,
        })
        .unwrap();
    guide.command(Command::Home).unwrap();
    guide
        .command(Command::TouchMove {
            id: 0,
            x: 250,
            y: 264,
        })
        .unwrap();
    guide
        .command(Command::TouchUp {
            id: 0,
            x: 250,
            y: 264,
        })
        .unwrap();
    assert_eq!(guide.state().page, "Home");
    assert_eq!(guide.state().count, 0);
    for (id, x, y) in [(16, 20, 20), (0, -1, 20), (0, 540, 20), (0, 20, 960)] {
        assert!(guide.command(Command::TouchDown { id, x, y }).is_err());
    }
    tap(&mut guide, "Open Detail");
    assert_eq!(guide.state().page, "Detail");
}

fn control(guide: &Guide, label: &str) -> Control {
    guide
        .controls_state()
        .controls
        .into_iter()
        .find(|control| control.label == label)
        .unwrap()
}

fn tap_control(guide: &mut Guide, label: &str) {
    let control = control(guide, label);
    guide
        .controls_command(Command::TouchDown {
            id: 0,
            x: control.x,
            y: control.y,
        })
        .unwrap();
    guide
        .controls_command(Command::TouchUp {
            id: 0,
            x: control.x,
            y: control.y,
        })
        .unwrap();
}

#[test]
fn controls_change_the_frame_and_retain_state_after_back() {
    let mut guide = Guide::default();
    let before = guide.controls_png().unwrap();
    tap_control(&mut guide, "Action");
    assert_eq!(guide.controls_state().count, 1);
    assert_ne!(before, guide.controls_png().unwrap());
    tap_control(&mut guide, "Selected");
    assert!(!guide.controls_state().selected);
    tap_control(&mut guide, "Unavailable");
    assert_eq!(guide.controls_state().count, 1);
    assert!(!guide.controls_state().pending);
    tap_control(&mut guide, "Component details");
    assert_eq!(guide.controls_state().page, "Component details");
    tap_control(&mut guide, "Back");
    let state = guide.controls_state();
    assert_eq!(
        (state.page, state.count, state.selected),
        ("Controls", 1, false)
    );
}

#[test]
fn pending_controls_block_actions_until_finish_and_discard_disabled_presses() {
    let mut guide = Guide::default();
    tap_control(&mut guide, "Start operation");
    assert!(guide.controls_state().pending);
    for control in guide.controls_state().controls {
        assert_eq!(control.enabled, control.label == "Finish operation");
    }
    tap_control(&mut guide, "Action");
    tap_control(&mut guide, "Selected");
    tap_control(&mut guide, "Component details");
    assert_eq!(guide.controls_state().page, "Controls");
    assert_eq!(guide.controls_state().count, 0);
    assert!(guide.controls_state().selected);
    tap_control(&mut guide, "Finish operation");
    assert!(!guide.controls_state().pending);
    tap_control(&mut guide, "Action");
    assert_eq!(guide.controls_state().count, 1);

    tap_control(&mut guide, "Start operation");
    let action = control(&guide, "Action");
    guide
        .controls_command(Command::TouchDown {
            id: 0,
            x: action.x,
            y: action.y,
        })
        .unwrap();
    guide.controls_command(Command::Finish).unwrap();
    guide
        .controls_command(Command::TouchUp {
            id: 0,
            x: action.x,
            y: action.y,
        })
        .unwrap();
    assert_eq!(guide.controls_state().count, 1);
}

#[test]
fn controls_cancel_drags_and_multiple_contacts() {
    let mut guide = Guide::default();
    let action = control(&guide, "Action");
    for cancel in [
        Command::TouchMove {
            id: 0,
            x: 20,
            y: 500,
        },
        Command::TouchDown {
            id: 1,
            x: action.x,
            y: action.y,
        },
        Command::Cancel,
    ] {
        guide
            .controls_command(Command::TouchDown {
                id: 0,
                x: action.x,
                y: action.y,
            })
            .unwrap();
        guide.controls_command(cancel).unwrap();
        guide
            .controls_command(Command::TouchUp {
                id: 0,
                x: action.x,
                y: action.y,
            })
            .unwrap();
        guide
            .controls_command(Command::TouchUp {
                id: 1,
                x: action.x,
                y: action.y,
            })
            .unwrap();
        assert_eq!(guide.controls_state().count, 0);
    }
    tap_control(&mut guide, "Action");
    assert_eq!(guide.controls_state().count, 1);
}

#[test]
fn guide_examples_navigate_and_reset_independently() {
    let mut guide = Guide::default();
    tap(&mut guide, "Open Detail");
    tap(&mut guide, "Count a tap");
    tap_control(&mut guide, "Component details");
    guide.controls_command(Command::Reset).unwrap();
    assert_eq!(guide.controls_state().page, "Controls");
    assert_eq!(guide.state().page, "Detail");
    assert_eq!(guide.state().count, 1);
    tap_control(&mut guide, "Action");
    guide.command(Command::Reset).unwrap();
    assert_eq!(guide.controls_state().count, 1);
    assert_eq!(guide.state().page, "Home");
}

fn tap_style(guide: &mut Guide, label: &str) {
    let control = guide
        .text_state()
        .controls
        .into_iter()
        .find(|control| control.label == label)
        .unwrap();
    guide
        .text_command(Command::TouchDown {
            id: 0,
            x: control.x,
            y: control.y,
        })
        .unwrap();
    guide
        .text_command(Command::TouchUp {
            id: 0,
            x: control.x,
            y: control.y,
        })
        .unwrap();
}

#[test]
fn style_controls_change_the_rendered_sample_without_changing_other_examples() {
    let mut guide = Guide::default();
    let before = guide.text_png().unwrap();
    tap_style(&mut guide, "Caption");
    assert_eq!(guide.text_state().role, "Caption");
    let caption = guide.text_png().unwrap();
    assert_ne!(before, caption);
    tap_style(&mut guide, "Section gap");
    assert_eq!(guide.text_state().gap, STYLE.spacing.section_gap);
    let spacing = guide.text_png().unwrap();
    assert_ne!(caption, spacing);
    tap_style(&mut guide, "Control fill");
    assert_eq!(guide.text_state().shade, STYLE.control_fill.luma());
    assert_ne!(spacing, guide.text_png().unwrap());
    tap_style(&mut guide, "Underline");
    tap_style(&mut guide, "Bubbled");
    tap_style(&mut guide, "Secondary");
    assert!(guide.text_state().underlined);
    assert!(guide.text_state().bubbled);

    tap(&mut guide, "Open Detail");
    tap_control(&mut guide, "Action");
    guide.text_command(Command::Reset).unwrap();
    let state = guide.text_state();
    assert!(!state.underlined);
    assert!(!state.bubbled);
    assert_eq!(
        (state.role, state.spacing, state.color),
        ("Body", "Control gap", "Foreground")
    );
    assert_eq!(guide.state().page, "Detail");
    assert_eq!(guide.controls_state().count, 1);
    tap_style(&mut guide, "Secondary");
    guide.command(Command::Reset).unwrap();
    guide.controls_command(Command::Reset).unwrap();
    assert_eq!(guide.text_state().role, "Secondary");
}

#[test]
fn underline_changes_the_sample_for_every_text_role_and_can_be_removed() {
    let mut guide = Guide::default();
    for (_, label, _) in TEXT_ROLES {
        tap_style(&mut guide, label);
        let mut normal = Canvas::new();
        guide.text.example.ui.render(&mut normal, &STYLE).unwrap();

        tap_style(&mut guide, "Underline");
        assert!(guide.text_state().underlined);
        assert_eq!(guide.text_state().role, label);
        let mut underlined = Canvas::new();
        guide
            .text
            .example
            .ui
            .render(&mut underlined, &STYLE)
            .unwrap();
        // Compare the sample region, which excludes the toggle button.
        let sample = (360 * SIZE.width) as usize..(400 * SIZE.width) as usize;
        assert_ne!(normal.0[sample.clone()], underlined.0[sample]);

        tap_style(&mut guide, "Underline");
        assert!(!guide.text_state().underlined);
        let mut restored = Canvas::new();
        guide.text.example.ui.render(&mut restored, &STYLE).unwrap();
        assert_eq!(normal.0, restored.0);
    }
}

#[test]
fn bubbles_render_for_every_role_and_combine_with_underline() {
    let mut guide = Guide::default();
    for (_, label, _) in TEXT_ROLES {
        tap_style(&mut guide, label);
        let mut normal = Canvas::new();
        guide.text.example.ui.render(&mut normal, &STYLE).unwrap();
        tap_style(&mut guide, "Bubbled");
        assert!(guide.text_state().bubbled);
        let mut bubbled = Canvas::new();
        guide.text.example.ui.render(&mut bubbled, &STYLE).unwrap();
        let sample = (360 * SIZE.width) as usize..(400 * SIZE.width) as usize;
        assert_ne!(normal.0[sample.clone()], bubbled.0[sample.clone()]);

        tap_style(&mut guide, "Underline");
        assert!(guide.text_state().bubbled && guide.text_state().underlined);
        let mut underlined = Canvas::new();
        guide
            .text
            .example
            .ui
            .render(&mut underlined, &STYLE)
            .unwrap();
        assert_ne!(bubbled.0[sample.clone()], underlined.0[sample]);

        tap_style(&mut guide, "Bubbled");
        assert!(guide.text_state().underlined);
        tap_style(&mut guide, "Underline");
        let mut restored = Canvas::new();
        guide.text.example.ui.render(&mut restored, &STYLE).unwrap();
        assert_eq!(normal.0, restored.0);
    }
}

#[test]
fn catalogs_and_live_pages_render_at_the_documented_size() {
    let guide = Guide::default();
    for png in [
        guide.png(),
        guide.controls_png(),
        guide.text_png(),
        catalog_png(Sheet::Overview),
        catalog_png(Sheet::Text),
        catalog_png(Sheet::Controls),
    ] {
        let bytes = png.unwrap();
        let reader = png::Decoder::new(std::io::Cursor::new(bytes))
            .read_info()
            .unwrap();
        assert_eq!((reader.info().width, reader.info().height), (540, 960));
    }
    let snippets = snippets();
    assert_eq!(snippets.as_object().unwrap().len(), 9);
    assert!(
        snippets
            .as_object()
            .unwrap()
            .values()
            .all(|value| !value.as_str().unwrap().is_empty())
    );
}
