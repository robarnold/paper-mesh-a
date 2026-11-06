use super::*;
use embedded_graphics::pixelcolor::Gray4;
use paper_mesh_platform::TouchEvent;

struct FaultDisplay {
    fail_draw: bool,
    fail_refresh: bool,
}
impl OriginDimensions for FaultDisplay {
    fn size(&self) -> Size {
        crate::SCREEN_SIZE
    }
}
impl DrawTarget for FaultDisplay {
    type Color = Gray4;
    type Error = &'static str;
    fn draw_iter<I: IntoIterator<Item = Pixel<Gray4>>>(&mut self, _: I) -> Result<(), Self::Error> {
        if self.fail_draw { Err("draw") } else { Ok(()) }
    }
}
impl paper_mesh_platform::Display for FaultDisplay {
    fn is_busy(&self) -> bool {
        false
    }
    fn refresh(&mut self) -> Result<(), Self::Error> {
        if self.fail_refresh {
            Err("refresh")
        } else {
            Ok(())
        }
    }
}

// Drawing and submission failures retain pending work until a successful refresh.
#[test]
fn application_retains_damage_after_display_failures() {
    let mut app = crate::App::new_at(0);
    let mut display = FaultDisplay {
        fail_draw: false,
        fail_refresh: false,
    };
    app.present(&mut display).unwrap();
    app.display_completed(0);
    let p = HOME_TARGET.center();
    app.touch(TouchEvent::touch(0, p));
    app.touch(TouchEvent::release(0, p));
    display.fail_draw = true;
    assert_eq!(app.present(&mut display), Err("draw"));
    assert!(app.input_ready());
    assert!(app.calculator().input_ready());
    display.fail_draw = false;
    display.fail_refresh = true;
    assert_eq!(app.present(&mut display), Err("refresh"));
    assert!(app.input_ready());
    display.fail_refresh = false;
    assert_eq!(app.present(&mut display), Ok(true));
    assert!(app.calculator().input_ready());
    app.display_completed(0);
    assert!(app.calculator().input_ready());
    assert_eq!(app.present(&mut display), Ok(false));
    app.display_failed();
    assert!(app.calculator().input_ready());
    assert_eq!(app.present(&mut display), Ok(true));
    app.display_completed(1);
    assert!(app.calculator().input_ready());
}

fn enter(model: &mut Calculator, text: &str) {
    for c in text.chars() {
        model.press(match c {
            '.' => Key::Decimal,
            '-' => Key::Sign,
            _ => Key::Digit(c.to_digit(10).unwrap() as u8),
        });
    }
}

// Sequential operations resolve from left to right, and repeated equals preserves the result.
#[test]
fn sequential_arithmetic_and_decimal_results() {
    let mut model = Calculator::default();
    enter(&mut model, "2");
    model.press(Key::Add);
    enter(&mut model, "3");
    model.press(Key::Multiply);
    enter(&mut model, "4");
    model.press(Key::Equals);
    assert_eq!(model.value(), "20");
    model.press(Key::Equals);
    assert_eq!(model.value(), "20");
    model.press(Key::Clear);
    enter(&mut model, "0.1");
    model.press(Key::Add);
    enter(&mut model, "0.2");
    model.press(Key::Equals);
    assert_eq!(model.value(), "0.3");
    model.press(Key::Clear);
    enter(&mut model, "-2");
    model.press(Key::Divide);
    enter(&mut model, "3");
    model.press(Key::Equals);
    assert_eq!(model.value(), "-0.666667");
}

// Missing operands remain editable, operator replacement is explicit, and errors recover on new input.
#[test]
fn editing_incomplete_operations_and_errors() {
    let mut model = Calculator::default();
    enter(&mut model, "12");
    model.press(Key::Delete);
    assert_eq!(model.value(), "1");
    model.press(Key::Add);
    model.press(Key::Equals);
    assert!(model.notice().contains("next number"));
    model.press(Key::Subtract);
    enter(&mut model, "3");
    model.press(Key::Equals);
    assert_eq!(model.value(), "-2");
    model.press(Key::Divide);
    enter(&mut model, "0");
    model.press(Key::Equals);
    assert!(model.notice().contains("zero"));
    enter(&mut model, "7");
    assert_eq!(model.value(), "7");
    assert!(!model.failed);
}

// Input limits preserve accepted digits and arithmetic overflow reports a recoverable failure.
#[test]
fn capacity_overflow_and_rounding() {
    let mut model = Calculator::default();
    enter(&mut model, "9999999999.1234567");
    assert_eq!(model.value(), "999999999.123456");
    model.press(Key::Add);
    enter(&mut model, "1");
    model.press(Key::Equals);
    assert!(model.failed);
    assert!(model.notice().contains("Numeric limit"));
    model.press(Key::Clear);
    enter(&mut model, "0.000001");
    model.press(Key::Divide);
    enter(&mut model, "2");
    model.press(Key::Equals);
    assert_eq!(model.value(), "0.000001");
}

// The latest committed keypad accepts taps before presentation and retains state across visits.
#[test]
fn keypad_routes_complete_taps_and_retains_page_state() {
    use paper_mesh_platform::ui::{Navigator, Shell};
    let mut ui = Navigator::<_, 0>::launch(
        Route::Calculator,
        Shell::new(crate::screen::LAYOUT),
        CalculatorPage::default(),
    );
    let p = ui.pages().key_bounds(4).center();
    for expected in ["7", "77"] {
        ui.touch(TouchEvent::touch(0, p)).unwrap();
        ui.touch(TouchEvent::release(0, p)).unwrap();
        assert_eq!(ui.pages().value(), expected);
    }
    assert!(ui.input_ready());
}
